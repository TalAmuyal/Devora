//! The named commands, the events they emit, and the reducer that applies one to the other.

use crate::model::{AppState, Pane, PreviewPane, Session, SessionKind, Tab, TabId, Window};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A named change to the state: the only way a UI changes it.
///
/// The same commands back keybindings and, later, the `:` command line (ADR-008).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    SetGlobalPaused {
        paused: bool,
    },
    SetAgentKeepPaused {
        profile_path: String,
        name: String,
        keep_paused: bool,
    },
    ClearAgentDraft {
        profile_path: String,
        name: String,
        text: String,
    },
    SetAgentDraft {
        profile_path: String,
        name: String,
        text: String,
    },
    SetAgentScroll {
        profile_path: String,
        name: String,
        anchor: Option<crate::ChatScrollAnchor>,
    },
    ClearAgentView {
        profile_path: String,
        name: String,
    },
    /// Opens a tab at the end of the tab bar and activates it.
    OpenSession {
        title: String,
        kind: SessionKind,
        profile_path: Option<String>,
    },
    /// Closes a tab; when it was active, the tab now at its position (or the new last one) becomes active.
    CloseSession {
        tab: TabId,
    },
    ActivateTab {
        tab: TabId,
    },
    /// Activates the tab after the active one, wrapping around to the first.
    ActivateNext,
    /// Activates the tab before the active one, wrapping around to the last.
    ActivatePrevious,
    /// Moves the active tab one place towards the start of the tab bar; it doesn't wrap, so the first tab stays put.
    MoveActiveTabBackward,
    /// Moves the active tab one place towards the end of the tab bar; it doesn't wrap, so the last tab stays put.
    MoveActiveTabForward,
    /// Moves a tab to `index` in the tab bar, shifting the tabs in between.
    MoveTab {
        tab: TabId,
        index: u32,
    },
    SetTitle {
        tab: TabId,
        title: String,
    },
    /// A [`SessionKind::CreatingWorkspace`] tab's workspace is ready at `path`.
    WorkspaceCreated {
        tab: TabId,
        path: String,
    },
    /// A [`SessionKind::CreatingWorkspace`] tab's creation failed.
    WorkspaceCreationFailed {
        tab: TabId,
        reason: String,
    },
    /// Open/replace a preview for this terminal, or refresh the same file without changing its pane ID.
    OpenPreview {
        terminal: u64,
        path: String,
    },
    /// Add a pane, or refresh an already-open path in place without disturbing its siblings.
    StackPreview {
        terminal: u64,
        path: String,
    },
    ResizePreview {
        tab: TabId,
        pane: u64,
        width: u32,
    },
    ClosePreview {
        tab: TabId,
        pane: u64,
    },
    SetActiveProfile {
        profile_path: Option<String>,
    },
}

/// What a command changed, for the UIs to fold into what they render.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    GlobalPauseChanged {
        paused: bool,
    },
    AgentDraftCleared {
        profile_path: String,
        name: String,
        text: String,
    },
    AgentViewChanged {
        profile_path: String,
        name: String,
        view: Option<crate::AgentViewState>,
    },
    /// The whole state; always the first event of a [`crate::Subscription`], and never emitted by a command.
    Snapshot {
        state: AppState,
    },
    SessionOpened {
        tab: Tab,
        index: u32,
    },
    SessionClosed {
        tab: TabId,
    },
    /// The active tab changed; `None` once the last tab is closed.
    TabActivated {
        tab: Option<TabId>,
    },
    TabMoved {
        tab: TabId,
        from: u32,
        to: u32,
    },
    TitleChanged {
        tab: TabId,
        title: String,
    },
    /// A tab's session changed kind (its workspace was created, or creation failed).
    SessionChanged {
        tab: TabId,
        session: Session,
    },
    PanesChanged {
        tab: TabId,
        pane: Pane,
    },
    ActiveProfileChanged {
        profile_path: Option<String>,
    },
}

/// Why a command was rejected; a rejected command changes nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateError {
    GlobalPauseRequired,
    InvalidPreviewWidth {
        width: u32,
    },
    UnknownTerminal {
        terminal: u64,
    },
    UnknownPreview {
        pane: u64,
    },
    UnknownTab {
        tab: TabId,
    },
    IndexOutOfRange {
        index: u32,
        len: u32,
    },
    /// A creation outcome was reported for a tab that isn't creating a workspace.
    NotCreating {
        tab: TabId,
    },
    /// The state database couldn't be opened, read or written; a command that failed to save takes no effect.
    Storage {
        message: String,
    },
    /// The state database is held by another process: another instance of the same app, since apps never share one (ADR-008).
    InUse {
        message: String,
    },
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GlobalPauseRequired => f.write_str("Keep paused requires an active global hold"),
            Self::InvalidPreviewWidth { width } => write!(
                f,
                "preview width {width} is outside {}..={}",
                crate::MIN_PREVIEW_WIDTH,
                crate::MAX_PREVIEW_WIDTH
            ),
            Self::UnknownTerminal { terminal } => write!(f, "no terminal pane with id {terminal}"),
            Self::UnknownPreview { pane } => write!(f, "no preview pane with id {pane}"),
            Self::UnknownTab { tab } => write!(f, "no tab with id {tab}"),
            Self::IndexOutOfRange { index, len } => {
                write!(f, "tab index {index} is out of range for {len} tabs")
            }
            Self::NotCreating { tab } => write!(f, "tab {tab} is not creating a workspace"),
            Self::Storage { message } | Self::InUse { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for StateError {}

fn agent_view<'a>(
    app: &'a mut AppState,
    profile: &str,
    name: &str,
) -> &'a mut crate::AgentViewState {
    let index = app
        .agent_views
        .binary_search_by(|view| {
            (view.profile_path.as_str(), view.name.as_str()).cmp(&(profile, name))
        })
        .unwrap_or_else(|index| {
            app.agent_views.insert(
                index,
                crate::AgentViewState {
                    profile_path: profile.into(),
                    name: name.into(),
                    ..Default::default()
                },
            );
            index
        });
    &mut app.agent_views[index]
}

pub(crate) fn apply(
    app: &mut AppState,
    next_id: &mut u64,
    command: Command,
) -> Result<Vec<Event>, StateError> {
    let window = &mut app.window;
    let events = match command {
        Command::SetGlobalPaused { paused } => {
            if app.global_paused == paused {
                vec![]
            } else {
                app.global_paused = paused;
                let mut events = vec![];
                if !paused {
                    for view in &mut app.agent_views {
                        if view.keep_paused {
                            view.keep_paused = false;
                            events.push(Event::AgentViewChanged {
                                profile_path: view.profile_path.clone(),
                                name: view.name.clone(),
                                view: Some(view.clone()),
                            });
                        }
                    }
                }
                events.push(Event::GlobalPauseChanged { paused });
                events
            }
        }
        Command::SetAgentKeepPaused {
            profile_path,
            name,
            keep_paused,
        } => {
            if !app.global_paused {
                return Err(StateError::GlobalPauseRequired);
            }
            if !keep_paused
                && !app.agent_views.iter().any(|view| {
                    view.profile_path == profile_path && view.name == name && view.keep_paused
                })
            {
                return Ok(vec![]);
            }
            let view = agent_view(app, &profile_path, &name);
            if view.keep_paused == keep_paused {
                return Ok(vec![]);
            }
            view.keep_paused = keep_paused;
            vec![Event::AgentViewChanged {
                profile_path,
                name,
                view: Some(view.clone()),
            }]
        }
        Command::ClearAgentDraft {
            profile_path,
            name,
            text,
        } => {
            let Some(view) = app
                .agent_views
                .iter_mut()
                .find(|v| v.profile_path == profile_path && v.name == name && v.draft == text)
            else {
                return Ok(vec![]);
            };
            view.draft.clear();
            vec![Event::AgentDraftCleared {
                profile_path,
                name,
                text,
            }]
        }
        Command::SetAgentDraft {
            profile_path,
            name,
            text,
        } => {
            if app
                .agent_views
                .iter()
                .any(|v| v.profile_path == profile_path && v.name == name && v.draft == text)
            {
                return Ok(vec![]);
            }
            let view = agent_view(app, &profile_path, &name);
            view.draft = text;
            vec![Event::AgentViewChanged {
                profile_path,
                name,
                view: Some(view.clone()),
            }]
        }
        Command::SetAgentScroll {
            profile_path,
            name,
            anchor,
        } => {
            if app
                .agent_views
                .iter()
                .any(|v| v.profile_path == profile_path && v.name == name && v.anchor == anchor)
            {
                return Ok(vec![]);
            }
            let view = agent_view(app, &profile_path, &name);
            view.anchor = anchor;
            vec![Event::AgentViewChanged {
                profile_path,
                name,
                view: Some(view.clone()),
            }]
        }
        Command::ClearAgentView { profile_path, name } => {
            app.agent_views
                .retain(|view| view.profile_path != profile_path || view.name != name);
            vec![Event::AgentViewChanged {
                profile_path,
                name,
                view: None,
            }]
        }
        Command::OpenSession {
            title,
            kind,
            profile_path,
        } => {
            let id = allocate(next_id);
            let pane = Pane::Terminal {
                id: allocate(next_id),
            };
            let tab = Tab {
                id,
                title,
                session: Session { kind, profile_path },
                pane,
            };
            let index = to_u32(window.tabs.len());
            window.tabs.push(tab.clone());
            window.active_tab = Some(id);
            vec![
                Event::SessionOpened { tab, index },
                Event::TabActivated { tab: Some(id) },
            ]
        }
        Command::CloseSession { tab } => {
            let index = find(window, tab)?;
            window.tabs.remove(index);
            let mut events = vec![Event::SessionClosed { tab }];
            if window.active_tab == Some(tab) {
                let nearest = window
                    .tabs
                    .get(index.min(window.tabs.len().saturating_sub(1)))
                    .map(|t| t.id);
                window.active_tab = nearest;
                events.push(Event::TabActivated { tab: nearest });
            }
            events
        }
        Command::ActivateTab { tab } => {
            find(window, tab)?;
            activate(window, tab)
        }
        Command::ActivateNext | Command::ActivatePrevious => {
            let len = window.tabs.len();
            match window.active_tab.and_then(|id| window.index_of(id)) {
                Some(index) if len > 1 => {
                    let target = if command == Command::ActivateNext {
                        (index + 1) % len
                    } else {
                        (index + len - 1) % len
                    };
                    let id = window.tabs[target].id;
                    activate(window, id)
                }
                _ => vec![],
            }
        }
        Command::MoveActiveTabBackward | Command::MoveActiveTabForward => {
            let step = if command == Command::MoveActiveTabForward {
                1
            } else {
                -1
            };
            let len = window.tabs.len();
            let from = window.active_tab.and_then(|id| window.index_of(id));
            match from.and_then(|from| Some((from, from.checked_add_signed(step)?))) {
                Some((from, to)) if to < len => {
                    window.tabs.swap(from, to);
                    vec![Event::TabMoved {
                        tab: window.tabs[to].id,
                        from: to_u32(from),
                        to: to_u32(to),
                    }]
                }
                _ => vec![],
            }
        }
        Command::MoveTab { tab, index } => {
            let from = find(window, tab)?;
            let len = to_u32(window.tabs.len());
            if index >= len {
                return Err(StateError::IndexOutOfRange { index, len });
            }
            if from as u32 == index {
                vec![]
            } else {
                let moved = window.tabs.remove(from);
                window.tabs.insert(index as usize, moved);
                vec![Event::TabMoved {
                    tab,
                    from: from as u32,
                    to: index,
                }]
            }
        }
        Command::SetTitle { tab, title } => {
            let index = find(window, tab)?;
            let current = &mut window.tabs[index].title;
            if *current == title {
                vec![]
            } else {
                current.clone_from(&title);
                vec![Event::TitleChanged { tab, title }]
            }
        }
        Command::WorkspaceCreated { tab, path } => {
            finish_creation(window, tab, SessionKind::Workspace { path })?
        }
        Command::WorkspaceCreationFailed { tab, reason } => {
            finish_creation(window, tab, SessionKind::CreationFailed { reason })?
        }
        Command::OpenPreview { terminal, path } => {
            open_preview(window, next_id, terminal, path, false)?
        }
        Command::StackPreview { terminal, path } => {
            open_preview(window, next_id, terminal, path, true)?
        }
        Command::ResizePreview { tab, pane, width } => {
            if !(crate::MIN_PREVIEW_WIDTH..=crate::MAX_PREVIEW_WIDTH).contains(&width) {
                return Err(StateError::InvalidPreviewWidth { width });
            }
            let index = find(window, tab)?;
            let current = &mut window.tabs[index].pane;
            let Pane::PreviewRow { previews, .. } = current else {
                return Err(StateError::UnknownPreview { pane });
            };
            let preview = previews
                .iter_mut()
                .find(|p| p.id == pane)
                .ok_or(StateError::UnknownPreview { pane })?;
            if preview.width == width {
                vec![]
            } else {
                preview.width = width;
                vec![Event::PanesChanged {
                    tab,
                    pane: current.clone(),
                }]
            }
        }
        Command::ClosePreview { tab, pane } => {
            let index = find(window, tab)?;
            let current = &mut window.tabs[index].pane;
            if !current.previews().iter().any(|p| p.id == pane) {
                return Err(StateError::UnknownPreview { pane });
            }
            let terminal = current.terminal_id();
            let previews = current
                .previews()
                .iter()
                .filter(|p| p.id != pane)
                .cloned()
                .collect::<Vec<_>>();
            *current = if previews.is_empty() {
                Pane::Terminal { id: terminal }
            } else {
                Pane::PreviewRow { terminal, previews }
            };
            vec![Event::PanesChanged {
                tab,
                pane: current.clone(),
            }]
        }
        Command::SetActiveProfile { profile_path } => {
            if app.active_profile == profile_path {
                vec![]
            } else {
                app.active_profile.clone_from(&profile_path);
                vec![Event::ActiveProfileChanged { profile_path }]
            }
        }
    };
    Ok(events)
}

/// Same-path refresh always wins over replace/stack, matching Ember's session behavior.
fn open_preview(
    window: &mut Window,
    next_id: &mut u64,
    terminal: u64,
    path: String,
    stack: bool,
) -> Result<Vec<Event>, StateError> {
    let tab = window
        .tabs
        .iter_mut()
        .find(|t| t.pane.terminal_id() == terminal)
        .ok_or(StateError::UnknownTerminal { terminal })?;
    let mut previews = tab.pane.previews().to_vec();
    if let Some(current) = previews.iter_mut().find(|p| p.path == path) {
        current.revision = current.revision.wrapping_add(1);
    } else {
        if !stack {
            previews.clear();
        }
        previews.push(PreviewPane {
            id: allocate(next_id),
            path,
            revision: 0,
            width: crate::DEFAULT_PREVIEW_WIDTH,
        });
    }
    tab.pane = Pane::PreviewRow { terminal, previews };
    Ok(vec![Event::PanesChanged {
        tab: tab.id,
        pane: tab.pane.clone(),
    }])
}

fn allocate(next_id: &mut u64) -> u64 {
    *next_id += 1;
    *next_id
}

fn activate(window: &mut Window, tab: TabId) -> Vec<Event> {
    if window.active_tab == Some(tab) {
        return vec![];
    }
    window.active_tab = Some(tab);
    vec![Event::TabActivated { tab: Some(tab) }]
}

fn find(window: &Window, tab: TabId) -> Result<usize, StateError> {
    window.index_of(tab).ok_or(StateError::UnknownTab { tab })
}

fn finish_creation(
    window: &mut Window,
    tab: TabId,
    kind: SessionKind,
) -> Result<Vec<Event>, StateError> {
    let index = find(window, tab)?;
    let tab = &mut window.tabs[index];
    if tab.session.kind != SessionKind::CreatingWorkspace {
        return Err(StateError::NotCreating { tab: tab.id });
    }
    tab.session.kind = kind;
    Ok(vec![Event::SessionChanged {
        tab: tab.id,
        session: tab.session.clone(),
    }])
}

/// Tab counts and indices cross the facade as `u32`; a window never holds anywhere near `u32::MAX` tabs.
fn to_u32(n: usize) -> u32 {
    u32::try_from(n).expect("tab count fits in u32")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::State;

    fn open(state: &mut State, title: &str, kind: SessionKind) -> TabId {
        let events = state
            .apply(Command::OpenSession {
                title: title.into(),
                kind,
                profile_path: Some("/profiles/work".into()),
            })
            .unwrap();
        match &events[0] {
            Event::SessionOpened { tab, .. } => tab.id,
            other => panic!("expected SessionOpened, got {other:?}"),
        }
    }

    fn shell(state: &mut State, title: &str) -> TabId {
        open(state, title, SessionKind::Shell)
    }

    fn order(state: &State) -> Vec<TabId> {
        state
            .snapshot()
            .window
            .tabs
            .iter()
            .map(|tab| tab.id)
            .collect()
    }

    fn active(state: &State) -> Option<TabId> {
        state.snapshot().window.active_tab
    }

    #[test]
    fn open_session_appends_and_activates_the_tab() {
        let mut state = State::new();
        let first = shell(&mut state, "Shell");
        let events = state
            .apply(Command::OpenSession {
                title: "api".into(),
                kind: SessionKind::Workspace {
                    path: "/ws/1".into(),
                },
                profile_path: Some("/profiles/work".into()),
            })
            .unwrap();

        let tab = state.snapshot().window.tabs[1].clone();
        assert_eq!(
            events,
            vec![
                Event::SessionOpened {
                    tab: tab.clone(),
                    index: 1
                },
                Event::TabActivated { tab: Some(tab.id) }
            ]
        );
        assert_eq!(tab.title, "api");
        assert_eq!(tab.session.workspace_path(), Some("/ws/1"));
        assert_eq!(tab.session.profile_path.as_deref(), Some("/profiles/work"));
        assert_eq!(order(&state), vec![first, tab.id]);
        assert_eq!(active(&state), Some(tab.id));
    }

    #[test]
    fn tab_and_pane_ids_are_unique_and_never_reused() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        state.apply(Command::CloseSession { tab: a }).unwrap();
        let b = shell(&mut state, "b");
        let pane = state.snapshot().window.tabs[0].pane.terminal_id();
        let mut ids = vec![a, b, pane];
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn closing_the_active_tab_activates_the_tab_now_at_its_position() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        let b = shell(&mut state, "b");
        let c = shell(&mut state, "c");
        state.apply(Command::ActivateTab { tab: b }).unwrap();

        let events = state.apply(Command::CloseSession { tab: b }).unwrap();
        assert_eq!(
            events,
            vec![
                Event::SessionClosed { tab: b },
                Event::TabActivated { tab: Some(c) }
            ]
        );
        assert_eq!(order(&state), vec![a, c]);
        assert_eq!(active(&state), Some(c));
    }

    #[test]
    fn closing_the_last_active_tab_activates_the_new_last_one() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        let b = shell(&mut state, "b");
        let events = state.apply(Command::CloseSession { tab: b }).unwrap();
        assert_eq!(
            events,
            vec![
                Event::SessionClosed { tab: b },
                Event::TabActivated { tab: Some(a) }
            ]
        );
    }

    #[test]
    fn closing_the_only_tab_leaves_no_active_tab() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        let events = state.apply(Command::CloseSession { tab: a }).unwrap();
        assert_eq!(
            events,
            vec![
                Event::SessionClosed { tab: a },
                Event::TabActivated { tab: None }
            ]
        );
        assert_eq!(state.snapshot().window.tabs, vec![]);
        assert_eq!(active(&state), None);
    }

    #[test]
    fn closing_an_inactive_tab_keeps_the_active_tab() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        let b = shell(&mut state, "b");
        let events = state.apply(Command::CloseSession { tab: a }).unwrap();
        assert_eq!(events, vec![Event::SessionClosed { tab: a }]);
        assert_eq!(active(&state), Some(b));
    }

    #[test]
    fn activate_tab_emits_only_on_change() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        shell(&mut state, "b");
        assert_eq!(
            state.apply(Command::ActivateTab { tab: a }).unwrap(),
            vec![Event::TabActivated { tab: Some(a) }]
        );
        assert_eq!(
            state.apply(Command::ActivateTab { tab: a }).unwrap(),
            vec![]
        );
        assert_eq!(active(&state), Some(a));
    }

    #[test]
    fn activate_next_and_previous_wrap_around() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        let b = shell(&mut state, "b");
        let c = shell(&mut state, "c");

        assert_eq!(
            state.apply(Command::ActivateNext).unwrap(),
            vec![Event::TabActivated { tab: Some(a) }]
        );
        assert_eq!(
            state.apply(Command::ActivateNext).unwrap(),
            vec![Event::TabActivated { tab: Some(b) }]
        );
        assert_eq!(
            state.apply(Command::ActivatePrevious).unwrap(),
            vec![Event::TabActivated { tab: Some(a) }]
        );
        assert_eq!(
            state.apply(Command::ActivatePrevious).unwrap(),
            vec![Event::TabActivated { tab: Some(c) }]
        );
    }

    #[test]
    fn activate_next_and_previous_do_nothing_with_fewer_than_two_tabs() {
        let mut state = State::new();
        assert_eq!(state.apply(Command::ActivateNext).unwrap(), vec![]);
        shell(&mut state, "a");
        assert_eq!(state.apply(Command::ActivateNext).unwrap(), vec![]);
        assert_eq!(state.apply(Command::ActivatePrevious).unwrap(), vec![]);
    }

    #[test]
    fn move_tab_shifts_the_tabs_in_between() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        let b = shell(&mut state, "b");
        let c = shell(&mut state, "c");

        assert_eq!(
            state.apply(Command::MoveTab { tab: a, index: 2 }).unwrap(),
            vec![Event::TabMoved {
                tab: a,
                from: 0,
                to: 2
            }]
        );
        assert_eq!(order(&state), vec![b, c, a]);
        assert_eq!(
            state.apply(Command::MoveTab { tab: a, index: 0 }).unwrap(),
            vec![Event::TabMoved {
                tab: a,
                from: 2,
                to: 0
            }]
        );
        assert_eq!(order(&state), vec![a, b, c]);
        assert_eq!(
            active(&state),
            Some(c),
            "moving doesn't change the active tab"
        );
    }

    #[test]
    fn move_active_tab_moves_it_one_place_without_wrapping() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        let b = shell(&mut state, "b");
        let c = shell(&mut state, "c");

        assert_eq!(
            state.apply(Command::MoveActiveTabForward).unwrap(),
            vec![],
            "the last tab stays put"
        );
        assert_eq!(
            state.apply(Command::MoveActiveTabBackward).unwrap(),
            vec![Event::TabMoved {
                tab: c,
                from: 2,
                to: 1
            }]
        );
        state.apply(Command::MoveActiveTabBackward).unwrap();
        assert_eq!(order(&state), vec![c, a, b]);
        assert_eq!(
            state.apply(Command::MoveActiveTabBackward).unwrap(),
            vec![],
            "the first tab stays put"
        );
        assert_eq!(
            active(&state),
            Some(c),
            "moving doesn't change the active tab"
        );
    }

    #[test]
    fn move_active_tab_does_nothing_without_an_active_tab() {
        let mut state = State::new();
        assert_eq!(state.apply(Command::MoveActiveTabForward).unwrap(), vec![]);
        assert_eq!(state.apply(Command::MoveActiveTabBackward).unwrap(), vec![]);
    }

    #[test]
    fn move_tab_to_its_own_index_does_nothing() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        shell(&mut state, "b");
        assert_eq!(
            state.apply(Command::MoveTab { tab: a, index: 0 }).unwrap(),
            vec![]
        );
    }

    #[test]
    fn move_tab_past_the_end_is_rejected() {
        let mut state = State::new();
        let a = shell(&mut state, "a");
        shell(&mut state, "b");
        let before = state.snapshot().clone();
        assert_eq!(
            state.apply(Command::MoveTab { tab: a, index: 2 }),
            Err(StateError::IndexOutOfRange { index: 2, len: 2 })
        );
        assert_eq!(state.snapshot(), &before);
    }

    #[test]
    fn set_title_emits_only_on_change() {
        let mut state = State::new();
        let a = shell(&mut state, "Shell");
        assert_eq!(
            state
                .apply(Command::SetTitle {
                    tab: a,
                    title: "vim".into()
                })
                .unwrap(),
            vec![Event::TitleChanged {
                tab: a,
                title: "vim".into()
            }]
        );
        assert_eq!(
            state
                .apply(Command::SetTitle {
                    tab: a,
                    title: "vim".into()
                })
                .unwrap(),
            vec![]
        );
        assert_eq!(state.snapshot().window.tabs[0].title, "vim");
    }

    #[test]
    fn workspace_created_turns_a_creating_tab_into_a_workspace() {
        let mut state = State::new();
        let a = open(&mut state, "task", SessionKind::CreatingWorkspace);
        let events = state
            .apply(Command::WorkspaceCreated {
                tab: a,
                path: "/ws/2".into(),
            })
            .unwrap();
        let session = Session {
            kind: SessionKind::Workspace {
                path: "/ws/2".into(),
            },
            profile_path: Some("/profiles/work".into()),
        };
        assert_eq!(
            events,
            vec![Event::SessionChanged {
                tab: a,
                session: session.clone()
            }]
        );
        assert_eq!(state.snapshot().window.tabs[0].session, session);
    }

    #[test]
    fn workspace_creation_failed_keeps_the_tab_with_the_reason() {
        let mut state = State::new();
        let a = open(&mut state, "task", SessionKind::CreatingWorkspace);
        let events = state
            .apply(Command::WorkspaceCreationFailed {
                tab: a,
                reason: "clone failed".into(),
            })
            .unwrap();
        let kind = SessionKind::CreationFailed {
            reason: "clone failed".into(),
        };
        assert_eq!(
            events,
            vec![Event::SessionChanged {
                tab: a,
                session: Session {
                    kind,
                    profile_path: Some("/profiles/work".into())
                }
            }]
        );
        assert_eq!(order(&state), vec![a]);
    }

    #[test]
    fn creation_outcomes_are_rejected_for_tabs_that_are_not_creating() {
        let mut state = State::new();
        let a = shell(&mut state, "Shell");
        let before = state.snapshot().clone();
        assert_eq!(
            state.apply(Command::WorkspaceCreated {
                tab: a,
                path: "/ws".into()
            }),
            Err(StateError::NotCreating { tab: a })
        );
        assert_eq!(
            state.apply(Command::WorkspaceCreationFailed {
                tab: a,
                reason: "x".into()
            }),
            Err(StateError::NotCreating { tab: a })
        );

        let b = open(&mut state, "task", SessionKind::CreatingWorkspace);
        state
            .apply(Command::WorkspaceCreated {
                tab: b,
                path: "/ws".into(),
            })
            .unwrap();
        assert_eq!(
            state.apply(Command::WorkspaceCreated {
                tab: b,
                path: "/ws".into()
            }),
            Err(StateError::NotCreating { tab: b })
        );
        assert_eq!(state.snapshot().window.tabs[0], before.window.tabs[0]);
    }

    #[test]
    fn set_active_profile_emits_only_on_change() {
        let mut state = State::new();
        let work = Some("/profiles/work".to_string());
        assert_eq!(
            state
                .apply(Command::SetActiveProfile {
                    profile_path: work.clone()
                })
                .unwrap(),
            vec![Event::ActiveProfileChanged {
                profile_path: work.clone()
            }]
        );
        assert_eq!(
            state
                .apply(Command::SetActiveProfile {
                    profile_path: work.clone()
                })
                .unwrap(),
            vec![]
        );
        assert_eq!(state.snapshot().active_profile, work);
        assert_eq!(
            state
                .apply(Command::SetActiveProfile { profile_path: None })
                .unwrap(),
            vec![Event::ActiveProfileChanged { profile_path: None }]
        );
    }

    #[test]
    fn commands_on_an_unknown_tab_are_rejected_without_changes() {
        let mut state = State::new();
        shell(&mut state, "a");
        let before = state.snapshot().clone();
        let unknown = 999;
        for command in [
            Command::CloseSession { tab: unknown },
            Command::ActivateTab { tab: unknown },
            Command::MoveTab {
                tab: unknown,
                index: 0,
            },
            Command::SetTitle {
                tab: unknown,
                title: "x".into(),
            },
            Command::WorkspaceCreated {
                tab: unknown,
                path: "/ws".into(),
            },
            Command::WorkspaceCreationFailed {
                tab: unknown,
                reason: "x".into(),
            },
        ] {
            assert_eq!(
                state.apply(command.clone()),
                Err(StateError::UnknownTab { tab: unknown }),
                "{command:?}"
            );
        }
        assert_eq!(state.snapshot(), &before);
    }

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(
            StateError::UnknownTab { tab: 7 }.to_string(),
            "no tab with id 7"
        );
        assert_eq!(
            StateError::IndexOutOfRange { index: 3, len: 2 }.to_string(),
            "tab index 3 is out of range for 2 tabs"
        );
        assert_eq!(
            StateError::NotCreating { tab: 7 }.to_string(),
            "tab 7 is not creating a workspace"
        );
    }

    #[test]
    fn state_and_events_round_trip_through_json() {
        let mut state = State::new();
        let events = state
            .apply(Command::OpenSession {
                title: "task".into(),
                kind: SessionKind::CreatingWorkspace,
                profile_path: None,
            })
            .unwrap();
        let json = serde_json::to_string(&events).unwrap();
        assert_eq!(serde_json::from_str::<Vec<Event>>(&json).unwrap(), events);
        let json = serde_json::to_string(state.snapshot()).unwrap();
        assert_eq!(
            &serde_json::from_str::<AppState>(&json).unwrap(),
            state.snapshot()
        );
    }
}
