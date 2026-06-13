//! Tracker synchronization — the §4 logic (the tracker is the source of truth).
//!
//! Pull: render the `fetch` command, run it, read issues out of its JSON via the
//! configured map, and reconcile each against local state. Push: render the
//! `push` command and record what we pushed so the next pull can tell "we changed
//! it" from "someone else changed it" and flag genuine divergence.

use chrono::Utc;
use serde_json::json;

use crate::config::Config;
use crate::db::Db;
use crate::events::{Event, EventBus};
use crate::model::Issue;
use crate::{jsonpath, runner, template, Error, Result};

#[derive(Debug, Default, Clone)]
pub struct SyncOutcome {
    pub fetched: usize,
    pub diverged: usize,
    pub diverged_keys: Vec<String>,
}

/// Pull all issues from the tracker and reconcile them into the local store.
pub async fn sync(db: &Db, config: &Config, bus: &EventBus) -> Result<SyncOutcome> {
    let fetch_cmd = template::render(&config.tracker.fetch, &json!({ "jql": config.tracker.jql }))?;
    let out = runner::run_capture(&fetch_cmd, Some(&config.repo_dir())).await?;
    if !out.success() {
        return Err(Error::command(format!(
            "tracker fetch failed (exit {}): {}",
            out.status,
            out.stderr.trim()
        )));
    }

    let value: serde_json::Value = serde_json::from_str(out.stdout.trim()).map_err(|e| {
        Error::other(format!("tracker fetch did not return valid JSON: {e}"))
    })?;

    let map = &config.tracker.map;
    let items = jsonpath::get_array(&value, &map.items).ok_or_else(|| {
        Error::other(format!(
            "tracker.map.items path '{}' did not resolve to an array",
            map.items
        ))
    })?;

    let mut outcome = SyncOutcome::default();
    for item in items {
        let key = match jsonpath::get_str(item, &map.key) {
            Some(k) if !k.is_empty() => k,
            _ => continue, // an item with no key isn't actionable
        };
        let summary = jsonpath::get_str(item, &map.summary).unwrap_or_default();
        let tracker_status = jsonpath::get_str(item, &map.status).unwrap_or_default();
        let assignee = map.assignee.as_deref().and_then(|p| jsonpath::get_str(item, p));
        let url = map.url.as_deref().and_then(|p| jsonpath::get_str(item, p));
        let labels = map
            .labels
            .as_deref()
            .map(|p| jsonpath::get_str_array(item, p))
            .unwrap_or_default();

        let existing = db.get_issue(&key).await?;
        let issue = reconcile(existing, key.clone(), summary, tracker_status, assignee, url, labels);

        if issue.diverged {
            outcome.diverged += 1;
            outcome.diverged_keys.push(key.clone());
        }
        db.upsert_issue(&issue).await?;
        bus.publish(Event::IssueSynced {
            key: key.clone(),
            status: issue.tracker_status.clone(),
            diverged: issue.diverged,
        });
        outcome.fetched += 1;
    }

    db.insert_sync_log(outcome.fetched as i64, outcome.diverged as i64, None).await?;
    bus.publish(Event::SyncCompleted {
        fetched: outcome.fetched,
        diverged: outcome.diverged,
    });
    Ok(outcome)
}

/// Push a status change to the tracker, then record it locally so future pulls
/// recognize the change as ours (and don't flag it as divergence).
pub async fn push_status(db: &Db, config: &Config, key: &str, target: &str) -> Result<()> {
    let push_tmpl = config
        .tracker
        .push
        .as_deref()
        .ok_or_else(|| Error::config("tracker.push is not configured; cannot set status"))?;
    let cmd = template::render(push_tmpl, &json!({ "key": key, "status": target }))?;
    let out = runner::run_capture(&cmd, Some(&config.repo_dir())).await?;
    if !out.success() {
        return Err(Error::command(format!(
            "tracker push failed (exit {}): {}",
            out.status,
            out.stderr.trim()
        )));
    }
    db.mark_issue_pushed(key, target).await?;
    Ok(())
}

/// The reconciliation rule (§4). The tracker always wins on the *value*; the only
/// question is whether the change was ours (reconciled) or external (diverged).
fn reconcile(
    existing: Option<Issue>,
    key: String,
    summary: String,
    tracker_status: String,
    assignee: Option<String>,
    url: Option<String>,
    labels: Vec<String>,
) -> Issue {
    match existing {
        None => {
            // First time we've seen this issue: adopt everything, no divergence.
            let mut i = Issue::new(key, summary, &tracker_status);
            i.assignee = assignee;
            i.url = url;
            i.labels = labels;
            i.last_synced = Some(Utc::now());
            i
        }
        Some(mut i) => {
            i.summary = summary;
            i.assignee = assignee;
            i.url = url;
            i.labels = labels;
            i.last_synced = Some(Utc::now());

            if tracker_status != i.local_status {
                let we_caused = i.last_pushed_status.as_deref() == Some(tracker_status.as_str());
                // Tracker wins regardless; flag only when we weren't the cause.
                i.local_status = tracker_status.clone();
                i.diverged = !we_caused;
            } else {
                i.diverged = false;
            }
            i.tracker_status = tracker_status;
            i
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(key: &str, status: &str) -> Issue {
        Issue::new(key, "s", status)
    }

    #[test]
    fn new_issue_is_not_diverged() {
        let i = reconcile(None, "P-1".into(), "s".into(), "To Do".into(), None, None, vec![]);
        assert!(!i.diverged);
        assert_eq!(i.tracker_status, "To Do");
        assert_eq!(i.local_status, "To Do");
    }

    #[test]
    fn unchanged_status_is_not_diverged() {
        let existing = seen("P-1", "In Progress");
        let i = reconcile(Some(existing), "P-1".into(), "s".into(), "In Progress".into(), None, None, vec![]);
        assert!(!i.diverged);
    }

    #[test]
    fn our_own_push_reconciles_without_divergence() {
        let mut existing = seen("P-1", "In Progress");
        // We pushed "In Review"; locally we already reflect it.
        existing.local_status = "In Review".into();
        existing.last_pushed_status = Some("In Review".into());
        // Tracker now reports the value we pushed.
        let i = reconcile(Some(existing), "P-1".into(), "s".into(), "In Review".into(), None, None, vec![]);
        assert!(!i.diverged);
        assert_eq!(i.local_status, "In Review");
    }

    #[test]
    fn external_change_diverges_and_tracker_wins() {
        let mut existing = seen("P-1", "In Progress");
        existing.local_status = "In Progress".into();
        existing.last_pushed_status = Some("In Progress".into());
        // Someone else moved it to Done in the tracker.
        let i = reconcile(Some(existing), "P-1".into(), "s".into(), "Done".into(), None, None, vec![]);
        assert!(i.diverged);
        assert_eq!(i.local_status, "Done");
        assert_eq!(i.tracker_status, "Done");
    }
}
