import asyncio, json, sys
sys.path.insert(0, '.')
import harness as h

async def main(mode):
    settings = json.dumps({"permissions": {"deny": ["Bash(git log:*)"], "ask": ["Bash(ls:*)"]}})
    s = h.Session(h.BASE + ["--permission-mode", mode, "--permission-prompt-tool", "stdio", "--settings", settings],
                  f"fixtures/settings-deny-{mode}.jsonl", "callback")
    await s.start()
    await s.control({"subtype": "initialize", "hooks": {"PreToolUse": [{"matcher": "Bash", "hookCallbackIds": ["judgess_pre"]}]}})
    await s.user("Use the Bash tool to run exactly these two commands, one call each, nothing else: (1) `git log --oneline -3` (2) `ls -la`. If one is denied, move on. Then say done.")
    print(mode, (await s.until_result()).get("result", "")[:200])
    await s.close()
asyncio.run(main(sys.argv[1]))
