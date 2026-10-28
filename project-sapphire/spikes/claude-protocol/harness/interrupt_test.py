import asyncio, json, sys
sys.path.insert(0, '.')
import harness as h

async def main():
    s = h.Session(h.BASE + ["--permission-mode", "manual"], "fixtures/interrupt.jsonl", None)
    await s.start()
    await s.control({"subtype": "initialize", "hooks": None})
    await s.user("Write a 3000-word essay about the history of terminals. Start immediately.")
    deltas = 0
    while deltas < 20:
        m = await s.events.get()
        if m.get("type") == "stream_event" and m["event"].get("type") == "content_block_delta":
            deltas += 1
    t = h.now_ms()
    print("interrupt", await s.control({"subtype": "interrupt"}))
    r = await s.until_result()
    print("result after", round(h.now_ms() - t), "ms:", r.get("subtype"), r.get("stop_reason"), r.get("terminal_reason"), r.get("is_error"))
    await s.user("Reply with just: still alive")
    r = await s.until_result()
    print("next turn:", r.get("result"))
    await s.close()
asyncio.run(main())
