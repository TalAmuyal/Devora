import asyncio, json, sys
sys.path.insert(0, '.')
import harness as h

class AskSession(h.Session):
    async def _answer(self, msg):
        req = msg["request"]
        if req.get("subtype") == "hook_callback":
            cmd = ((req.get("input") or {}).get("tool_input") or {}).get("command", "")
            body = {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "ask",
                    "permissionDecisionReason": "Judgess defers this to the user"}} if cmd.startswith("touch") else {}
            await self.write({"type": "control_response", "response": {"subtype": "success", "request_id": msg["request_id"], "response": body}})
        else:
            await super()._answer(msg)

async def main():
    s = AskSession(h.BASE + ["--permission-mode", "auto", "--permission-prompt-tool", "stdio"], "fixtures/ask-auto.jsonl", None)
    await s.start()
    await s.control({"subtype": "initialize", "hooks": {"PreToolUse": [{"matcher": "Bash", "hookCallbackIds": ["ask"]}]}})
    await s.user("Use the Bash tool to run exactly `touch scratch-file.txt` and nothing else, then say done.")
    print((await s.until_result()).get("result"))
    await s.close()
asyncio.run(main())
