"""Spike 1 harness: drive `claude -p` over stream-json + the control protocol and record every line.

Usage: python3 -I harness.py <scenario> <fixture-out.jsonl>
Scenarios are defined at the bottom. Every line in both directions is written to the fixture with a monotonic timestamp.
"""

import asyncio
import json
import os
import subprocess
import sys
import time

S = os.path.dirname(os.path.abspath(__file__))
JUDGESS = os.path.join(S, "plugin", "judgess")
WS = os.path.join(S, "ws")
T0 = time.monotonic()


def now_ms():
    return round((time.monotonic() - T0) * 1000, 2)


class Session:
    def __init__(self, args, fixture, hook_mode):
        self.args = args
        self.fixture = open(fixture, "w")
        self.hook_mode = hook_mode  # "callback" (judgess answers hook_callback), "can_use_tool" (judgess answers can_use_tool), None
        self.pending = {}
        self.counter = 0
        self.events = asyncio.Queue()
        self.timings = []
        self.inflight = {}
        self.prompt_delay = float(os.environ.get("PROMPT_DELAY", "0"))

    def log(self, direction, obj):
        self.fixture.write(json.dumps({"t": now_ms(), "dir": direction, "msg": obj}) + "\n")
        self.fixture.flush()

    async def start(self):
        env_script = os.path.join(S, "cleanenv.sh")
        self.proc = await asyncio.create_subprocess_exec(
            env_script, "env", f"HOME={os.path.join(S, 'home')}", "claude", *self.args,
            cwd=WS, stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE, limit=1 << 24,
        )
        self.reader = asyncio.create_task(self._read())
        self.err_reader = asyncio.create_task(self._read_err())

    async def _read_err(self):
        async for line in self.proc.stderr:
            self.log("stderr", line.decode(errors="replace").rstrip())

    async def write(self, obj):
        self.log("out", obj)
        self.proc.stdin.write((json.dumps(obj) + "\n").encode())
        await self.proc.stdin.drain()

    async def control(self, request, timeout=60):
        self.counter += 1
        rid = f"req_{self.counter}"
        fut = asyncio.get_running_loop().create_future()
        self.pending[rid] = fut
        await self.write({"type": "control_request", "request_id": rid, "request": request})
        return await asyncio.wait_for(fut, timeout)

    async def user(self, text):
        await self.write({"type": "user", "message": {"role": "user", "content": text},
                          "parent_tool_use_id": None, "session_id": ""})

    async def _read(self):
        async for raw in self.proc.stdout:
            line = raw.decode(errors="replace").strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                self.log("in-raw", line)
                continue
            self.log("in", msg)
            kind = msg.get("type")
            if kind == "control_response":
                resp = msg["response"]
                fut = self.pending.pop(resp.get("request_id"), None)
                if fut and not fut.done():
                    fut.set_result(resp)
            elif kind == "control_request":
                self.inflight[msg["request_id"]] = asyncio.create_task(self._answer(msg))
            elif kind == "control_cancel_request":
                task = self.inflight.pop(msg.get("request_id"), None)
                if task:
                    task.cancel()
            else:
                await self.events.put(msg)
        await self.events.put({"type": "__eof__"})

    def judgess(self, hook_input):
        started = time.perf_counter()
        out = subprocess.run([JUDGESS, "hook"], input=json.dumps(hook_input), capture_output=True, text=True,
                             env={"HOME": os.path.join(S, "home")}).stdout.strip()
        self.timings.append((time.perf_counter() - started) * 1000)
        return json.loads(out) if out else {}

    async def _answer(self, msg):
        rid = msg["request_id"]
        req = msg["request"]
        sub = req.get("subtype")
        if sub == "hook_callback" and req.get("callback_id") == "judgess_pre":
            hook_input = dict(req.get("input") or {}, hook_event_name="PermissionRequest")
            decision = (self.judgess(hook_input).get("hookSpecificOutput") or {}).get("decision") or {}
            body = {}
            if decision.get("behavior") == "allow":
                body = {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "allow",
                                               "permissionDecisionReason": "Approved by Judgess"}}
            elif decision.get("behavior") == "deny":
                body = {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny",
                                               "permissionDecisionReason": decision.get("message", "Denied by Judgess")}}
        elif sub == "hook_callback":
            out = self.judgess(req.get("input") or {}) if self.hook_mode == "callback" else {}
            body = out
        elif sub == "can_use_tool":
            body = None
            if self.hook_mode == "can_use_tool":
                hook_input = {"hook_event_name": "PermissionRequest", "tool_name": req["tool_name"],
                              "tool_input": req["input"], "cwd": WS, "session_id": "spike"}
                out = self.judgess(hook_input)
                decision = (out.get("hookSpecificOutput") or {}).get("decision")
                if decision:
                    body = dict(decision)
                    if body["behavior"] == "allow":
                        body["updatedInput"] = req["input"]
            if body is None:
                await asyncio.sleep(self.prompt_delay)  # a human taking time to answer the native prompt
                # Stand-in for Sapphire's native prompt: the "user" approves anything Judgess left open.
                body = {"behavior": "allow", "updatedInput": req["input"]}
        else:
            body = {}
        await self.write({"type": "control_response",
                          "response": {"subtype": "success", "request_id": rid, "response": body}})

    async def until_result(self, timeout=240):
        while True:
            msg = await asyncio.wait_for(self.events.get(), timeout)
            if msg.get("type") in ("result", "__eof__"):
                return msg

    async def close(self):
        self.proc.stdin.close()
        try:
            await asyncio.wait_for(self.proc.wait(), 30)
        except asyncio.TimeoutError:
            self.proc.kill()
        self.fixture.close()


BASE = ["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose",
        "--include-partial-messages", "--include-hook-events", "--setting-sources", "", "--model", "haiku"]
PERM_HOOK = {"PermissionRequest": [{"matcher": "^(?!ExitPlanMode$).*", "hookCallbackIds": ["judgess"]}]}

PROMPT_TOOLS = (
    "Use the Bash tool to run each of these commands, one tool call per command, in this order, "
    "and do not run anything else: (1) `git status`  (2) `eval echo hi`  (3) `touch scratch-file.txt`. "
    "After each, briefly say what happened. If a command is denied, move on to the next one."
)


async def scenario(name, fixture):
    if name == "basic":  # two turns in one process: streaming, usage, caching
        s = Session(BASE + ["--permission-mode", "manual"], fixture, None)
        await s.start()
        print("init", await s.control({"subtype": "initialize", "hooks": None}))
        await s.user("Reply with one short sentence about prompt caching.")
        print("r1", (await s.until_result()).get("usage"))
        await s.user("Now reply with one short sentence about compaction.")
        print("r2", (await s.until_result()).get("usage"))
        await s.close()
    elif name in ("callback", "callback-auto", "cut", "cut-auto"):
        mode = "auto" if name.endswith("auto") else "manual"
        hook_mode = "callback" if name.startswith("callback") else "can_use_tool"
        args = BASE + ["--permission-mode", mode, "--permission-prompt-tool", "stdio"]
        s = Session(args, fixture, hook_mode)
        await s.start()
        init = await s.control({"subtype": "initialize", "hooks": PERM_HOOK if hook_mode == "callback" else None})
        print("init", init.get("subtype"))
        await s.user(PROMPT_TOOLS)
        print("result", (await s.until_result()).get("result", "")[:300])
        print("judgess ms", [round(t, 2) for t in s.timings])
        await s.close()
    elif name in ("pre", "pre-auto"):
        mode = "auto" if name.endswith("auto") else "manual"
        args = BASE + ["--permission-mode", mode, "--permission-prompt-tool", "stdio"]
        s = Session(args, fixture, "callback")
        await s.start()
        hooks = {"PreToolUse": [{"matcher": "Bash", "hookCallbackIds": ["judgess_pre"]}]}
        print("init", (await s.control({"subtype": "initialize", "hooks": hooks})).get("subtype"))
        await s.user(PROMPT_TOOLS.replace("`git status`", "`git log --oneline -3`").replace("`eval echo hi`", "`cargo test`"))
        print("result", (await s.until_result()).get("result", "")[:300])
        print("judgess ms", [round(t, 2) for t in s.timings])
        await s.close()
    elif name in ("plugin", "plugin-auto"):
        mode = "auto" if name.endswith("auto") else "manual"
        args = BASE + ["--permission-mode", mode, "--permission-prompt-tool", "stdio", "--plugin-dir", os.path.join(S, "plugin")]
        s = Session(args, fixture, None)
        await s.start()
        print("init", (await s.control({"subtype": "initialize", "hooks": None})).get("subtype"))
        await s.user(PROMPT_TOOLS)
        print("result", (await s.until_result()).get("result", "")[:300])
        await s.close()
    elif name == "runtime":  # set_permission_mode, interrupt, /compact, then kill and resume
        s = Session(BASE + ["--permission-mode", "manual", "--permission-prompt-tool", "stdio"], fixture, None)
        await s.start()
        await s.control({"subtype": "initialize", "hooks": None})
        print("set_mode", await s.control({"subtype": "set_permission_mode", "mode": "acceptEdits"}))
        await s.user("Remember this code word: AMBER-42. Reply with just 'noted'.")
        r = await s.until_result()
        sid = r.get("session_id")
        print("r1", r.get("result"), sid)
        await s.user("Count slowly from 1 to 200, one number per line.")
        await asyncio.sleep(3)
        print("interrupt", await s.control({"subtype": "interrupt"}))
        print("r2", (await s.until_result()).get("subtype"))
        await s.user("/compact")
        print("r3", json.dumps(await s.until_result())[:400])
        s.proc.kill()
        await s.proc.wait()
        s.fixture.close()
        s2 = Session(BASE + ["--permission-mode", "manual", "--resume", sid], fixture + ".resume.jsonl", None)
        await s2.start()
        await s2.control({"subtype": "initialize", "hooks": None})
        await s2.user("What was the code word I asked you to remember? Reply with just the word.")
        r = await s2.until_result()
        print("resume", r.get("result"), r.get("session_id"), r.get("usage"))
        await s2.close()


if __name__ == "__main__":
    asyncio.run(scenario(sys.argv[1], sys.argv[2]))
