use mandate_gate::config::Config;
use mandate_gate::database::{init_db, DbConnection};

fn main() -> Result<(), String> {
    let cfg = Config::load();
    init_db(&cfg.db_path)?;

    let conn = DbConnection::open(&cfg.db_path)?;
    conn.execute("DELETE FROM mandates;")?;
    conn.execute("DELETE FROM audit_logs;")?;

    let today_str = get_current_date_str();
    let now_iso = get_current_iso_str();

    let sql = format!(
        "INSERT INTO mandates (
            mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total,
            spent_today, spent_total, last_spent_date, expires_at, created_at, status
        ) VALUES 
        ('mandate_zepto_01', 'user_rahul', 'zepto', 80000, 200000, 1000000, 50000, 150000, '{today_str}', '2027-12-31T23:59:59Z', '{now_iso}', 'active'),
        ('mandate_swiggy_01', 'user_rahul', 'swiggy', 150000, 300000, 1500000, 280000, 500000, '{today_str}', '2027-12-31T23:59:59Z', '{now_iso}', 'active'),
        ('mandate_amazon_01', 'user_priya', 'amazon', 500000, 1000000, 5000000, 0, 0, '{today_str}', '2027-12-31T23:59:59Z', '{now_iso}', 'active'),
        ('mandate_expired_01', 'user_rahul', 'blinkit', 100000, 200000, 500000, 0, 0, '{today_str}', '2025-01-01T00:00:00Z', '2024-01-01T00:00:00Z', 'expired');"
    );

    conn.execute(&sql)?;
    println!("Database successfully seeded with 4 sample mandates in {}", cfg.db_path);
    Ok(())
}

fn get_current_date_str() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let days = secs / 86400;
    let z = (days as i64) + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn get_current_iso_str() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    format!("{}-01-01T00:00:00Z", 1970 + secs / 31536000)
}
