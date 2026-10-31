//! The shared, thread-safe home of one [`State`], and the batched event subscriptions the UIs pull from (ADR-010).

use crate::feed::{feed, Feed, Subscription};
use crate::persist::Db;
use crate::{AppState, Command, Event, State, StateError};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// One [`State`] shared between threads: commands are applied one at a time, and their events reach every open [`Subscription`] in the order they were applied.
#[derive(Default)]
pub struct Store {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    state: State,
    subscribers: Vec<Feed<Event>>,
    /// Where every change is committed before it takes effect; `None` for an in-memory store.
    db: Option<Db>,
}

impl Store {
    /// An in-memory store, starting empty and forgotten when dropped.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the store persisted in the SQLite database at `path`, creating it (and its directory) if needed.
    ///
    /// The state is restored as the last committed command left it, except that a tab still creating a workspace becomes [`crate::SessionKind::CreationFailed`] with the reason "interrupted".
    /// Each app passes its own path, since apps never share a state database (ADR-008).
    /// The store holds the database until it is dropped: opening it while another store, in this process or another, holds it fails with [`StateError::InUse`].
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StateError> {
        let (db, state) = Db::open(path.as_ref())?;
        Ok(Self {
            inner: Mutex::new(Inner {
                state,
                subscribers: Vec::new(),
                db: Some(db),
            }),
        })
    }

    /// Applies a command and returns its events; open subscriptions receive the same events.
    ///
    /// A persistent store commits the change before the events go out.
    /// If that fails, the command takes no effect and returns [`StateError::Storage`].
    pub fn dispatch(&self, command: Command) -> Result<Vec<Event>, StateError> {
        let inner = &mut *lock(&self.inner);
        // Applied to a copy, which replaces the state only once committed, so memory never runs ahead of the disk.
        let mut next = inner.state.clone();
        let events = next.apply(command)?;
        if events.is_empty() {
            return Ok(events);
        }
        if let Some(db) = &mut inner.db {
            db.save(&next)?;
        }
        inner.state = next;
        inner.subscribers.retain(|feed| feed.push(&events));
        Ok(events)
    }

    pub fn snapshot(&self) -> AppState {
        lock(&self.inner).state.snapshot().clone()
    }

    #[cfg(test)]
    pub(crate) fn state(&self) -> State {
        lock(&self.inner).state.clone()
    }

    /// Opens a subscription whose first event is an [`Event::Snapshot`] of the current state, followed by every event from later commands.
    ///
    /// Taking the snapshot and registering happen under one lock, so no event is missed or seen twice.
    pub fn subscribe(&self) -> Subscription<Event> {
        let mut inner = lock(&self.inner);
        let (feed, subscription) = feed();
        feed.push(&[Event::Snapshot {
            state: inner.state.snapshot().clone(),
        }]);
        inner.subscribers.push(feed);
        subscription
    }
}

/// A panic while a lock was held can't leave the state half-applied (a command is applied to a copy, and an unfinished transaction rolls back), nor a queue (an item is pushed or taken whole), so a poisoned lock is still safe to use.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionKind;
    use futures_lite::future::{block_on, poll_once};
    use std::pin::pin;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;

    fn open_shell(store: &Store, title: &str) -> Vec<Event> {
        store
            .dispatch(Command::OpenSession {
                title: title.into(),
                kind: SessionKind::Shell,
                profile_path: None,
            })
            .unwrap()
    }

    #[test]
    fn a_subscription_starts_with_a_snapshot_then_receives_every_event_in_order() {
        let store = Store::new();
        open_shell(&store, "before");
        let subscription = store.subscribe();
        let opened = open_shell(&store, "after");

        let batch = subscription.try_next_batch(100);
        let mut expected = vec![Event::Snapshot {
            state: batch_snapshot(&batch),
        }];
        expected.extend(opened);
        assert_eq!(batch, expected);
        assert_eq!(
            batch_snapshot(&batch).window.tabs.len(),
            1,
            "the snapshot predates the second command"
        );
        assert_eq!(subscription.try_next_batch(100), vec![]);
    }

    fn batch_snapshot(batch: &[Event]) -> AppState {
        match &batch[0] {
            Event::Snapshot { state } => state.clone(),
            other => panic!("expected a snapshot first, got {other:?}"),
        }
    }

    #[test]
    fn batches_hold_at_most_max_events_and_at_least_one() {
        let store = Store::new();
        let subscription = store.subscribe();
        open_shell(&store, "a");
        open_shell(&store, "b");

        assert_eq!(
            subscription.try_next_batch(0).len(),
            1,
            "max 0 still takes one event"
        );
        assert_eq!(subscription.try_next_batch(2).len(), 2);
        assert_eq!(subscription.try_next_batch(100).len(), 2);
    }

    #[test]
    fn rejected_and_no_op_commands_reach_no_subscriber() {
        let store = Store::new();
        let subscription = store.subscribe();
        subscription.try_next_batch(100);
        assert!(store.dispatch(Command::ActivateTab { tab: 1 }).is_err());
        store.dispatch(Command::ActivateNext).unwrap();
        assert_eq!(subscription.try_next_batch(100), vec![]);
    }

    #[test]
    fn next_batch_blocking_waits_for_an_event_from_another_thread() {
        let store = Arc::new(Store::new());
        let subscription = store.subscribe();
        subscription.next_batch_blocking(100);

        let dispatcher = {
            let store = store.clone();
            thread::spawn(move || open_shell(&store, "a"))
        };
        let batch = subscription.next_batch_blocking(100);
        let expected = dispatcher.join().unwrap();
        // The dispatcher's two events are queued under one lock, so a woken consumer sees both.
        assert_eq!(batch, expected);
    }

    #[test]
    fn close_wakes_a_blocked_next_batch_with_an_empty_batch() {
        let store = Store::new();
        let subscription = Arc::new(store.subscribe());
        subscription.next_batch_blocking(100);

        let waiter = {
            let subscription = subscription.clone();
            thread::spawn(move || subscription.next_batch_blocking(100))
        };
        subscription.close();
        assert_eq!(waiter.join().unwrap(), vec![]);
        assert!(subscription.is_closed());
    }

    /// Awaits `next_batch` on another thread, and returns once that wait is known to be pending.
    fn await_next_batch_on_another_thread(
        subscription: &Arc<Subscription<Event>>,
    ) -> thread::JoinHandle<Vec<Event>> {
        let (pending_tx, pending_rx) = mpsc::channel();
        let subscription = subscription.clone();
        let waiter = thread::spawn(move || {
            block_on(async {
                let mut next = pin!(subscription.next_batch(100));
                assert!(poll_once(next.as_mut()).await.is_none());
                pending_tx.send(()).unwrap();
                next.await
            })
        });
        pending_rx.recv().unwrap();
        waiter
    }

    #[test]
    fn a_pending_next_batch_wakes_on_dispatch() {
        let store = Store::new();
        let subscription = Arc::new(store.subscribe());
        block_on(subscription.next_batch(100));

        let waiter = await_next_batch_on_another_thread(&subscription);
        let expected = open_shell(&store, "a");
        assert_eq!(waiter.join().unwrap(), expected);
    }

    #[test]
    fn close_wakes_a_pending_next_batch_with_an_empty_batch() {
        let store = Store::new();
        let subscription = Arc::new(store.subscribe());
        block_on(subscription.next_batch(100));

        let waiter = await_next_batch_on_another_thread(&subscription);
        subscription.close();
        assert_eq!(waiter.join().unwrap(), vec![]);
    }

    #[test]
    fn a_closed_subscription_receives_nothing_and_is_forgotten() {
        let store = Store::new();
        let kept = store.subscribe();
        let closed = store.subscribe();
        closed.close();
        drop(store.subscribe());
        open_shell(&store, "a");

        assert_eq!(closed.try_next_batch(100), vec![]);
        assert_eq!(kept.try_next_batch(100).len(), 3);
        assert_eq!(lock(&store.inner).subscribers.len(), 1);
    }

    #[test]
    fn dispatch_returns_the_events_and_snapshot_reflects_them() {
        let store = Store::new();
        let events = open_shell(&store, "a");
        assert_eq!(events.len(), 2);
        assert_eq!(store.snapshot().window.tabs[0].title, "a");
    }
}
