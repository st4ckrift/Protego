use std::sync::Arc;
use std::thread;
use mandate_gate::config::Config;
use mandate_gate::database::{init_db, DbConnection};
use mandate_gate::models::AuthorizeRequest;
use mandate_gate::gate_service::authorize_transaction;
use mandate_gate::audit_service::get_audit_logs;

fn setup_test_db(db_name: &str) -> (Config, String) {
    let db_path = format!("/tmp/test_gate_{}.db", db_name);
    let _ = std::fs::remove_file(&db_path);

    init_db(&db_path).unwrap();
    let conn = DbConnection::open(&db_path).unwrap();

    let sql = "
    INSERT INTO mandates (
        mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total,
        spent_today, spent_total, last_spent_date, expires_at, created_at, status
    ) VALUES 
    ('m_active_01', 'user_rahul', 'zepto', 80000, 200000, 1000000, 150000, 150000, '2026-08-30', '2028-12-31T23:59:59Z', '2026-08-30T00:00:00Z', 'active'),
    ('m_expired_01', 'user_rahul', 'blinkit', 100000, 200000, 500000, 0, 0, '2026-08-30', '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z', 'expired'),
    ('m_revoked_01', 'user_rahul', 'swiggy', 100000, 200000, 500000, 0, 0, '2026-08-30', '2028-12-31T23:59:59Z', '2026-08-30T00:00:00Z', 'revoked'),
    ('m_old_date_01', 'user_priya', 'amazon', 500000, 500000, 1000000, 450000, 450000, '2020-01-01', '2028-12-31T23:59:59Z', '2026-08-30T00:00:00Z', 'active');";

    conn.execute(sql).unwrap();

    let mut cfg = Config::load();
    cfg.db_path = db_path.clone();
    (cfg, db_path)
}

#[test]
fn test_approved_transaction() {
    let (cfg, db_path) = setup_test_db("approved");
    let conn = DbConnection::open(&db_path).unwrap();

    let req = AuthorizeRequest {
        user_id: "user_rahul".to_string(),
        merchant_id: "zepto".to_string(),
        amount: 20000, // ₹200 (spent_today 150000 -> 170000)
        mandate_id: Some("m_active_01".to_string()),
    };

    let decision = authorize_transaction(&conn, &req, &cfg);
    assert_eq!(decision.decision, "approved");
    assert_eq!(decision.reason, "passed_all_checks");
    assert!(decision.razorpay_order_id.is_some());

    let logs = get_audit_logs(&conn, Some("m_active_01"), None, 10).unwrap();
    assert!(!logs.is_empty());
    assert_eq!(logs[0].decision, "approved");

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_max_per_txn_exceeded() {
    let (cfg, db_path) = setup_test_db("max_txn");
    let conn = DbConnection::open(&db_path).unwrap();

    let req = AuthorizeRequest {
        user_id: "user_rahul".to_string(),
        merchant_id: "zepto".to_string(),
        amount: 100000, // ₹1,000 (limit is ₹800)
        mandate_id: Some("m_active_01".to_string()),
    };

    let decision = authorize_transaction(&conn, &req, &cfg);
    assert_eq!(decision.decision, "denied");
    assert_eq!(decision.reason, "max_per_txn_exceeded");
    assert_eq!(decision.limit, Some(80000));

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_max_per_day_exceeded() {
    let (cfg, db_path) = setup_test_db("max_day");
    let conn = DbConnection::open(&db_path).unwrap();

    let req = AuthorizeRequest {
        user_id: "user_rahul".to_string(),
        merchant_id: "zepto".to_string(),
        amount: 70000, // ₹700 (spent_today is ₹1,500, max_per_day is ₹2,000 -> max remaining is ₹500)
        mandate_id: Some("m_active_01".to_string()),
    };

    let decision = authorize_transaction(&conn, &req, &cfg);
    assert_eq!(decision.decision, "denied");
    assert_eq!(decision.reason, "max_per_day_exceeded");
    assert_eq!(decision.limit, Some(200000));

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_merchant_mismatch() {
    let (cfg, db_path) = setup_test_db("merchant_mismatch");
    let conn = DbConnection::open(&db_path).unwrap();

    let req = AuthorizeRequest {
        user_id: "user_rahul".to_string(),
        merchant_id: "swiggy".to_string(), // Mandate is for zepto
        amount: 20000,
        mandate_id: Some("m_active_01".to_string()),
    };

    let decision = authorize_transaction(&conn, &req, &cfg);
    assert_eq!(decision.decision, "denied");
    assert_eq!(decision.reason, "merchant_mismatch");

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_user_mismatch() {
    let (cfg, db_path) = setup_test_db("user_mismatch");
    let conn = DbConnection::open(&db_path).unwrap();

    let req = AuthorizeRequest {
        user_id: "user_priya".to_string(), // Mandate belongs to user_rahul
        merchant_id: "zepto".to_string(),
        amount: 20000,
        mandate_id: Some("m_active_01".to_string()),
    };

    let decision = authorize_transaction(&conn, &req, &cfg);
    assert_eq!(decision.decision, "denied");
    assert_eq!(decision.reason, "user_mismatch");

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_expired_mandate() {
    let (cfg, db_path) = setup_test_db("expired");
    let conn = DbConnection::open(&db_path).unwrap();

    let req = AuthorizeRequest {
        user_id: "user_rahul".to_string(),
        merchant_id: "blinkit".to_string(),
        amount: 10000,
        mandate_id: Some("m_expired_01".to_string()),
    };

    let decision = authorize_transaction(&conn, &req, &cfg);
    assert_eq!(decision.decision, "denied");
    assert_eq!(decision.reason, "mandate_expired");

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_revoked_mandate() {
    let (cfg, db_path) = setup_test_db("revoked");
    let conn = DbConnection::open(&db_path).unwrap();

    let req = AuthorizeRequest {
        user_id: "user_rahul".to_string(),
        merchant_id: "swiggy".to_string(),
        amount: 10000,
        mandate_id: Some("m_revoked_01".to_string()),
    };

    let decision = authorize_transaction(&conn, &req, &cfg);
    assert_eq!(decision.decision, "denied");
    assert_eq!(decision.reason, "mandate_revoked");

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_daily_reset_logic() {
    let (cfg, db_path) = setup_test_db("daily_reset");
    let conn = DbConnection::open(&db_path).unwrap();

    let req = AuthorizeRequest {
        user_id: "user_priya".to_string(),
        merchant_id: "amazon".to_string(),
        amount: 400000, // ₹4,000 (spent ₹4,500 yesterday, daily reset grants full ₹5,000 cap)
        mandate_id: Some("m_old_date_01".to_string()),
    };

    let decision = authorize_transaction(&conn, &req, &cfg);
    assert_eq!(decision.decision, "approved");

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_concurrency_race_condition() {
    let (cfg, db_path) = setup_test_db("concurrency");
    let cfg_arc = Arc::new(cfg);
    let db_path_clone = db_path.clone();

    let cfg1 = Arc::clone(&cfg_arc);
    let path1 = db_path_clone.clone();
    let handle1 = thread::spawn(move || {
        let conn = DbConnection::open(&path1).unwrap();
        let req = AuthorizeRequest {
            user_id: "user_rahul".to_string(),
            merchant_id: "zepto".to_string(),
            amount: 40000,
            mandate_id: Some("m_active_01".to_string()),
        };
        authorize_transaction(&conn, &req, &cfg1)
    });

    let cfg2 = Arc::clone(&cfg_arc);
    let path2 = db_path_clone.clone();
    let handle2 = thread::spawn(move || {
        let conn = DbConnection::open(&path2).unwrap();
        let req = AuthorizeRequest {
            user_id: "user_rahul".to_string(),
            merchant_id: "zepto".to_string(),
            amount: 40000,
            mandate_id: Some("m_active_01".to_string()),
        };
        authorize_transaction(&conn, &req, &cfg2)
    });

    let res1 = handle1.join().unwrap();
    let res2 = handle2.join().unwrap();

    let approved_count = (if res1.decision == "approved" { 1 } else { 0 }) + (if res2.decision == "approved" { 1 } else { 0 });
    let denied_count = (if res1.decision == "denied" { 1 } else { 0 }) + (if res2.decision == "denied" { 1 } else { 0 });

    assert_eq!(approved_count, 1, "Exactly one parallel transaction must be approved");
    assert_eq!(denied_count, 1, "Exactly one parallel transaction must be denied");

    let _ = std::fs::remove_file(db_path);
}
