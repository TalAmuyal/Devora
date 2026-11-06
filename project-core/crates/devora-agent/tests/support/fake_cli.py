#!/usr/bin/env python3
"""Deterministic subprocess peer using the recorded Claude wire contract."""
import copy
import json
import os
import pathlib
import sys

if "--version" in sys.argv:
    print(os.environ.get("DEVORA_FAKE_VERSION", "2.1.316 (Claude Code)"))
    sys.exit(0)

args = sys.argv[1:]
def argument(name):
    return args[args.index(name) + 1]

session_id = argument("--resume" if "--resume" in args else "--session-id")
root = pathlib.Path(os.environ["DEVORA_FAKE_FIXTURES"])
recording = [json.loads(line) for line in (root / "basic.jsonl").read_text().splitlines()]
def recorded(kind):
    return copy.deepcopy(next(row["msg"] for row in recording
        if isinstance(row.get("msg"), dict) and row["msg"].get("type") == kind))

def report(value):
    with open(os.environ["DEVORA_FAKE_REPORT"], "a") as stream:
        stream.write(json.dumps(value) + "\n")

def emit(value):
    if isinstance(value, dict) and "session_id" in value:
        value["session_id"] = session_id
    print(json.dumps(value), flush=True)

def result(interrupted=False):
    value = recorded("result")
    if interrupted:
        value.update(subtype="error_during_execution", is_error=True, terminal_reason="aborted_streaming")
    emit(value)

def question():
    emit({"type":"control_request","request_id":"question","request":{
        "subtype":"can_use_tool","tool_name":"Bash","input":{"command":"custom-tool"},
        "tool_use_id":"tool-1","decision_reason":"Ask rule",
        "permission_suggestions":[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"custom-tool"}],
            "behavior":"allow","destination":"localSettings"}]}})

ignore_interrupt = False
visual_tool = False
report({"kind":"start","args":args,"environment":dict(os.environ),"cwd":os.getcwd()})
for line in sys.stdin:
    message = json.loads(line)
    kind = message["type"]
    if kind == "control_request":
        request = message["request"]
        subtype = request["subtype"]
        report({"kind":subtype})
        if subtype == "interrupt" and ignore_interrupt:
            continue
        if subtype == "set_model" and request.get("model") == "hang":
            emit({"type":"fake_control_waiting"})
            continue
        if subtype == "initialize":
            if os.environ.get("DEVORA_FAKE_INIT_ERROR"):
                emit({"type":"control_response","response":{"subtype":"error","request_id":message["request_id"],"error":"initialize refused"}})
                continue
            response = recorded("control_response")
            response["response"]["request_id"] = message["request_id"]
            emit(response)
            emit(recorded("system"))
            if os.environ.get("DEVORA_FAKE_CRASH_UNUSED"):
                marker = pathlib.Path(os.environ["DEVORA_FAKE_CRASH_UNUSED"])
                if not marker.exists():
                    marker.touch()
                    os._exit(70)
        else:
            emit({"type":"control_response","response":{"subtype":"success","request_id":message["request_id"],
                "response":{"subtype":subtype,"mode":request.get("mode"),"model":request.get("model")}}})
            if subtype == "interrupt":
                result(True)
    elif kind == "control_response":
        if visual_tool:
            emit({"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"visual-tool","content":"Visual tool result"}]}})
            visual_tool = False
        emit({"type":"fake_answer","answer":message["response"].get("response", {"error":message["response"].get("error")})})
        result()
    elif kind == "user":
        report({"kind":"user","message":message["message"]})
        text = message["message"]["content"]
        if text == "permission":
            question()
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
        elif text.startswith("hook:"):
            emit({"type":"control_request","request_id":"hook","request":{
                "subtype":"hook_callback","callback_id":"devora-judgess","tool_use_id":"tool-1",
                "input":{"tool_name":"Bash","tool_input":{"command":text[5:]},"cwd":os.getcwd()}}})
        elif text == "unknown-request":
            emit({"type":"control_request","request_id":"future","request":{"subtype":"future_request"}})
        elif text == "mcp":
            emit({"type":"control_request","request_id":"mcp","request":{
                "subtype":"mcp_message","server_name":"devora","message":{"jsonrpc":"2.0","id":1,"method":"tools/list"}}})
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
