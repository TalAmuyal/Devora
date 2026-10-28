//! Spike 1 (TalAmuyal/Devora#205): one persistent `claude -p` process driven over stream-json and the control protocol.
//!
//! Judgess is linked as a library and answers an in-process `PreToolUse` hook callback.
//! Anything Judgess leaves undecided reaches `can_use_tool`, where a stand-in "user" approves after a delay.
//! Every line in both directions is recorded to a JSONL fixture.
//!
//! Usage: claude-protocol-spike <fixture.jsonl> <permission-mode> <prompt>...

use std::io::{BufRead, BufReader, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use judgess::{Request, Verdict, decide};
use serde_json::{Value, json};

/// The env a Devora-launched Claude gets: auth and proxy only, none of the host session's identity.
const KEEP_ENV: &[&str] = &[
    "PATH", "HOME", "USER", "LANG", "TERM", "SHELL", "TMPDIR", "ANTHROPIC_BASE_URL",
    "HTTPS_PROXY", "HTTP_PROXY", "https_proxy", "http_proxy", "NO_PROXY", "no_proxy",
    "GLOBAL_AGENT_NO_PROXY", "NODE_EXTRA_CA_CERTS", "SSL_CERT_FILE",
    "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST", "CLAUDE_CODE_ACCOUNT_UUID",
    "CLAUDE_CODE_ORGANIZATION_UUID", "CLAUDE_CODE_USER_EMAIL", "CLAUDE_CODE_PROXY_RESOLVES_HOSTS",
];

/// Judgess is consulted for every tool but plan approval, as the plugin does today.
const JUDGESS_MATCHER: &str = "^(?!ExitPlanMode$).*";
const JUDGESS_CALLBACK: &str = "judgess";
const PROMPT_DELAY: Duration = Duration::from_secs(2);

struct Recorder {
    started: Instant,
    fixture: Mutex<std::fs::File>,
}

impl Recorder {
    fn log(&self, direction: &str, message: &Value) {
        let t = self.started.elapsed().as_secs_f64() * 1000.0;
        let line = json!({ "t": (t * 100.0).round() / 100.0, "dir": direction, "msg": message });
        let _ = writeln!(self.fixture.lock().unwrap(), "{line}");
    }
}

struct Writer {
    stdin: Mutex<Option<ChildStdin>>,
    recorder: Arc<Recorder>,
}

impl Writer {
    fn send(&self, message: Value) {
        self.recorder.log("out", &message);
        if let Some(stdin) = self.stdin.lock().unwrap().as_mut() {
            let _ = writeln!(stdin, "{message}");
            let _ = stdin.flush();
        }
    }

    /// Closing stdin is how the host ends the session.
    fn close(&self) {
        self.stdin.lock().unwrap().take();
    }

    fn respond(&self, request_id: &str, response: Value) {
        self.send(json!({ "type": "control_response",
            "response": { "subtype": "success", "request_id": request_id, "response": response } }));
    }
}

/// Judgess's verdict as `PreToolUse` hook output. An abstention or a deferral returns nothing, so the CLI's own flow decides.
fn judge(input: &Value) -> Value {
    let Some(request) = Request::from_raw(input) else { return json!({}) };
    let verdict = catch_unwind(AssertUnwindSafe(|| decide(&request)));
    let (decision, reason) = match verdict {
        Ok(Verdict::Approve) => ("allow", "Approved by Judgess".to_string()),
        Ok(Verdict::Deny { rule }) => (
            "deny",
            rule.message().map_or_else(|| format!("Denied by Judgess rule '{}'.", rule.as_str()), str::to_string),
        ),
        _ => return json!({}),
    };
    json!({ "hookSpecificOutput": {
        "hookEventName": "PreToolUse", "permissionDecision": decision, "permissionDecisionReason": reason } })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [fixture, mode, prompts @ ..] = args.as_slice() else {
        eprintln!("usage: claude-protocol-spike <fixture.jsonl> <permission-mode> <prompt>...");
        std::process::exit(2);
    };

    let mut command = Command::new("claude");
    command
        .args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"])
        .args(["--include-partial-messages", "--permission-prompt-tool", "stdio", "--setting-sources", ""])
        .args(["--model", "haiku", "--permission-mode", mode])
        .env_clear()
        .envs(KEEP_ENV.iter().filter_map(|key| std::env::var(key).ok().map(|value| (*key, value))))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Ok(home) = std::env::var("SPIKE_HOME") {
        command.env("HOME", home);
    }
    let mut child = command.spawn().expect("spawn claude");

    let recorder = Arc::new(Recorder {
        started: Instant::now(),
        fixture: Mutex::new(std::fs::File::create(fixture).expect("create fixture")),
    });
    let writer = Arc::new(Writer { stdin: Mutex::new(child.stdin.take()), recorder: recorder.clone() });
    let (results, turn_ended) = mpsc::channel::<Value>();
    let judge_times = Arc::new(Mutex::new(Vec::<Duration>::new()));

    let stdout = child.stdout.take().unwrap();
    let reader = {
        let (writer, recorder, judge_times) = (writer.clone(), recorder.clone(), judge_times.clone());
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
                recorder.log("in", &message);
                match message["type"].as_str() {
                    Some("control_request") => {
                        let id = message["request_id"].as_str().unwrap_or_default().to_string();
                        let request = &message["request"];
                        match request["subtype"].as_str() {
                            Some("hook_callback") if request["callback_id"] == JUDGESS_CALLBACK => {
                                let started = Instant::now();
                                let output = judge(&request["input"]);
                                judge_times.lock().unwrap().push(started.elapsed());
                                writer.respond(&id, output);
                            }
                            Some("can_use_tool") => {
                                // Stand-in for Sapphire's native prompt: a "user" who approves after a pause.
                                let (writer, input) = (writer.clone(), request["input"].clone());
                                thread::spawn(move || {
                                    thread::sleep(PROMPT_DELAY);
                                    writer.respond(&id, json!({ "behavior": "allow", "updatedInput": input }));
                                });
                            }
                            _ => writer.respond(&id, json!({})),
                        }
                    }
                    Some("result") => {
                        let _ = results.send(message);
                    }
                    _ => {}
                }
            }
        })
    };

    writer.send(json!({ "type": "control_request", "request_id": "init", "request": { "subtype": "initialize",
        "hooks": { "PreToolUse": [{ "matcher": JUDGESS_MATCHER, "hookCallbackIds": [JUDGESS_CALLBACK] }] } } }));
    for prompt in prompts {
        writer.send(json!({ "type": "user", "message": { "role": "user", "content": prompt },
            "parent_tool_use_id": null, "session_id": "" }));
        let result = turn_ended.recv_timeout(Duration::from_secs(300)).expect("turn result");
        println!("turn: {} | {}", result["subtype"], result["result"].as_str().unwrap_or_default().replace('\n', " "));
    }

    writer.close();
    let _ = child.wait();
    let _ = reader.join();
    let mut times: Vec<f64> = judge_times.lock().unwrap().iter().map(|d| d.as_secs_f64() * 1e6).collect();
    times.sort_by(f64::total_cmp);
    println!("judgess in-process handling (µs, sorted): {times:.1?}");
}
