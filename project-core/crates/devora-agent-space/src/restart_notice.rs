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

/// The turn telling an agent that Devora restarted its session, so the waits it
/// had (watchers, builds, CI waits, background shells and subagents) may have
/// stopped and their notifications won't arrive.
///
/// Its lifecycle lives in the space's database (`space.restart_notice`, with
/// `space.launched_run`):
/// - **Owed:** the first launch in a new app run of an agent with a conversation
///   owes one. A launch in the same run (a resume, a crash-restart) owes none,
///   and a notice still owed from an earlier run covers the new one too.
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
                attempt: Some(_), ..
            }) => Some(Self {
                attempt: None,
                recovery: true,
            }),
            other => other,
        };
        carried.or(restarted.then_some(Self {
            attempt: None,
            recovery: false,
        }))
    }

    /// What the agent is told.
    pub fn prompt(&self) -> String {
        let notice = "Devora restarted your session. Background work from before the restart \
            (watchers, builds, CI waits, background shells and subagents) may have been stopped, \
            and notifications about it won't arrive. Check what you were waiting on and \
            re-establish whatever is gone. Don't start a duplicate of anything still running.";
        if self.recovery {
            format!("{notice} This may repeat a notice you already got; if so, there's nothing more to do.")
        } else {
            notice.into()
        }
    }

    /// The transcript row, which shows why the agent woke.
    pub fn row(&self) -> &'static str {
        if self.recovery {
            "Devora restarted: asked the agent to re-check its waits (resent)"
        } else {
            "Devora restarted: asked the agent to re-check its waits"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RestartNotice;

    fn owed(recovery: bool) -> Option<RestartNotice> {
        Some(RestartNotice {
            attempt: None,
            recovery,
        })
    }

    fn attempted() -> Option<RestartNotice> {
        Some(RestartNotice {
            attempt: Some("turn".into()),
            recovery: false,
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
