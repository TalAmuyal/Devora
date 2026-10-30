//! The app-state model every Devora UI renders (ADR-008): the window, its tabs and the session behind each tab, plus the active profile.
//!
//! The UIs are projections of this state.
//! They change it only by dispatching a named [`Command`], which one reducer applies, and they learn what changed from the typed [`Event`]s it emits (ADR-007: commands in, events out).
//!
//! - [`State`] is the pure reducer: no I/O, no threads, testable without a UI.
//! - [`Store`] shares one `State` between threads and fans its events out to [`Subscription`]s, which are pulled in batches (ADR-010).
//!
//! Everything that crosses this crate's API is owned data (records, plain enums, `String`, `u32`/`u64`), so the UniFFI facade can expose it as is.
//!
//! The model is deliberately the first release's: one terminal pane per tab.
//! Inner tabs, splits, overlays and agent sessions (ADR-008) grow out of [`Tab`] and [`Pane`] later.

mod command;
mod model;
mod store;

pub use command::{Command, Event, StateError};
pub use model::{AppState, Pane, PaneId, Session, SessionKind, Tab, TabId, Window};
pub use store::{Store, Subscription};

use command::apply;

/// The reducer: the current [`AppState`] and the id allocator, changed only through [`State::apply`].
#[derive(Clone, Debug, Default)]
pub struct State {
    app: AppState,
    next_id: u64,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current state, as a UI renders it.
    pub fn snapshot(&self) -> &AppState {
        &self.app
    }

    /// Applies one command and returns the events it caused, in order.
    ///
    /// A command that changes nothing (activating the active tab, a title set to itself) succeeds with no events.
    /// A failed command leaves the state untouched.
    pub fn apply(&mut self, command: Command) -> Result<Vec<Event>, StateError> {
        apply(&mut self.app, &mut self.next_id, command)
    }
}
