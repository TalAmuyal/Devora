use serde::{Deserialize, Serialize};

/// One app process's identity. An agent whose CLI last ran in another run was
/// stopped by a restart, which background work and its notifications don't
/// survive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppRun(String);

impl AppRun {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub(crate) fn id(&self) -> &str {
        &self.0
    }
}

impl Default for AppRun {
    fn default() -> Self {
        Self::new()
    }
}

/// The turn telling an agent that Devora restarted its session, or had to stop
/// it to pause it, so the waits it had (watchers, builds, CI waits, background
/// shells and subagents) may have stopped and their notifications won't arrive.
///
/// Its lifecycle lives in the space's database (`space.restart_notice`, with
/// `space.launched_run`):
/// - **Owed:** the first launch in a new app run of an agent with a conversation
///   owes one. So does a stop that was forced or interrupted a turn, since it
///   ended the agent's background work too, while a clean, idle stop owes none.
///   A pause that stopped the agent's background tasks owes one naming them
///   (`stopped`). Owing again merges into the one stored (`merge`): a pause's
///   named tasks win over a forced or interrupted stop, which wins over a
///   restart, and every stopped task is kept.
///   A launch in the same run (a resume, a crash-restart) owes none of its own,
///   and a notice still owed covers whatever comes after it.
/// - **Attempt:** the notice's transcript row and the ID it is sent with commit
///   together, before the send (`attempt`).
/// - **Delivered:** the result naming that ID clears the notice. If the process
///   ends first, the next launch reads Claude's saved session, the only evidence
///   that the CLI accepted it: an entry with that uuid means it was delivered.
/// - **Recovery:** an attempt missing from the saved session is owed again,
///   marked as a recovery, whose wording allows that it may repeat one already
///   seen. That includes a crash between the commit and the send, after which
///   the agent never got the first one.
///
/// So delivery is at least once, by the agent's next launch. A CLI that
/// crashes during the notice's turn and restarts within the same run doesn't
/// resend it: if Claude saved it, the resumed model still sees it, and if not,
/// the next launch sends it. Global and individual pauses only defer it: the
/// notice goes out when the agent next launches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RestartNotice {
    pub attempt: Option<String>,
    pub recovery: bool,
    /// Why it's owed, for its wording. A notice stored before stops owed one
    /// was a restart's.
    #[serde(default)]
    pub cause: NoticeCause,
    /// The background work a pause stopped, for the notice to name.
    #[serde(default)]
    pub stopped: Vec<StoppedTask>,
}

/// Why a notice is owed, from the least to the most specific. When two apply,
/// one notice is kept and the more specific cause words it: a pause that
/// stopped named tasks says exactly what to re-establish, a forced or
/// interrupted stop says the work is gone, and a restart only that it may be.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum NoticeCause {
    #[default]
    Restart,
    /// A pause that had to stop the agent: forced, or interrupting a turn.
    Stop,
    /// A pause that stopped the agent's background tasks (`stopped`) first.
    Pause,
}

/// A background task a pause stopped: a shell (`local_bash`) or a subagent
/// (`local_agent`), with the description the agent gave it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StoppedTask {
    pub task_type: String,
    pub description: String,
}

impl RestartNotice {
    /// What a launch owes, from the stored notice, whether Claude's saved
    /// session holds its attempt, whether Devora restarted since the agent's
    /// last launch, and whether the agent has a conversation to resume.
    pub fn at_launch(
        stored: Option<Self>,
        attempt_saved: bool,
        restarted: bool,
        has_conversation: bool,
    ) -> Option<Self> {
        if !has_conversation {
            return None;
        }
        let carried = match stored {
            Some(Self {
                attempt: Some(_), ..
            }) if attempt_saved => None,
            Some(
                notice @ Self {
                    attempt: Some(_), ..
                },
            ) => Some(Self {
                attempt: None,
                recovery: true,
                ..notice
            }),
            other => other,
        };
        carried.or(restarted.then_some(Self {
            attempt: None,
            recovery: false,
            cause: NoticeCause::Restart,
            stopped: vec![],
        }))
    }

    /// What a stop that was forced or interrupted a turn owes.
    pub fn after_stop() -> Self {
        Self {
            attempt: None,
            recovery: false,
            cause: NoticeCause::Stop,
            stopped: vec![],
        }
    }

    /// What a pause that stopped `stopped` owes.
    pub fn after_pause(stopped: Vec<StoppedTask>) -> Self {
        Self {
            attempt: None,
            recovery: false,
            cause: NoticeCause::Pause,
            stopped,
        }
    }

    /// One notice for both: the more specific cause, every stopped task, and
    /// whatever attempt the stored one (`self`) has in flight.
    pub fn merge(self, other: Self) -> Self {
        let mut stopped = self.stopped;
        for task in other.stopped {
            if !stopped.contains(&task) {
                stopped.push(task);
            }
        }
        Self {
            cause: self.cause.max(other.cause),
            stopped,
            ..self
        }
    }

    /// What the agent is told.
    pub fn prompt(&self) -> String {
        let notice = match self.cause {
            NoticeCause::Restart => "Devora restarted your session. Background work from before the restart \
                (watchers, builds, CI waits, background shells and subagents) may have been stopped, \
                and notifications about it won't arrive. Check what you were waiting on and \
                re-establish whatever is gone. Don't start a duplicate of anything still running.",
            NoticeCause::Stop => "Devora had to stop your session to pause it, so your background work \
                (watchers, builds, CI waits, background shells and subagents) was stopped, and \
                notifications about it won't arrive. Check what you were waiting on and \
                re-establish whatever you still need.",
            NoticeCause::Pause => &format!(
                "Devora paused you and stopped your background work: {}. Notifications about it \
                 won't arrive. Re-establish what you still need.",
                self.stopped_list()
            ),
        };
        if self.recovery {
            format!("{notice} This may repeat a notice you already got; if so, there's nothing more to do.")
        } else {
            notice.into()
        }
    }

    /// The transcript row, which shows why the agent woke.
    pub fn row(&self) -> String {
        let row = match self.cause {
            NoticeCause::Restart => {
                "Devora restarted: asked the agent to re-check its waits".into()
            }
            NoticeCause::Stop => {
                "Devora stopped the agent to pause it: asked it to re-check its waits".into()
            }
            NoticeCause::Pause => format!(
                "Devora paused the agent and stopped {}: asked it to re-establish what it needs",
                self.stopped_list()
            ),
        };
        if self.recovery {
            format!("{row} (resent)")
        } else {
            row
        }
    }

    /// "a watcher (shell), a CI wait (subagent)".
    fn stopped_list(&self) -> String {
        self.stopped
            .iter()
            .map(|task| match task.task_type.as_str() {
                "local_bash" => format!("{} (shell)", task.description),
                "local_agent" => format!("{} (subagent)", task.description),
                other => format!("{} ({other})", task.description),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::{NoticeCause, RestartNotice, StoppedTask};

    fn owed(recovery: bool) -> Option<RestartNotice> {
        Some(RestartNotice {
            attempt: None,
            recovery,
            cause: NoticeCause::Restart,
            stopped: vec![],
        })
    }

    fn attempted() -> Option<RestartNotice> {
        Some(RestartNotice {
            attempt: Some("turn".into()),
            recovery: false,
            cause: NoticeCause::Restart,
            stopped: vec![],
        })
    }

    #[test]
    fn a_restart_owes_a_notice_only_to_an_agent_with_a_conversation() {
        assert_eq!(
            RestartNotice::at_launch(None, false, true, true),
            owed(false)
        );
        assert_eq!(RestartNotice::at_launch(None, false, true, false), None);
        assert_eq!(RestartNotice::at_launch(None, false, false, true), None);
    }

    #[test]
    fn a_notice_not_yet_sent_stays_owed_and_covers_a_second_restart() {
        assert_eq!(
            RestartNotice::at_launch(owed(false), false, false, true),
            owed(false)
        );
        assert_eq!(
            RestartNotice::at_launch(owed(false), false, true, true),
            owed(false)
        );
        assert_eq!(
            RestartNotice::at_launch(owed(true), false, true, true),
            owed(true)
        );
    }

    #[test]
    fn a_stop_notice_keeps_its_cause_through_a_recovery_and_a_restart() {
        let stopped = RestartNotice {
            attempt: Some("turn".into()),
            ..RestartNotice::after_stop()
        };
        let resent = RestartNotice::at_launch(Some(stopped), false, true, true).unwrap();
        assert_eq!(resent.cause, NoticeCause::Stop);
        assert!(resent.recovery);
        assert!(resent.row().contains("(resent)"));
        assert!(resent.prompt().contains("to pause it"));
    }

    fn watcher() -> StoppedTask {
        StoppedTask {
            task_type: "local_bash".into(),
            description: "a watcher".into(),
        }
    }

    #[test]
    fn a_pause_notice_names_what_it_stopped() {
        let ci = StoppedTask {
            task_type: "local_agent".into(),
            description: "a CI wait".into(),
        };
        let notice = RestartNotice::after_pause(vec![watcher(), ci]);
        assert!(notice
            .prompt()
            .contains("stopped your background work: a watcher (shell), a CI wait (subagent)."));
        assert_eq!(
            notice.row(),
            "Devora paused the agent and stopped a watcher (shell), a CI wait (subagent): asked it to re-establish what it needs"
        );
    }

    // One stored notice: the more specific cause words it, every stopped task
    // is kept, and the stored one's attempt in flight stays.
    #[test]
    fn notices_owed_together_merge_into_the_most_specific() {
        let restart = owed(false).unwrap();
        let pause = RestartNotice::after_pause(vec![watcher()]);
        for (first, second) in [
            (restart.clone(), pause.clone()),
            (pause.clone(), restart.clone()),
            (pause.clone(), RestartNotice::after_stop()),
            (RestartNotice::after_stop(), pause.clone()),
        ] {
            let merged = first.merge(second);
            assert_eq!(merged.cause, NoticeCause::Pause);
            assert_eq!(merged.stopped, vec![watcher()]);
        }
        assert_eq!(
            restart.clone().merge(RestartNotice::after_stop()).cause,
            NoticeCause::Stop
        );
        let attempted = RestartNotice {
            attempt: Some("turn".into()),
            ..restart
        };
        let merged = attempted.merge(RestartNotice::after_pause(vec![watcher()]));
        assert_eq!(merged.attempt.as_deref(), Some("turn"));
        assert_eq!(
            merged
                .clone()
                .merge(RestartNotice::after_pause(vec![watcher()]))
                .stopped,
            vec![watcher()],
            "a task stopped twice is named once"
        );
    }

    #[test]
    fn a_notice_stored_before_causes_existed_is_a_restarts() {
        let stored: RestartNotice =
            serde_json::from_str(r#"{"attempt":null,"recovery":false}"#).unwrap();
        assert_eq!(stored.cause, NoticeCause::Restart);
    }

    #[test]
    fn an_attempt_in_the_saved_session_was_delivered_and_one_missing_from_it_is_resent() {
        assert_eq!(
            RestartNotice::at_launch(attempted(), true, false, true),
            None
        );
        assert_eq!(
            RestartNotice::at_launch(attempted(), true, true, true),
            owed(false),
            "a delivered notice doesn't cover a later restart"
        );
        assert_eq!(
            RestartNotice::at_launch(attempted(), false, false, true),
            owed(true)
        );
        assert_eq!(
            RestartNotice::at_launch(attempted(), false, true, true),
            owed(true)
        );
    }
}
