#!/usr/bin/env python3
"""Deterministic subprocess peer using the recorded Claude wire contract."""
import copy
import json
import os
import pathlib
import sys
import time

args = sys.argv[1:]
def argument(name):
    return args[args.index(name) + 1]

# A probe (`devora_agent::models::discover`) runs no session, so it has neither.
session_flag = next((flag for flag in ("--resume", "--session-id") if flag in args), None)
session_id = argument(session_flag) if session_flag else None
root = pathlib.Path(os.environ["DEVORA_FAKE_FIXTURES"])
recording = [json.loads(line) for line in (root / "basic.jsonl").read_text().splitlines()]
def recorded(kind):
    return copy.deepcopy(next(row["msg"] for row in recording
        if isinstance(row.get("msg"), dict) and row["msg"].get("type") == kind))

# The version of the recording it answers with, as a real CLI's `--version` agrees with its own initialize reply: the core
# caches the model list per CLI version, and a fake that disagreed would never be served from that cache.
if "--version" in sys.argv:
    recorded_version = recorded("control_response")["response"]["response"]["claude_code_version"]
    print(os.environ.get("DEVORA_FAKE_VERSION", f"{recorded_version} (Claude Code)"))
    sys.exit(0)

def report(value):
    with open(os.environ["DEVORA_FAKE_REPORT"], "a") as stream:
        stream.write(json.dumps(value) + "\n")

def emit(value):
    if isinstance(value, dict) and "session_id" in value:
        value["session_id"] = session_id
    print(json.dumps(value), flush=True)

def unrequested_interrupted_result():
    value = recorded("result")
    value.update(subtype="error_during_execution", is_error=True, terminal_reason="aborted_streaming",
        num_turns=1, origin={"kind":"task-notification"})
    value.pop("user_message_uuids", None)
    emit(value)

def lifecycle(command, state):
    # Where a prompt is, by the ID it was sent with: queued, started (the agent has it), completed (its turn ended).
    emit({"type":"command_lifecycle","command_uuid":command,"state":state,"session_id":session_id,"uuid":str(time.time())})

sent_turn = None
def result(interrupted=False):
    value = recorded("result")
    if interrupted:
        value.update(subtype="error_during_execution", is_error=True, terminal_reason="aborted_streaming")
    value["user_message_uuids"] = [sent_turn]
    emit(value)
    if sent_turn:
        lifecycle(sent_turn, "completed")

# With DEVORA_FAKE_FOLDED_WAITS, a prompt sent into a turn of the CLI's own waits, queued, until the agent's next step
# (the test's `step` cue), as the CLI's does until the tool call in progress ends.
held = None

def unrequested_result():
    # What the CLI emits for a turn it starts itself (background work finishing),
    # or for a resume after a kill: a result naming no user message.
    value = recorded("result")
    value.update(num_turns=0, origin={"kind":"task-notification"})
    value.pop("user_message_uuids", None)
    emit(value)

def save(message):
    # Claude's saved session, where an accepted user turn keeps its uuid. Only
    # with CLAUDE_CONFIG_DIR, so a run never writes into a real ~/.claude.
    config = os.environ.get("CLAUDE_CONFIG_DIR")
    if config:
        project = pathlib.Path(config) / "projects" / "fake-project"
        project.mkdir(parents=True, exist_ok=True)
        with open(project / f"{session_id}.jsonl", "a") as stream:
            stream.write(json.dumps({"type":"user","uuid":message.get("uuid"),"message":message["message"]}) + "\n")

import threading
output = threading.Lock()
background = []
# A stopped subagent's notification turn, running until it's interrupted.
notification_turn = False

def background_changed():
    emit({"type":"system","subtype":"background_tasks_changed","session_id":session_id,"tasks":list(background)})

def start_notification_turn():
    global notification_turn
    with output:
        notification_turn = True
        report({"kind":"notification_turn"})
        emit(recorded("system"))
        emit(recorded("stream_event"))

def own_turns(directory):
    """Turns the CLI starts itself, as when a background task reports, on the test's cues in `directory`: `start` opens
    one, then `end` ends it with a result naming no user message, `crash` kills the CLI during it, or `step` gives the
    agent the prompt held in it (`held`), which then ends the turn as that prompt's."""
    global notification_turn, held, sent_turn
    def cue(*names):
        while True:
            for name in names:
                if (directory / name).exists():
                    (directory / name).unlink()
                    return name
            time.sleep(0.01)
    while True:
        cue("start")
        start_notification_turn()
        name = cue("end", "crash", "step")
        if name == "crash":
            os._exit(70)
        if name == "step":
            with output:
                notification_turn = False
                sent_turn, held = held, None
                lifecycle(sent_turn, "started")
                emit(recorded("assistant"))
                result()
            continue
        with output:
            # A prompt sent during the turn was folded into it and already answered it.
            if notification_turn:
                notification_turn = False
                report({"kind":"own_turn_ended"})
                unrequested_result()

if os.environ.get("DEVORA_FAKE_OWN_TURNS"):
    threading.Thread(target=own_turns, args=(pathlib.Path(os.environ["DEVORA_FAKE_OWN_TURNS"]),), daemon=True).start()

def question():
    emit({"type":"control_request","request_id":"question","request":{
        "subtype":"can_use_tool","tool_name":"Bash","input":{"command":"custom-tool"},
        "tool_use_id":"tool-1","decision_reason":"Ask rule",
        "permission_suggestions":[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"custom-tool"}],
            "behavior":"allow","destination":"localSettings"}]}})

ignore_interrupt = False
visual_tool = False
reply_after_answer = False
report({"kind":"start","args":args,"environment":dict(os.environ),"cwd":os.getcwd(),"pid":os.getpid()})
for line in sys.stdin:
    message = json.loads(line)
    kind = message["type"]
    if kind == "control_request":
        request = message["request"]
        subtype = request["subtype"]
        report({"kind":subtype, **({"model":request["model"]} if subtype == "set_model" else {}),
            **({"settings":request["settings"]} if subtype == "apply_flag_settings" else {})})
        if subtype == "interrupt" and ignore_interrupt:
            continue
        if subtype == "stop_task":
            task_id = request["task_id"]
            report({"kind":"stop_task","task_id":task_id})
            if os.environ.get("DEVORA_FAKE_STOP_FAILS") == task_id:
                emit({"type":"control_response","response":{"subtype":"error","request_id":message["request_id"],"error":"the task won't stop"}})
                continue
            task = next(task for task in background if task["task_id"] == task_id)
            background.remove(task)
            emit({"type":"control_response","response":{"subtype":"success","request_id":message["request_id"],"response":{}}})
            emit({"type":"system","subtype":"task_notification","session_id":session_id,"task_id":task_id,"status":"stopped"})
            background_changed()
            if task["task_type"] == "local_agent":
                # As the CLI does: a turn of its own about the stopped subagent, here
                # optionally starting only after a delay.
                delay = float(os.environ.get("DEVORA_FAKE_AGENT_TURN_DELAY", "0"))
                threading.Timer(delay, start_notification_turn).start()
            continue
        if subtype == "set_model" and request.get("model") == "hang":
            emit({"type":"fake_control_waiting"})
            continue
        if subtype == "apply_flag_settings":
            # As the CLI answers it: a success with no `response`, and an error only when it can't take the request at all.
            refused = os.environ.get("DEVORA_FAKE_EFFORT_REFUSED")
            if refused and request.get("settings", {}).get("effortLevel") == refused:
                emit({"type":"control_response","response":{"subtype":"error","request_id":message["request_id"],"error":"effort refused"}})
            else:
                emit({"type":"control_response","response":{"subtype":"success","request_id":message["request_id"]}})
            continue
        if subtype == "set_model" and request.get("model") == "refused":
            emit({"type":"control_response","response":{"subtype":"error","request_id":message["request_id"],"error":"model refused"}})
            continue
        if subtype == "initialize":
            if session_id is None and os.environ.get("DEVORA_FAKE_PROBE_STALL"):
                # A model lookup that never gets its answer, for ending it from outside.
                report({"kind":"probe-stalled","pid":os.getpid()})
                continue
            if os.environ.get("DEVORA_FAKE_INIT_ERROR"):
                emit({"type":"control_response","response":{"subtype":"error","request_id":message["request_id"],"error":"initialize refused"}})
                continue
            response = recorded("control_response")
            response["response"]["request_id"] = message["request_id"]
            emit(response)
            if os.environ.get("DEVORA_FAKE_CRASH_UNUSED"):
                marker = pathlib.Path(os.environ["DEVORA_FAKE_CRASH_UNUSED"])
                if not marker.exists():
                    marker.touch()
                    os._exit(70)
        else:
            emit({"type":"control_response","response":{"subtype":"success","request_id":message["request_id"],
                "response":{"subtype":subtype,"mode":request.get("mode"),"model":request.get("model")}}})
            if subtype == "interrupt" and notification_turn:
                notification_turn = False
                unrequested_interrupted_result()
            elif subtype == "interrupt":
                result(True)
    elif kind == "control_response":
        if visual_tool:
            emit({"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"visual-tool","content":"Visual tool result"}]}})
            visual_tool = False
        emit({"type":"fake_answer","answer":message["response"].get("response", {"error":message["response"].get("error")})})
        if reply_after_answer:
            reply_after_answer = False
            emit(recorded("assistant"))
        result()
    elif kind == "user":
        report({"kind":"user","message":message["message"],"uuid":message.get("uuid")})
        # As the CLI does, a prompt sent during a turn it started itself joins that turn, whose one result answers the
        # prompt, and opens no turn of its own (probed on the pinned CLI).
        with output:
            folded = notification_turn
            waits = folded and bool(os.environ.get("DEVORA_FAKE_FOLDED_WAITS"))
            notification_turn = notification_turn and waits
            lifecycle(message.get("uuid"), "queued")
            if waits:
                held = message.get("uuid")
                report({"kind":"held","uuid":held})
            else:
                lifecycle(message.get("uuid"), "started")
        if waits:
            continue
        sent_turn = message.get("uuid")
        text = message["message"]["content"]
        # Dies once on Devora's restart notice, before or after saving it, as a crash mid-delivery would.
        marker = os.environ.get("DEVORA_FAKE_NOTICE_CRASH")
        crash = (isinstance(text, str) and text.startswith("Devora restarted your session")
            and marker and not pathlib.Path(marker).exists())
        if not crash or os.environ.get("DEVORA_FAKE_NOTICE_SAVED"):
            save(message)
        if crash:
            pathlib.Path(marker).touch()
            os._exit(70)
        # As the CLI does, `init` opens each turn, and nothing else sends one (the recordings).
        if not folded:
            emit(recorded("system"))
        if text == "permission":
            question()
        elif text == "permission-then-reply":
            question()
            reply_after_answer = True
        elif text == "cancel-permission":
            question()
            emit({"type":"control_cancel_request","request_id":"question"})
            result()
        elif text == "chat-fixture":
            assistant = recorded("assistant")
            assistant["message"]["content"] = [
                {"type":"thinking","thinking":"A collapsed thought","signature":"test-signature"},
                {"type":"text","text":"A visible answer"},
                {"type":"tool_use","id":"visual-tool","name":"Bash","input":{"command":"pwd"}}]
            emit(assistant)
            visual_tool = True
            emit({"type":"control_request","request_id":"visual-hook","request":{
                "subtype":"hook_callback","callback_id":"devora-judgess","tool_use_id":"visual-tool",
                "input":{"tool_name":"Bash","tool_input":{"command":"pwd"},"cwd":os.getcwd()}}})
        elif text.startswith("history:"):
            # A long transcript in one turn: one assistant message per "History turn <n>".
            for turn in range(int(text[8:])):
                assistant = recorded("assistant")
                assistant["message"]["id"] = f"history-{turn}"
                assistant["message"]["content"] = [{"type":"text","text":f"History turn {turn}"}]
                emit(assistant)
            result()
        elif text.startswith("tools:"):
            # A turn of tool calls, each with its result, then a reply: a transcript that is nearly all system messages.
            for call in range(int(text[6:])):
                assistant = recorded("assistant")
                assistant["message"]["id"] = f"tools-{call}"
                assistant["message"]["content"] = [{"type":"tool_use","id":f"tools-{call}","name":"Read","input":{"file_path":f"file-{call}"}}]
                emit(assistant)
                emit({"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":f"tools-{call}","content":"read"}]}})
            assistant = recorded("assistant")
            assistant["message"]["id"] = "tools-done"
            assistant["message"]["content"] = [{"type":"text","text":"Done with the tools"}]
            emit(assistant)
            result()
        elif text == "wide-code":
            # A code block much wider than any transcript, for scrolling over it.
            lines = "\n".join(f"line {n}: " + "wide " * 80 for n in range(15))
            assistant = recorded("assistant")
            assistant["message"]["id"] = "wide-code"
            assistant["message"]["content"] = [{"type":"text","text":f"Wide code:\n\n```\n{lines}\n```"}]
            emit(assistant)
            result()
        elif text in ("mermaid", "bad-mermaid"):
            # A reply with a Mermaid diagram, which renders, or one whose source doesn't parse.
            diagram = "flowchart LR\n  A[Client] --> B[Server]" if text == "mermaid" else "flowchart LR\n  A[Client --> ((("
            assistant = recorded("assistant")
            assistant["message"]["id"] = text
            assistant["message"]["content"] = [{"type":"text","text":f"A diagram:\n\n```mermaid\n{diagram}\n```"}]
            emit(assistant)
            result()
        elif text.startswith("hook:"):
            emit({"type":"control_request","request_id":"hook","request":{
                "subtype":"hook_callback","callback_id":"devora-judgess","tool_use_id":"tool-1",
                "input":{"tool_name":"Bash","tool_input":{"command":text[5:]},"cwd":os.getcwd()}}})
        elif text == "unknown-request":
            emit({"type":"control_request","request_id":"future","request":{"subtype":"future_request"}})
        elif text == "mcp":
            emit({"type":"control_request","request_id":"mcp","request":{
                "subtype":"mcp_message","server_name":"devora","message":{"jsonrpc":"2.0","id":1,"method":"tools/list"}}})
        elif text.startswith("Devora restarted your session") and os.environ.get("DEVORA_FAKE_NOTICE_UNREQUESTED"):
            # A resume's zero-turn result arriving after the notice was sent, then the notice's turn running on.
            unrequested_result()
            emit(recorded("stream_event"))
            emit({"type":"fake_stream_ready"})
        elif text.startswith("background-work"):
            # Background work as the CLI lists it: "background-work" starts a shell
            # and a subagent, "background-work:shell" only the shell.
            background.append({"task_id":"shell-1","task_type":"local_bash","description":"a watcher"})
            if text != "background-work:shell":
                background.append({"task_id":"agent-1","task_type":"local_agent","description":"a CI wait"})
            background_changed()
            result()
        elif text == "unrequested-result":
            unrequested_result()
            emit(recorded("stream_event"))
            emit({"type":"fake_stream_ready"})
        elif text == "background-shell":
            # A background shell that leads its own session, as Claude Code's do,
            # in a turn that ignores the interrupt, so only a forced stop ends it.
            import subprocess
            shell = subprocess.Popen(["sleep", "60"], start_new_session=True)
            report({"kind":"background","pid":shell.pid})
            ignore_interrupt = True
            emit(recorded("stream_event"))
            emit({"type":"fake_stream_ready"})
        elif text in ("stream", "ignore-interrupt"):
            ignore_interrupt = text == "ignore-interrupt"
            emit(recorded("stream_event"))
            emit({"type":"fake_stream_ready"})
        elif text == "assistant-then-crash":
            emit(recorded("assistant"))
            os._exit(70)
        elif text == "crash":
            os._exit(70)
        elif text == "drift":
            emit({"type":"assistant","message":7})
            emit({"type":"assistant","message":8})
            emit({"type":"future_message","data":9})
            result()
        elif text.startswith("model:"):
            # A whole reply (no stream) from the model named after the colon, as the API names the model that answered.
            reply = recorded("assistant")
            reply["message"]["model"] = text[len("model:"):]
            emit(reply)
            result()
        elif text in ("subagent:mid", "subagent:last"):
            # A subagent's whole message within the turn (the CLI streams none for it): it names the tool call
            # that started the subagent, and reports the subagent's own, small context.
            subagent = recorded("assistant")
            subagent["parent_tool_use_id"] = "toolu_fake_subagent"
            subagent["message"]["usage"] = {"input_tokens": 3, "output_tokens": 1}
            emit(recorded("assistant"))
            emit(subagent)
            if text == "subagent:mid":
                emit(recorded("assistant"))
            result()
        elif text == "malformed":
            print("{not JSON}",flush=True)
        elif text in ("compact", "/compact"):
            emit({"type":"system","subtype":"compact_boundary","session_id":session_id,
                "compact_metadata":{"trigger":"manual","pre_tokens":20000,"post_tokens":2000}})
            result()
        else:
            emit(recorded("stream_event"))
            emit(recorded("assistant"))
            result()
report({"kind":"eof"})
# A CLI that won't exit, for a stop that has to be forced.
if os.environ.get("DEVORA_FAKE_HANG_AT_EOF"):
    time.sleep(3600)
# As the CLI does after an interrupted turn: it exits by itself, with code 1.
sys.exit(int(os.environ.get("DEVORA_FAKE_EOF_EXIT", "0")))
