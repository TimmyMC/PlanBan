//! In-process event bus for live updates.
//!
//! Constitution §3 (overview first) and §8 (fast feedback): one broadcast channel
//! that the CLI `--watch` view and, later, the Tauri UI both subscribe to. Core
//! publishes; drivers consume. Core never knows who is listening (§7).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    SessionLog {
        session_id: i64,
        stream: String,
        line: String,
        ts: DateTime<Utc>,
    },
    SessionStatus {
        session_id: i64,
        issue_key: String,
        status: String,
    },
    IssueSynced {
        key: String,
        status: String,
        diverged: bool,
    },
    SyncCompleted {
        fetched: usize,
        diverged: usize,
    },
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        EventBus { tx }
    }

    /// Publish an event. Having no subscribers is not an error.
    pub fn publish(&self, ev: Event) {
        let _ = self.tx.send(ev);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn published_event_reaches_a_subscriber() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.publish(Event::SyncCompleted {
            fetched: 7,
            diverged: 2,
        });
        // A no-op `publish` (the mutant) would leave nothing to receive.
        match rx.try_recv() {
            Ok(Event::SyncCompleted { fetched, diverged }) => {
                assert_eq!(fetched, 7);
                assert_eq!(diverged, 2);
            }
            other => panic!("expected the published event, got {other:?}"),
        }
    }
}
