use std::time::{SystemTime, UNIX_EPOCH};
use crate::database::DbConnection;
use crate::models::AuditLogEntry;

pub fn log_audit_event(
    conn: &DbConnection,
    mandate_id: &str,
    user_id: &str,
    merchant_id: &str,
    requested_amount: i64,
    decision: &str,
    reason: &str,
    details_json: &str,
    razorpay_order_id: Option<&str>,
) -> Result<AuditLogEntry, String> {
    let log_id = format!("log_{}", generate_hex_id(6));
    let timestamp = get_iso_timestamp();

    let entry = AuditLogEntry {
        log_id,
        timestamp,
        mandate_id: mandate_id.to_string(),
        user_id: user_id.to_string(),
        merchant_id: merchant_id.to_string(),
        requested_amount,
        decision: decision.to_string(),
        reason: reason.to_string(),
        details_json: details_json.to_string(),
        razorpay_order_id: razorpay_order_id.map(|s| s.to_string()),
    };

    conn.insert_audit_log(&entry)?;
    Ok(entry)
}

pub fn get_audit_logs(
    conn: &DbConnection,
    mandate_id: Option<&str>,
    decision: Option<&str>,
    limit: usize,
) -> Result<Vec<AuditLogEntry>, String> {
    conn.query_audit_logs(mandate_id, decision, limit)
}

fn generate_hex_id(len: usize) -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
    let s = format!("{:x}{:x}", nanos, std::process::id());
    s.chars().take(len).collect()
}

pub fn get_iso_timestamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    format!("{}-01-01T00:00:00Z", 1970 + secs / 31536000)
}
