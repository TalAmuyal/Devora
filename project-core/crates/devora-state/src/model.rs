//! The state records: what a UI renders and what a later restore rebuilds.

use serde::{Deserialize, Serialize};

/// Identifies a tab for its whole life; ids are never reused within a [`crate::State`].
pub type TabId = u64;

/// Identifies a pane; drawn from the same allocator as [`TabId`].
pub type PaneId = u64;

/// The whole app state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub window: Window,
    /// The profile the Workspace Hub shows and creates workspaces in, as the profile's root path.
    pub active_profile: Option<String>,
    #[serde(default)]
    pub agent_views: Vec<AgentViewState>,
}

/// The app's one window.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    /// The workspace tabs, in tab-bar order.
    pub tabs: Vec<Tab>,
    /// The visible tab; `None` only when there are no tabs.
    pub active_tab: Option<TabId>,
}

impl Window {
    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    pub(crate) fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == id)
    }
}

/// A workspace tab: one open session with its terminal and optional previews.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub session: Session,
    /// The tab's terminal, optionally sharing its row with persistent file previews.
    pub pane: Pane,
}

/// What a tab is open on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub kind: SessionKind,
    /// The profile the session belongs to, as the profile's root path; `None` for a shell outside any profile.
    /// For a workspace it is the workspace's own profile, never the active one: the terminal's profile-scoped settings resolve from it.
    pub profile_path: Option<String>,
}

impl Session {
    /// The workspace directory, once the session has one.
    pub fn workspace_path(&self) -> Option<&str> {
        match &self.kind {
            SessionKind::Workspace { path } => Some(path),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionKind {
    /// An Agent Space in its owning profile, with a live chat projection in Store.
    AgentSpace { name: String },
    /// A plain shell, not tied to a workspace.
    Shell,
    /// A workspace is being created for this tab; it becomes [`SessionKind::Workspace`] or [`SessionKind::CreationFailed`].
    CreatingWorkspace,
    /// The tab's workspace creation failed; the tab stays open to show why until it is closed.
    CreationFailed { reason: String },
    /// An open workspace.
    Workspace { path: String },
}

/// A tab's content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pane {
    /// A terminal; the UI owns the live terminal and finds it by `id`.
    Terminal { id: PaneId },
    /// A terminal with its persistent preview panes, in display order.
    PreviewRow {
        terminal: PaneId,
        previews: Vec<PreviewPane>,
    },
}

/// Widths are logical display points; the terminal absorbs free row space.
pub const MIN_PREVIEW_WIDTH: u32 = 200;
pub const MIN_TERMINAL_WIDTH: u32 = 240;
pub const DEFAULT_PREVIEW_WIDTH: u32 = 320;
pub const MAX_PREVIEW_WIDTH: u32 = 16_384;

/// A file preview's durable identity; content is re-read by the UI on restore or revision change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewPane {
    pub id: PaneId,
    pub path: String,
    pub revision: u64,
    /// Preferred width; a smaller window may temporarily constrain the rendered row.
    pub width: u32,
}

impl Pane {
    pub fn terminal_id(&self) -> PaneId {
        match self {
            Self::Terminal { id } => *id,
            Self::PreviewRow { terminal, .. } => *terminal,
        }
    }
    pub fn previews(&self) -> &[PreviewPane] {
        match self {
            Self::Terminal { .. } => &[],
            Self::PreviewRow { previews, .. } => previews,
        }
    }
}

/// Durable chat presentation state, retained when its tab is closed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentViewState {
    pub profile_path: String,
    pub name: String,
    pub draft: String,
    pub anchor: Option<ChatScrollAnchor>,
}

/// The top visible transcript item and its offset, or None for following output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatScrollAnchor {
    pub sequence: u64,
    pub offset: i32,
}
