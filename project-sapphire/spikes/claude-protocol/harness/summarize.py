import json, sys
# Print the permission-relevant flow of a fixture: tool uses, hook events, control requests/responses, tool results.
for l in open(sys.argv[1]):
    e = json.loads(l); m = e['msg']; t = e['t']; d = e['dir']
    if not isinstance(m, dict):
        if d == 'stderr' and m.strip(): print(t, 'STDERR', m[:200])
        continue
    ty = m.get('type'); st = m.get('subtype')
    if ty == 'control_request':
        r = m['request']
        if r.get('subtype') == 'initialize': continue
        print(t, d, 'CTRL_REQ', r.get('subtype'), json.dumps({k: r[k] for k in r if k in ('tool_name','input','callback_id','decision_reason','permission_suggestions','mode')})[:300])
        if r.get('subtype') == 'hook_callback': print('     hook input:', json.dumps(r.get('input'))[:300])
    elif ty == 'control_response' and d == 'out':
        print(t, d, 'CTRL_RESP', json.dumps(m['response'].get('response'))[:200])
    elif ty == 'assistant':
        for c in m['message'].get('content', []):
            if c.get('type') == 'tool_use': print(t, 'TOOL_USE', c['name'], json.dumps(c['input'])[:150])
    elif ty == 'user' and d == 'in':
        cs = m['message'].get('content')
        if isinstance(cs, list):
            for c in cs:
                if c.get('type') == 'tool_result':
                    txt = c.get('content'); txt = txt if isinstance(txt, str) else json.dumps(txt)
                    print(t, 'TOOL_RESULT', 'ERR' if c.get('is_error') else 'ok', txt[:160].replace('\n',' | '))
    elif ty == 'system' and st and st.startswith('hook'):
        print(t, 'SYS', st, json.dumps({k: m.get(k) for k in ('hook_name','hook_event','outcome','exit_code','output','stdout')})[:250])
    elif ty == 'system' and st not in ('init','status'):
        print(t, 'SYS', st, json.dumps(m)[:250])
    elif ty == 'result':
        print(t, 'RESULT', st, 'denials=', json.dumps(m.get('permission_denials'))[:300])
