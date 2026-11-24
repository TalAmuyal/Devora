use devora_agent::models::ModelList;
use devora_agent::session::SessionEvent;
use devora_agent_space::{ActiveModel, ChosenEffort, ChosenModel, CompactionProgress, SpaceEvent};
use devora_state::{ChatCommand, ChatEffort, ChatModel, ChatUpdate, PermissionQuestion, Store};

pub(super) fn update(state: &Store, tab: u64, generation: u64, update: ChatUpdate) {
    // Closed tabs and older generations deliberately discard late updates.
    let _ = state.dispatch_chat(ChatCommand::Update {
        tab,
        generation,
        update,
    });
}

pub(super) fn publish(state: &Store, tab: u64, generation: u64, event: SpaceEvent) {
    match event {
        SpaceEvent::Restored(items) => {
            update(state, tab, generation, ChatUpdate::Restore { items })
        }
        SpaceEvent::Transcript(change) => {
            update(state, tab, generation, ChatUpdate::Transcript { change })
        }
        SpaceEvent::Session(event) => session(state, tab, generation, event),
        SpaceEvent::Context(context) => update(
            state,
            tab,
            generation,
            ChatUpdate::Context {
                context: devora_state::ChatContext {
                    used_tokens: context.used_tokens,
                    window_tokens: context.window_tokens,
                    threshold_percent: context.threshold_percent,
                    default_threshold_percent: context.default_threshold_percent,
                    own_threshold_percent: context.own_threshold_percent,
                    compaction: match context.progress {
                        CompactionProgress::Idle => devora_state::CompactionProgress::Idle,
                        CompactionProgress::Requested => {
                            devora_state::CompactionProgress::Requested
                        }
                        CompactionProgress::Underway => devora_state::CompactionProgress::Underway,
                    },
                },
            },
        ),
        // The registry presents it, with the model list its launch was offered (`model`).
        SpaceEvent::Model(_) | SpaceEvent::ChosenModel(_) | SpaceEvent::ChosenEffort(_) => {}
        SpaceEvent::TurnStarted => {
            update(state, tab, generation, ChatUpdate::Turn { running: true })
        }
        SpaceEvent::TurnEnded => {
            update(state, tab, generation, ChatUpdate::Turn { running: false })
        }
    }
}

/// The effort widget's view: the level set in this run (`chosen`), else the one the agent launched with, and the levels
/// the model it's set to takes (the chosen model, else its launch model), from `list`, the models its launch was offered.
pub(super) fn effort(
    chosen: Option<&ChosenEffort>,
    model: &str,
    launch_effort: Option<&str>,
    list: Option<&ModelList>,
) -> ChatEffort {
    let set_to = chosen.map_or(launch_effort.map(str::to_owned), |chosen| {
        chosen.set_to.clone()
    });
    let levels = list
        .and_then(|list| list.effort_levels(model))
        .map(<[String]>::to_vec);
    ChatEffort {
        runs_as: set_to
            .as_deref()
            .zip(levels.as_deref())
            .and_then(|(set, levels)| runs_as(set, levels)),
        set_to,
        levels,
        pending: chosen.is_some_and(|chosen| chosen.pending),
        not_taken: chosen.and_then(|chosen| chosen.not_taken.clone()),
    }
}

/// The level Claude Code runs for `set` on a model that takes `levels` but not `set`: the nearest one below it on Claude
/// Code's scale, as the pinned-CLI suite asserts. None when the model takes `set`, takes no level at all, or takes none
/// below it.
fn runs_as(set: &str, levels: &[String]) -> Option<String> {
    let scale = devora_config::config::CLAUDE_EFFORT_LEVELS;
    let at = scale.iter().position(|level| *level == set)?;
    if levels.is_empty() || levels.iter().any(|level| level == set) {
        return None;
    }
    scale[..at]
        .iter()
        .rev()
        .find(|level| levels.iter().any(|taken| taken == *level))
        .map(|level| (*level).to_owned())
}

/// The model widget's view of `active` for an agent set to `set_to`, named and checked against `list`, the models its
/// launch was offered; a model with no name (or no list) shows its ID as it is, and claims no mismatch.
pub(super) fn model(
    active: &ActiveModel,
    chosen: &ChosenModel,
    launch_model: &str,
    list: Option<&ModelList>,
) -> ChatModel {
    let set_to = chosen.set_to.as_deref().unwrap_or(launch_model);
    let name = |id: &str| {
        list.and_then(|list| list.name_of(id))
            .unwrap_or(id)
            .to_owned()
    };
    let latest = active.latest.as_deref();
    // A model just chosen is what the widget names until the agent has answered on it; it's marked as a mismatch meanwhile.
    let label = if chosen.awaiting_reply {
        let resolved = list.and_then(|list| list.resolve(set_to)).unwrap_or(set_to);
        Some(name(resolved))
    } else {
        latest.map(name)
    };
    ChatModel {
        latest: active.latest.clone(),
        label,
        set_to: set_to.to_owned(),
        set_to_resolved: list
            .and_then(|list| list.resolve(set_to))
            .map(str::to_owned),
        differs: chosen.awaiting_reply
            || chosen.not_taken.is_some()
            || matches!((latest, list), (Some(id), Some(list)) if !list.expects(set_to, id)),
        fallback: active.fallback.clone(),
        awaiting_change: chosen.awaiting_reply,
        change_not_taken: chosen.not_taken.clone(),
    }
}

fn session(state: &Store, tab: u64, generation: u64, event: SessionEvent) {
    match event {
        SessionEvent::Ready { session_id, .. } => {
            update(state, tab, generation, ChatUpdate::Ready { session_id })
        }
        SessionEvent::PermissionRequested {
            request_id,
            question,
        } => {
            let decision_reason = question.decision_reason().map(str::to_owned);
            let suggested_rules_json = question
                .permission_suggestions()
                .map(|rules| serde_json::json!(rules).to_string());
            let can_always_allow = question
                .permission_suggestions()
                .is_some_and(|rules| !rules.is_empty());
            update(
                state,
                tab,
                generation,
                ChatUpdate::PermissionRequested {
                    question: PermissionQuestion {
                        request_id,
                        tool_name: question.tool_name,
                        tool_use_id: question.tool_use_id,
                        input_json: question.input.to_string(),
                        decision_reason,
                        can_always_allow,
                        suggested_rules_json,
                    },
                },
            );
        }
        SessionEvent::PermissionCancelled { request_id } => update(
            state,
            tab,
            generation,
            ChatUpdate::PermissionRemoved { request_id },
        ),
        SessionEvent::Usage { usage, .. } => update(
            state,
            tab,
            generation,
            ChatUpdate::Usage {
                payload_json: usage.to_string(),
            },
        ),
        SessionEvent::Exited { .. } => update(
            state,
            tab,
            generation,
            ChatUpdate::Phase {
                phase: devora_state::ChatPhase::Opening,
            },
        ),
        // These already produced typed, committed transcript changes in the owner.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devora_agent::models::ModelOption;

    fn list(opus: &str) -> ModelList {
        let option = |value: &str, resolved: &str, name: &str| ModelOption {
            value: value.into(),
            resolved_model: Some(resolved.into()),
            display_name: name.into(),
            description: None,
            effort_levels: vec![],
        };
        ModelList {
            cli_version: Some("2.1.326".into()),
            models: vec![
                option("opus", opus, "Opus 5.5"),
                option("sonnet", "claude-sonnet-5-5", "Sonnet 5.5"),
            ],
        }
    }
    fn active(latest: Option<&str>, fallback: Option<&str>) -> ActiveModel {
        ActiveModel {
            latest: latest.map(str::to_owned),
            fallback: fallback.map(str::to_owned),
        }
    }

    #[test]
    fn a_reply_from_the_model_set_shows_its_name_and_no_mismatch() {
        let shown = model(
            &active(Some("claude-opus-5-5"), None),
            &ChosenModel::default(),
            "opus",
            Some(&list("claude-opus-5-5")),
        );
        assert_eq!(shown.label.as_deref(), Some("Opus 5.5"));
        assert_eq!(shown.set_to_resolved.as_deref(), Some("claude-opus-5-5"));
        assert!(!shown.differs);
    }

    #[test]
    fn a_fallback_is_a_mismatch_with_the_clis_sentence() {
        let sentence = "Switched to Sonnet 5.5 due to high demand for Opus 5.5";
        let shown = model(
            &active(Some("claude-sonnet-5-5"), Some(sentence)),
            &ChosenModel::default(),
            "opus",
            Some(&list("claude-opus-5-5")),
        );
        assert_eq!(shown.label.as_deref(), Some("Sonnet 5.5"));
        assert!(shown.differs);
        assert_eq!(shown.fallback.as_deref(), Some(sentence));
    }

    #[test]
    fn without_a_reply_or_a_list_nothing_is_claimed() {
        let pending = model(
            &active(None, None),
            &ChosenModel::default(),
            "opus",
            Some(&list("claude-opus-5-5")),
        );
        assert_eq!(
            (pending.latest, pending.label, pending.differs),
            (None, None, false)
        );
        let unlisted = model(
            &active(Some("claude-sonnet-4-5-20250929"), None),
            &ChosenModel::default(),
            "opus",
            None,
        );
        assert_eq!(
            unlisted.label.as_deref(),
            Some("claude-sonnet-4-5-20250929"),
            "the ID as it is"
        );
        assert!(!unlisted.differs, "no list, no guess");
        assert_eq!(unlisted.set_to_resolved, None);
    }

    fn chose(model: &str, awaiting_reply: bool, not_taken: Option<&str>) -> ChosenModel {
        ChosenModel {
            set_to: Some(model.into()),
            awaiting_reply,
            not_taken: not_taken.map(str::to_owned),
        }
    }

    #[test]
    fn a_model_just_chosen_is_named_and_marked_until_the_agent_answers_on_it() {
        let list = list("claude-opus-5-5");
        let shown = model(
            &active(Some("claude-opus-5-5"), None),
            &chose("sonnet", true, None),
            "opus",
            Some(&list),
        );
        assert_eq!(
            shown.label.as_deref(),
            Some("Sonnet 5.5"),
            "the chosen model, not the last reply's"
        );
        assert_eq!(
            shown.latest.as_deref(),
            Some("claude-opus-5-5"),
            "what last answered stays known"
        );
        assert_eq!(shown.set_to, "sonnet");
        assert!(shown.differs && shown.awaiting_change);
        let answered = model(
            &active(Some("claude-sonnet-5-5"), None),
            &chose("sonnet", false, None),
            "opus",
            Some(&list),
        );
        assert_eq!(answered.label.as_deref(), Some("Sonnet 5.5"));
        assert!(
            !answered.differs && !answered.awaiting_change,
            "it answered on the chosen model"
        );
    }

    #[test]
    fn after_the_first_reply_a_fallback_shows_as_an_ordinary_mismatch() {
        let shown = model(
            &active(Some("claude-opus-5-5"), Some("Switched to Opus 5.5")),
            &chose("sonnet", false, None),
            "opus",
            Some(&list("claude-opus-5-5")),
        );
        assert_eq!(
            shown.label.as_deref(),
            Some("Opus 5.5"),
            "the model that answered"
        );
        assert!(shown.differs && !shown.awaiting_change);
    }

    #[test]
    fn a_change_the_session_didnt_take_shows_as_a_mismatch_with_why() {
        let shown = model(
            &active(Some("claude-opus-5-5"), None),
            &chose("sonnet", false, Some("set_model failed")),
            "opus",
            Some(&list("claude-opus-5-5")),
        );
        assert!(shown.differs);
        assert_eq!(shown.change_not_taken.as_deref(), Some("set_model failed"));
        assert_eq!(
            shown.label.as_deref(),
            Some("Opus 5.5"),
            "the model still answering, not the refused one"
        );
        assert!(!shown.awaiting_change, "a refused model never replies");
        assert_eq!(shown.set_to, "sonnet", "the refused pick stays stored");
    }

    #[test]
    fn the_effort_shown_is_the_one_set_this_run_else_the_launchs_with_the_models_levels() {
        let mut list = list("claude-opus-5-5");
        list.models[0].effort_levels = vec!["low".into(), "high".into()];
        let launched = effort(None, "opus", Some("high"), Some(&list));
        assert_eq!(
            (
                launched.set_to.as_deref(),
                launched.levels.clone(),
                launched.pending
            ),
            (
                Some("high"),
                Some(vec!["low".to_owned(), "high".to_owned()]),
                false
            )
        );
        let pending = ChosenEffort {
            set_to: None,
            pending: true,
            not_taken: None,
        };
        let changed = effort(Some(&pending), "opus", Some("high"), Some(&list));
        assert_eq!(
            (changed.set_to, changed.pending),
            (None, true),
            "Claude Code default, set during a turn"
        );
        let unknown = effort(None, "opus", Some("high"), None);
        assert_eq!(unknown.levels, None, "no list, no levels claimed");
    }

    #[test]
    fn a_level_the_model_lacks_runs_as_the_nearest_one_below_it_takes() {
        let levels = |levels: &[&str]| levels.iter().map(|l| l.to_string()).collect::<Vec<_>>();
        let sonnet_4_6 = levels(&["low", "medium", "high", "max"]);
        assert_eq!(runs_as("xhigh", &sonnet_4_6).as_deref(), Some("high"));
        assert_eq!(runs_as("high", &sonnet_4_6), None, "it takes it");
        assert_eq!(
            runs_as("high", &[]),
            None,
            "a model that takes no level runs none"
        );
        assert_eq!(
            runs_as("low", &levels(&["high", "max"])),
            None,
            "nothing below it to name"
        );
        assert_eq!(
            runs_as("turbo", &sonnet_4_6),
            None,
            "not a level on the scale"
        );
    }
}
