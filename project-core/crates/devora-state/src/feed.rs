//! Queues the UIs pull in batches (ADR-010): what a [`crate::Store`] fans its events out through, and any other stream a UI pulls, such as the errors the app reports.

use crate::store::lock;
use event_listener::{EventListener, Listener};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A new queue: items pushed to the [`Feed`] wait in it until the [`Subscription`] pulls them.
pub fn feed<T>() -> (Feed<T>, Subscription<T>) {
    let queue = Arc::new(Queue {
        state: Mutex::new(QueueState {
            items: VecDeque::new(),
            closed: false,
        }),
        ready: event_listener::Event::new(),
    });
    (
        Feed {
            queue: queue.clone(),
        },
        Subscription { queue },
    )
}

/// The producing end of a queue.
pub struct Feed<T> {
    queue: Arc<Queue<T>>,
}

impl<T: Clone> Feed<T> {
    /// Queues the items and wakes the consumer; returns false once the subscription is closed, so the producer can forget it.
    pub fn push(&self, items: &[T]) -> bool {
        let mut state = lock(&self.queue.state);
        if state.closed {
            return false;
        }
        state.items.extend(items.iter().cloned());
        self.queue.ready.notify(usize::MAX);
        true
    }
}

/// The consuming end of a queue, pulled in batches.
///
/// Items queue up until pulled, so a slow consumer never stalls the producer.
/// Dropping a subscription closes it.
pub struct Subscription<T> {
    queue: Arc<Queue<T>>,
}

impl<T> Subscription<T> {
    /// Waits until at least one item is queued, then takes up to `max` of them (at least one).
    ///
    /// Returns an empty batch once the subscription is closed, from this thread or another one.
    /// Needs no particular async runtime; dropping the future abandons the wait, but only [`Subscription::close`] ends the subscription.
    pub async fn next_batch(&self, max: u32) -> Vec<T> {
        loop {
            match self.take_or_listen(max) {
                Ok(batch) => return batch,
                Err(listener) => listener.await,
            }
        }
    }

    /// Like [`Subscription::next_batch`], but blocks the calling thread.
    pub fn next_batch_blocking(&self, max: u32) -> Vec<T> {
        loop {
            match self.take_or_listen(max) {
                Ok(batch) => return batch,
                Err(listener) => listener.wait(),
            }
        }
    }

    /// Takes a batch if one is ready, or else starts listening; the listener is registered under the queue's lock, so no notification is missed.
    fn take_or_listen(&self, max: u32) -> Result<Vec<T>, EventListener> {
        let mut state = lock(&self.queue.state);
        if state.items.is_empty() && !state.closed {
            return Err(self.queue.ready.listen());
        }
        Ok(take(&mut state, max))
    }

    /// Takes up to `max` queued items without waiting; empty when none are queued.
    pub fn try_next_batch(&self, max: u32) -> Vec<T> {
        take(&mut lock(&self.queue.state), max)
    }

    /// Stops delivery and wakes every pending [`Subscription::next_batch`] and [`Subscription::next_batch_blocking`]; items still queued are dropped.
    pub fn close(&self) {
        let mut state = lock(&self.queue.state);
        state.closed = true;
        state.items.clear();
        self.queue.ready.notify(usize::MAX);
    }

    pub fn is_closed(&self) -> bool {
        lock(&self.queue.state).closed
    }
}

impl<T> Drop for Subscription<T> {
    fn drop(&mut self) {
        self.close();
    }
}

struct Queue<T> {
    state: Mutex<QueueState<T>>,
    ready: event_listener::Event,
}

struct QueueState<T> {
    items: VecDeque<T>,
    closed: bool,
}

fn take<T>(state: &mut QueueState<T>, max: u32) -> Vec<T> {
    let n = state.items.len().min(max.max(1) as usize);
    state.items.drain(..n).collect()
}
