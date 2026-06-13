//! Audit trail for manual interventions (Constitution §2).
//!
//! Right now the only audited action is overriding a blocked required step, which
//! must carry a human-supplied reason — the regulatory paper trail for "why did
//! this workflow proceed despite a failed mandatory step?".

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
