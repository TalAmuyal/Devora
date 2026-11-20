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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NoticeCause {
    #[default]
    Restart,
    /// A pause that had to stop the agent: forced, or interrupting a turn.
    Stop,
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
            Some(Self {
                attempt: Some(_),
                cause,
                ..
            }) => Some(Self {
                attempt: None,
                recovery: true,
                cause,
            }),
            other => other,
        };
        carried.or(restarted.then_some(Self {
            attempt: None,
            recovery: false,
            cause: NoticeCause::Restart,
        }))
    }

    /// What a stop that was forced or interrupted a turn owes.
    pub fn after_stop() -> Self {
        Self {
            attempt: None,
            recovery: false,
            cause: NoticeCause::Stop,
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
        };
        if self.recovery {
            format!("{notice} This may repeat a notice you already got; if so, there's nothing more to do.")
        } else {
            notice.into()
        }
    }

    /// The transcript row, which shows why the agent woke.
    pub fn row(&self) -> &'static str {
        match (self.cause, self.recovery) {
            (NoticeCause::Restart, false) => {
                "Devora restarted: asked the agent to re-check its waits"
            }
            (NoticeCause::Restart, true) => {
                "Devora restarted: asked the agent to re-check its waits (resent)"
            }
            (NoticeCause::Stop, false) => {
                "Devora stopped the agent to pause it: asked it to re-check its waits"
            }
            (NoticeCause::Stop, true) => {
                "Devora stopped the agent to pause it: asked it to re-check its waits (resent)"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NoticeCause, RestartNotice};

    fn owed(recovery: bool) -> Option<RestartNotice> {
        Some(RestartNotice {
            attempt: None,
            recovery,
            cause: NoticeCause::Restart,
        })
    }

    fn attempted() -> Option<RestartNotice> {
        Some(RestartNotice {
            attempt: Some("turn".into()),
            recovery: false,
            cause: NoticeCause::Restart,
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
