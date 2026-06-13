//! Operational log of manual interventions (Constitution §2).
//!
//! Right now the only logged action is overriding a blocked required step, which
//! carries a human-supplied reason. This is for debugging and operational clarity
//! ("why did this workflow proceed despite a failed mandatory step?") — not a
//! regulatory compliance record.

use crate::db::Db;
use crate::Result;

/// Record that a human overrode a blocked required step, with a mandatory reason.
pub async fn record_override(
    db: &Db,
    issue_key: &str,
    step_run_id: i64,
    step_id: &str,
    reason: &str,
) -> Result<()> {
    let detail = format!("step_run {step_run_id} ({step_id})");
    db.insert_audit("override", Some(issue_key), Some(reason), Some(&detail))
        .await
}
