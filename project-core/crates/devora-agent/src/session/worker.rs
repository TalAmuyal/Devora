use super::{
    judging, Command, PermissionAnswer, Reply, SessionEvent, SessionOptions, StopInfo, UsageSource,
};
use crate::protocol::*;
use devora_app::log::{Level, Log};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, Instant};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_GRACE: Duration = Duration::from_secs(5);
const CALLBACK: &str = "devora-judgess";

#[cfg(panic = "abort")]
compile_error!("agent sessions require panic=unwind so a Judgess panic can abstain");

pub(super) async fn check_version(options: &SessionOptions) -> Result<(), String> {
    let output = timeout(
        REQUEST_TIMEOUT,
        tokio::process::Command::new(&options.program)
            .arg("--version")
            .current_dir(&options.working_directory)
            .env_clear()
            .envs(&options.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "Claude version check timed out")?
    .map_err(|error| format!("couldn't check Claude version: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Claude version check exited with {}",
            output.status
        ));
    }
    crate::version::check_cli_version(&String::from_utf8_lossy(&output.stdout))
}

struct Process {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr: Lines<BufReader<ChildStderr>>,
    stderr_open: bool,
}

impl Process {
    fn spawn(
        options: &SessionOptions,
        id: &str,
        resume: bool,
        model: &str,
        mode: &str,
    ) -> Result<Self, String> {
        let launch = options.launch(id, resume, model, mode)?;
        let mut child = tokio::process::Command::new(&options.program)
            .args(launch.args)
            .env_clear()
            .envs(launch.env)
            .current_dir(&options.working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("couldn't start Claude: {error}"))?;
        Ok(Self {
            stdin: child.stdin.take(),
            stdout: BufReader::new(child.stdout.take().expect("piped stdout")).lines(),
            stderr: BufReader::new(child.stderr.take().expect("piped stderr")).lines(),
            stderr_open: true,
            child,
        })
    }

    async fn send(&mut self, message: Message) -> Result<(), String> {
        let mut line = serde_json::to_vec(&message).map_err(|error| error.to_string())?;
        line.push(b'\n');
        let stdin = self.stdin.as_mut().ok_or("session input is closed")?;
        let result = match timeout(REQUEST_TIMEOUT, stdin.write_all(&line)).await {
            Ok(result) => result.map_err(|error| format!("couldn't write to Claude: {error}")),
            Err(_) => Err("Claude input write timed out".into()),
        };
        if result.is_err() {
            self.stdin.take();
        }
        result
    }

    async fn kill(&mut self) {
        self.stdin.take();
        let _ = self.child.kill().await;
    }
}

struct Pending {
    request: ControlRequest,
    reply: Option<Reply<Value>>,
    deadline: Instant,
}

struct Stopping {
    reply: Option<Reply<StopInfo>>,
    deadline: Instant,
    interrupt_pending: bool,
}

struct Worker {
    options: SessionOptions,
    id: String,
    log: Arc<Log>,
    events: mpsc::UnboundedSender<SessionEvent>,
    ready: Option<oneshot::Sender<Result<(), String>>>,
    initialized: bool,
    ever_ready: bool,
    active_turn: bool,
    has_conversation: bool,
    model: String,
    mode: String,
    sequence: u64,
    controls: HashMap<String, Pending>,
    questions: HashMap<String, PermissionRequest>,
    mcp: HashSet<String>,
    warned: HashSet<String>,
    stopping: Option<Stopping>,
    crash_restarts: usize,
}

fn next_deadline(
    controls: &HashMap<String, Pending>,
    stopping: &Option<Stopping>,
) -> Option<Instant> {
    controls
        .values()
        .map(|pending| pending.deadline)
        .chain(stopping.iter().map(|stop| stop.deadline))
        .min()
}

enum End {
    Eof(Option<i32>, bool),
    ForcedStop,
    Unresponsive,
}

pub(super) async fn run(
    options: SessionOptions,
    id: String,
    log: Arc<Log>,
    mut commands: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<SessionEvent>,
    ready: oneshot::Sender<Result<(), String>>,
) {
    let mut worker = Worker {
        model: options.model.clone(),
        mode: options.permission_mode.clone(),
        has_conversation: options
            .resume
            .as_ref()
            .is_some_and(|resume| resume.has_conversation),
        options,
        id,
        log,
        events,
        ready: Some(ready),
        initialized: false,
        ever_ready: false,
        active_turn: false,
        sequence: 0,
        controls: HashMap::new(),
        questions: HashMap::new(),
        mcp: HashSet::new(),
        warned: HashSet::new(),
        stopping: None,
        crash_restarts: 0,
    };
    loop {
        let resume = worker.has_conversation;
        let mut process = match Process::spawn(
            &worker.options,
            &worker.id,
            resume,
            &worker.model,
            &worker.mode,
        ) {
            Ok(process) => process,
            Err(error) => {
                worker.fail(error);
                return;
            }
        };
        let initialize = ControlRequest::Initialize {
            hooks: Nullable::Value(BTreeMap::from_iter([(
                "PreToolUse".into(),
                vec![HookMatcher {
                    matcher: "^(?!ExitPlanMode$).*".into(),
                    hook_callback_ids: vec![CALLBACK.into()],
                    extra: Map::new(),
                }],
            )])),
            extra: Map::new(),
        };
        let result = match worker.control(&mut process, initialize, None).await {
            Ok(()) => worker.serve(&mut process, &mut commands, resume).await,
            Err(error) => Err(error),
        };
        let result = if matches!(result, Ok(End::Unresponsive)) {
            worker.warn("Claude control request timed out; terminating it".into());
            process.kill().await;
            Ok(End::Eof(None, false))
        } else {
            result
        };
        match result {
            Ok(End::ForcedStop) => {
                process.kill().await;
                worker.finish_stop(false);
                return;
            }
            Ok(End::Eof(code, graceful)) if worker.stopping.is_some() => {
                worker.emit(SessionEvent::Exited {
                    code,
                    restarting: false,
                });
                worker.finish_stop(graceful);
                return;
            }
            Ok(End::Eof(code, _)) => {
                let restart = worker.ever_ready && worker.crash_restarts == 0;
                worker.emit(SessionEvent::Exited {
                    code,
                    restarting: restart,
                });
                worker.clear_pending("Claude exited");
                if !restart {
                    worker.fail("Claude exited and could not be recovered".into());
                    return;
                }
                worker.crash_restarts += 1;
                worker.initialized = false;
                worker.active_turn = false;
            }
            Err(error) => {
                process.kill().await;
                worker.fail(error);
                return;
            }
            Ok(End::Unresponsive) => unreachable!("converted to an exit above"),
        }
    }
}

impl Worker {
    fn emit(&self, event: SessionEvent) {
        let _ = self.events.send(event);
    }

    fn warn(&self, message: String) {
        self.log.write(Level::Warn, &message);
        self.emit(SessionEvent::Warning(message));
    }

    fn clear_pending(&mut self, reason: &str) {
        for (_, pending) in self.controls.drain() {
            if let Some(reply) = pending.reply {
                let _ = reply.send(Err(reason.into()));
            }
        }
        let questions: Vec<_> = self.questions.drain().map(|(id, _)| id).collect();
        for request_id in questions {
            self.emit(SessionEvent::PermissionCancelled { request_id });
        }
        self.mcp.clear();
    }

    fn fail(&mut self, error: String) {
        self.clear_pending(&error);
        if let Some(ready) = self.ready.take() {
            let _ = ready.send(Err(error.clone()));
        }
        if let Some(stopping) = self.stopping.take() {
            if let Some(reply) = stopping.reply {
                let _ = reply.send(Err(error.clone()));
            }
        }
        self.log.report_error(&error);
        self.emit(SessionEvent::Failed(error));
    }

    fn finish_stop(&mut self, graceful: bool) {
        self.clear_pending("session paused");
        if let Some(ready) = self.ready.take() {
            let _ = ready.send(Err("session paused before initialize".into()));
        }
        let info = StopInfo {
            session_id: self.id.clone(),
            has_conversation: self.has_conversation,
            graceful,
        };
        if let Some(stopping) = self.stopping.take() {
            if let Some(reply) = stopping.reply {
                let _ = reply.send(Ok(info.clone()));
            }
        }
        self.emit(SessionEvent::Stopped(info));
    }

    async fn control(
        &mut self,
        process: &mut Process,
        request: ControlRequest,
        reply: Option<Reply<Value>>,
    ) -> Result<(), String> {
        self.sequence += 1;
        let request_id = format!("devora-{}", self.sequence);
        process
            .send(Message::request(request_id.clone(), request.clone()))
            .await?;
        self.controls.insert(
            request_id,
            Pending {
                request,
                reply,
                deadline: Instant::now() + REQUEST_TIMEOUT,
            },
        );
        Ok(())
    }

    fn close_if_stopped(&self, process: &mut Process) {
        if self
            .stopping
            .as_ref()
            .is_some_and(|stop| !stop.interrupt_pending)
            && !self.active_turn
        {
            process.stdin.take();
        }
    }

    async fn serve(
        &mut self,
        process: &mut Process,
        commands: &mut mpsc::UnboundedReceiver<Command>,
        resumed: bool,
    ) -> Result<End, String> {
        let mut commands_open = true;
        loop {
            let deadline = next_deadline(&self.controls, &self.stopping);
            tokio::select! {
                command = commands.recv(), if commands_open => {
                    match command {
                        Some(command) => self.command(process,command).await?,
                        None => {
                            commands_open = false;
                            self.pause(process,None).await?;
                        }
                    }
                }
                line = process.stdout.next_line() => {
                    match line.map_err(|error|format!("Claude stdout failed: {error}"))? {
                        Some(line) => {
                            let message = Message::parse(&line).map_err(|error|format!("invalid Claude protocol JSON: {error}"))?;
                            self.message(process,message,resumed).await?;
                        }
                        None => {
                            process.stdin.take();
                            let status = match timeout(STOP_GRACE,process.child.wait()).await {
                                Ok(status) => status.map_err(|error|format!("couldn't reap Claude: {error}"))?,
                                Err(_) => {
                                    process.kill().await;
                                    return Ok(End::Eof(None,false));
                                }
                            };
                            return Ok(End::Eof(status.code(),status.success()));
                        }
                    }
                }
                line = process.stderr.next_line(), if process.stderr_open => {
                    match line {
                        Ok(Some(line)) => self.warn(format!("Claude stderr: {line}")),
                        Ok(None) => process.stderr_open=false,
                        Err(error) => { process.stderr_open=false; self.warn(format!("Claude stderr failed: {error}")); }
                    }
                }
                _ = tokio::time::sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some() => {
                    if self.stopping.as_ref().is_some_and(|stop|stop.deadline<=Instant::now()) {
                        self.warn("Claude did not stop within the grace period; terminating it".into());
                        return Ok(End::ForcedStop);
                    }
                    return Ok(End::Unresponsive);
                }
            }
            self.close_if_stopped(process);
        }
    }

    async fn pause(
        &mut self,
        process: &mut Process,
        reply: Option<Reply<StopInfo>>,
    ) -> Result<(), String> {
        if self.stopping.is_some() {
            if let Some(reply) = reply {
                let _ = reply.send(Err("session is already pausing".into()));
            }
            return Ok(());
        }
        self.stopping = Some(Stopping {
            reply,
            deadline: Instant::now() + STOP_GRACE,
            interrupt_pending: self.active_turn,
        });
        if self.active_turn {
            self.control(
                process,
                ControlRequest::Interrupt { extra: Map::new() },
                None,
            )
            .await?;
        }
        Ok(())
    }

    async fn command(&mut self, process: &mut Process, command: Command) -> Result<(), String> {
        if let Command::Pause(reply) = command {
            return self.pause(process, reply).await;
        }
        let unavailable = if self.stopping.is_some() {
            Some("session is pausing")
        } else if !self.initialized {
            Some("session is initializing or recovering")
        } else {
            None
        };
        match command {
            Command::Control(request, reply) => {
                if let Some(error) = unavailable {
                    let _ = reply.send(Err(error.into()));
                } else {
                    self.control(process, request, Some(reply)).await?;
                }
            }
            Command::Turn(text, reply) => {
                let result = if let Some(error) = unavailable {
                    Err(error.into())
                } else if self.active_turn {
                    Err("a turn is already running".into())
                } else {
                    process
                        .send(Message::user_text(self.id.clone(), text))
                        .await
                        .map(|()| {
                            self.active_turn = true;
                        })
                };
                let failed = result.is_err() && unavailable.is_none() && !self.active_turn;
                let error = result.as_ref().err().cloned();
                let _ = reply.send(result);
                if failed {
                    return Err(error.unwrap_or_else(|| "session write failed".into()));
                }
            }
            Command::Permission(id, answer, reply) => {
                let result = if let Some(error) = unavailable {
                    Err(error.into())
                } else {
                    self.permission(process, &id, answer).await
                };
                let _ = reply.send(result);
            }
            Command::Mcp(id, response, reply) => {
                let result = if let Some(error) = unavailable {
                    Err(error.into())
                } else if !self.mcp.contains(&id) {
                    Err("MCP request is no longer pending".into())
                } else {
                    process
                        .send(Message::success(
                            id.clone(),
                            json!({"mcp_response": response}),
                        ))
                        .await
                        .map(|()| {
                            self.mcp.remove(&id);
                        })
                };
                let _ = reply.send(result);
            }
            Command::Pause(_) => unreachable!("handled before command dispatch"),
        }
        if process.stdin.is_none() && self.stopping.is_none() {
            return Err("Claude input closed while handling a command".into());
        }
        Ok(())
    }

    async fn permission(
        &mut self,
        process: &mut Process,
        id: &str,
        answer: PermissionAnswer,
    ) -> Result<(), String> {
        let question = self
            .questions
            .get(id)
            .ok_or("permission question is no longer pending")?;
        let answer = match answer {
            PermissionAnswer::Allow => PermissionResponse::Allow {
                updated_input: question.input.clone(),
                extra: Map::new(),
            },
            PermissionAnswer::Deny { message } => PermissionResponse::Deny {
                message,
                extra: Map::new(),
            },
            PermissionAnswer::AllowWithSuggestedRule => {
                let rules = question
                    .permission_suggestions()
                    .filter(|rules| !rules.is_empty())
                    .ok_or("question has no suggested permission rule")?;
                PermissionResponse::Allow {
                    updated_input: question.input.clone(),
                    extra: Map::from_iter([("updatedPermissions".into(), json!(rules))]),
                }
            }
        };
        let response = serde_json::to_value(answer).map_err(|error| error.to_string())?;
        process.send(Message::success(id.into(), response)).await?;
        self.questions.remove(id);
        Ok(())
    }

    async fn response(&mut self, response: &ControlResponse, resumed: bool) -> Result<(), String> {
        let (id, result) = match response {
            ControlResponse::Success {
                request_id,
                response,
                ..
            } => (request_id, Ok(response.clone())),
            ControlResponse::Error {
                request_id, error, ..
            } => (request_id, Err(error.clone())),
        };
        let Some(pending) = self.controls.remove(id) else {
            return Ok(());
        };
        if let Ok(value) = &result {
            match &pending.request {
                ControlRequest::Initialize { .. } => {
                    let capabilities: InitializeResponse = serde_json::from_value(value.clone())
                        .map_err(|error| format!("invalid initialize response: {error}"))?;
                    self.initialized = true;
                    self.ever_ready = true;
                    self.has_conversation |= resumed;
                    self.emit(SessionEvent::Ready {
                        session_id: self.id.clone(),
                        resumed,
                        capabilities,
                    });
                    if let Some(ready) = self.ready.take() {
                        let _ = ready.send(Ok(()));
                    }
                }
                ControlRequest::SetPermissionMode { mode, .. } => self.mode = mode.clone(),
                ControlRequest::SetModel {
                    model: Nullable::Value(model),
                    ..
                } => self.model = model.clone(),
                _ => {}
            }
        } else if matches!(pending.request, ControlRequest::Initialize { .. }) {
            return Err(format!("Claude initialize failed: {}", result.unwrap_err()));
        }
        if pending.reply.is_none() && matches!(pending.request, ControlRequest::Interrupt { .. }) {
            if let Some(stop) = self.stopping.as_mut() {
                stop.interrupt_pending = false;
            }
        }
        if let Some(reply) = pending.reply {
            let _ = reply.send(result);
        }
        Ok(())
    }

    async fn message(
        &mut self,
        process: &mut Process,
        message: Message,
        resumed: bool,
    ) -> Result<(), String> {
        if let Some(kind) = drift_kind(&message) {
            if self.warned.insert(kind.clone()) {
                self.warn(format!(
                    "Claude protocol drift: known kind {kind} decoded as Raw"
                ));
            }
        }
        match &message {
            Message::Known(KnownMessage::ControlResponse {
                response: Extensible::Known(response),
                ..
            }) => self.response(response, resumed).await?,
            Message::Known(KnownMessage::ControlRequest {
                request_id,
                request: Extensible::Known(request),
                ..
            }) => match request {
                ControlRequest::HookCallback {
                    callback_id,
                    input,
                    tool_use_id,
                    ..
                } if callback_id == CALLBACK => {
                    let (verdict, output) = judging::judge(input);
                    let tool_use_id = match tool_use_id {
                        Nullable::Value(id) => Some(id.clone()),
                        Nullable::Null(()) => None,
                    };
                    self.emit(SessionEvent::ToolJudged {
                        tool_use_id,
                        verdict,
                    });
                    process
                        .send(Message::success(request_id.clone(), output))
                        .await?;
                }
                ControlRequest::CanUseTool(question) => {
                    self.questions.insert(request_id.clone(), question.clone());
                    self.emit(SessionEvent::PermissionRequested {
                        request_id: request_id.clone(),
                        question: question.clone(),
                    });
                }
                ControlRequest::McpMessage {
                    server_name,
                    message,
                    ..
                } => {
                    self.mcp.insert(request_id.clone());
                    self.emit(SessionEvent::McpRequested {
                        request_id: request_id.clone(),
                        server_name: server_name.clone(),
                        message: message.clone(),
                    });
                }
                _ => {
                    process
                        .send(Message::Known(KnownMessage::ControlResponse {
                            response: Extensible::Known(ControlResponse::Error {
                                request_id: request_id.clone(),
                                error: "unsupported CLI control request".into(),
                                extra: Map::new(),
                            }),
                            extra: Map::new(),
                        }))
                        .await?;
                }
            },
            Message::Known(KnownMessage::ControlRequest {
                request_id,
                request: Extensible::Raw(_),
                ..
            }) => {
                process
                    .send(Message::Known(KnownMessage::ControlResponse {
                        response: Extensible::Known(ControlResponse::Error {
                            request_id: request_id.clone(),
                            error: "unsupported CLI control request".into(),
                            extra: Map::new(),
                        }),
                        extra: Map::new(),
                    }))
                    .await?;
            }
            Message::Known(KnownMessage::ControlCancelRequest { request_id, .. }) => {
                if self.questions.remove(request_id).is_some() {
                    self.emit(SessionEvent::PermissionCancelled {
                        request_id: request_id.clone(),
                    });
                }
                self.mcp.remove(request_id);
            }
            Message::Known(KnownMessage::Assistant { message, .. }) => {
                self.usage(UsageSource::Assistant, message.extra.get("usage"))
            }
            Message::Known(KnownMessage::StreamEvent {
                event: Extensible::Known(event),
                ..
            }) => match event {
                StreamEvent::MessageStart { message, .. } => {
                    self.usage(UsageSource::MessageStart, message.extra.get("usage"))
                }
                StreamEvent::MessageDelta { usage, .. } => self.usage(
                    UsageSource::MessageDelta,
                    Some(&serde_json::to_value(usage).map_err(|error| error.to_string())?),
                ),
                _ => {}
            },
            Message::Known(KnownMessage::System(System::Init { session_id, .. })) => {
                if session_id != &self.id {
                    return Err("Claude started with a different session identity".into());
                }
            }
            Message::Known(KnownMessage::System(System::CompactBoundary {
                compact_metadata,
                ..
            })) => self.emit(SessionEvent::Compacted(compact_metadata.clone())),
            Message::Known(KnownMessage::Result(result)) => {
                self.active_turn = false;
                self.has_conversation = true;
                if !result.is_error {
                    self.crash_restarts = 0;
                }
                self.usage(
                    UsageSource::Result,
                    Some(&serde_json::to_value(&result.usage).map_err(|error| error.to_string())?),
                );
            }
            _ => {}
        }
        self.emit(SessionEvent::Message(message));
        Ok(())
    }

    fn usage(&self, source: UsageSource, usage: Option<&Value>) {
        if let Some(usage) = usage {
            self.emit(SessionEvent::Usage {
                source,
                usage: usage.clone(),
            });
        }
    }
}

fn drift_kind(message: &Message) -> Option<String> {
    match message {
        Message::Raw(value) => {
            let kind = value.get("type")?.as_str()?;
            matches!(
                kind,
                "user"
                    | "assistant"
                    | "system"
                    | "result"
                    | "stream_event"
                    | "control_request"
                    | "control_response"
                    | "control_cancel_request"
            )
            .then(|| kind.into())
        }
        Message::Known(KnownMessage::ControlRequest {
            request: Extensible::Raw(value),
            ..
        }) => {
            let kind = value.get("subtype")?.as_str()?;
            matches!(
                kind,
                "initialize"
                    | "interrupt"
                    | "set_permission_mode"
                    | "set_model"
                    | "can_use_tool"
                    | "hook_callback"
                    | "mcp_message"
            )
            .then(|| format!("control_request.{kind}"))
        }
        Message::Known(KnownMessage::ControlResponse {
            response: Extensible::Raw(_),
            ..
        }) => Some("control_response".into()),
        Message::Known(KnownMessage::StreamEvent {
            event: Extensible::Raw(_),
            ..
        }) => Some("stream_event".into()),
        Message::Known(KnownMessage::StreamEvent {
            event:
                Extensible::Known(StreamEvent::ContentBlockDelta {
                    delta: Extensible::Raw(_),
                    ..
                }),
            ..
        }) => Some("stream_event.content_block_delta".into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_sessions_have_no_timer_and_stopping_uses_the_earliest_deadline() {
        let mut pending = HashMap::new();
        assert_eq!(next_deadline(&pending, &None), None);
        let now = Instant::now();
        pending.insert(
            "control".into(),
            Pending {
                request: ControlRequest::Interrupt { extra: Map::new() },
                reply: None,
                deadline: now + REQUEST_TIMEOUT,
            },
        );
        assert_eq!(next_deadline(&pending, &None), Some(now + REQUEST_TIMEOUT));
        let stopping = Some(Stopping {
            reply: None,
            deadline: now + STOP_GRACE,
            interrupt_pending: true,
        });
        assert_eq!(next_deadline(&pending, &stopping), Some(now + STOP_GRACE));
    }
}
