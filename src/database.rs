use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use crate::models::{MandateModel, MandateStatus, AuditLogEntry};

#[repr(C)]
struct Sqlite3Opaque {
    _private: [u8; 0],
}

#[repr(C)]
struct Sqlite3StmtOpaque {
    _private: [u8; 0],
}

type Sqlite3 = Sqlite3Opaque;
type Sqlite3Stmt = Sqlite3StmtOpaque;

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_OPEN_READWRITE: c_int = 0x00000002;
const SQLITE_OPEN_CREATE: c_int = 0x00000004;

#[link(name = "sqlite3")]
extern "C" {
    fn sqlite3_open_v2(
        filename: *const c_char,
        ppDb: *mut *mut Sqlite3,
        flags: c_int,
        zVfs: *const c_char,
    ) -> c_int;
    fn sqlite3_close(db: *mut Sqlite3) -> c_int;
    fn sqlite3_exec(
        db: *mut Sqlite3,
        sql: *const c_char,
        callback: Option<extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int>,
        arg: *mut c_void,
        errmsg: *mut *mut c_char,
    ) -> c_int;
    fn sqlite3_prepare_v2(
        db: *mut Sqlite3,
        zSql: *const c_char,
        nByte: c_int,
        ppStmt: *mut *mut Sqlite3Stmt,
        pzTail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_step(stmt: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_finalize(stmt: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_bind_text(
        stmt: *mut Sqlite3Stmt,
        index: c_int,
        val: *const c_char,
        n: c_int,
        destructor: Option<extern "C" fn(*mut c_void)>,
    ) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut Sqlite3Stmt, index: c_int, val: i64) -> c_int;
    fn sqlite3_column_text(stmt: *mut Sqlite3Stmt, col: c_int) -> *const u8;
    fn sqlite3_column_int64(stmt: *mut Sqlite3Stmt, col: c_int) -> i64;
    fn sqlite3_errmsg(db: *mut Sqlite3) -> *const c_char;
    fn sqlite3_free(p: *mut c_void);
}

pub struct DbConnection {
    db: *mut Sqlite3,
}

unsafe impl Send for DbConnection {}
unsafe impl Sync for DbConnection {}

impl DbConnection {
    pub fn open(path: &str) -> Result<Self, String> {
        let c_path = CString::new(path).map_err(|e| e.to_string())?;
        let mut db: *mut Sqlite3 = ptr::null_mut();
        let flags = SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE;
        let rc = unsafe { sqlite3_open_v2(c_path.as_ptr(), &mut db, flags, ptr::null()) };
        if rc != SQLITE_OK {
            let err = if !db.is_null() {
                let msg = unsafe { CStr::from_ptr(sqlite3_errmsg(db)).to_string_lossy().into_owned() };
                unsafe { sqlite3_close(db); }
                msg
            } else {
                format!("Failed to open SQLite database at {}", path)
            };
            return Err(err);
        }
        let conn = DbConnection { db };
        conn.execute("PRAGMA journal_mode=WAL;")?;
        conn.execute("PRAGMA foreign_keys=ON;")?;
        conn.execute("PRAGMA busy_timeout=5000;")?;
        Ok(conn)
    }

    pub fn execute(&self, sql: &str) -> Result<(), String> {
        let c_sql = CString::new(sql).map_err(|e| e.to_string())?;
        let mut err_ptr: *mut c_char = ptr::null_mut();
        let rc = unsafe { sqlite3_exec(self.db, c_sql.as_ptr(), None, ptr::null_mut(), &mut err_ptr) };
        if rc != SQLITE_OK {
            let msg = if !err_ptr.is_null() {
                let m = unsafe { CStr::from_ptr(err_ptr).to_string_lossy().into_owned() };
                unsafe { sqlite3_free(err_ptr as *mut c_void); }
                m
            } else {
                "Unknown SQLite error".to_string()
            };
            return Err(msg);
        }
        Ok(())
    }

    pub fn begin_immediate(&self) -> Result<(), String> {
        self.execute("BEGIN IMMEDIATE;")
    }

    pub fn commit(&self) -> Result<(), String> {
        self.execute("COMMIT;")
    }

    pub fn rollback(&self) -> Result<(), String> {
        self.execute("ROLLBACK;")
    }

    pub fn query_mandate_by_id(&self, mandate_id: &str) -> Result<Option<MandateModel>, String> {
        let sql = "SELECT mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total, spent_today, spent_total, last_spent_date, expires_at, created_at, status FROM mandates WHERE mandate_id = ?;";
        let mut stmt: *mut Sqlite3Stmt = ptr::null_mut();
        let c_sql = CString::new(sql).unwrap();
        let rc = unsafe { sqlite3_prepare_v2(self.db, c_sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        if rc != SQLITE_OK {
            return Err(self.last_error());
        }

        let c_id = CString::new(mandate_id).unwrap();
        unsafe { sqlite3_bind_text(stmt, 1, c_id.as_ptr(), -1, None); }

        let step_rc = unsafe { sqlite3_step(stmt) };
        let result = if step_rc == SQLITE_ROW {
            let m = parse_mandate_row(stmt);
            Some(m)
        } else {
            None
        };

        unsafe { sqlite3_finalize(stmt); }
        Ok(result)
    }

    pub fn query_active_mandate(&self, user_id: &str, merchant_id: &str) -> Result<Option<MandateModel>, String> {
        let sql = "SELECT mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total, spent_today, spent_total, last_spent_date, expires_at, created_at, status FROM mandates WHERE user_id = ? AND merchant_id = ? AND status = 'active' ORDER BY created_at DESC LIMIT 1;";
        let mut stmt: *mut Sqlite3Stmt = ptr::null_mut();
        let c_sql = CString::new(sql).unwrap();
        let rc = unsafe { sqlite3_prepare_v2(self.db, c_sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        if rc != SQLITE_OK {
            return Err(self.last_error());
        }

        let c_user = CString::new(user_id).unwrap();
        let c_merch = CString::new(merchant_id).unwrap();
        unsafe {
            sqlite3_bind_text(stmt, 1, c_user.as_ptr(), -1, None);
            sqlite3_bind_text(stmt, 2, c_merch.as_ptr(), -1, None);
        }

        let step_rc = unsafe { sqlite3_step(stmt) };
        let result = if step_rc == SQLITE_ROW {
            let m = parse_mandate_row(stmt);
            Some(m)
        } else {
            None
        };

        unsafe { sqlite3_finalize(stmt); }
        Ok(result)
    }

    pub fn query_all_mandates(&self) -> Result<Vec<MandateModel>, String> {
        let sql = "SELECT mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total, spent_today, spent_total, last_spent_date, expires_at, created_at, status FROM mandates ORDER BY created_at DESC;";
        let mut stmt: *mut Sqlite3Stmt = ptr::null_mut();
        let c_sql = CString::new(sql).unwrap();
        let rc = unsafe { sqlite3_prepare_v2(self.db, c_sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        if rc != SQLITE_OK {
            return Err(self.last_error());
        }

        let mut list = Vec::new();
        while unsafe { sqlite3_step(stmt) } == SQLITE_ROW {
            list.push(parse_mandate_row(stmt));
        }

        unsafe { sqlite3_finalize(stmt); }
        Ok(list)
    }

    pub fn update_mandate_spend(
        &self,
        mandate_id: &str,
        spent_today: i64,
        spent_total: i64,
        last_spent_date: &str,
    ) -> Result<(), String> {
        let sql = "UPDATE mandates SET spent_today = ?, spent_total = ?, last_spent_date = ? WHERE mandate_id = ?;";
        let mut stmt: *mut Sqlite3Stmt = ptr::null_mut();
        let c_sql = CString::new(sql).unwrap();
        if unsafe { sqlite3_prepare_v2(self.db, c_sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) } != SQLITE_OK {
            return Err(self.last_error());
        }

        let c_date = CString::new(last_spent_date).unwrap();
        let c_id = CString::new(mandate_id).unwrap();
        unsafe {
            sqlite3_bind_int64(stmt, 1, spent_today);
            sqlite3_bind_int64(stmt, 2, spent_total);
            sqlite3_bind_text(stmt, 3, c_date.as_ptr(), -1, None);
            sqlite3_bind_text(stmt, 4, c_id.as_ptr(), -1, None);
            sqlite3_step(stmt);
            sqlite3_finalize(stmt);
        }
        Ok(())
    }

    pub fn update_mandate_status(&self, mandate_id: &str, status: &str) -> Result<(), String> {
        let sql = "UPDATE mandates SET status = ? WHERE mandate_id = ?;";
        let mut stmt: *mut Sqlite3Stmt = ptr::null_mut();
        let c_sql = CString::new(sql).unwrap();
        if unsafe { sqlite3_prepare_v2(self.db, c_sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) } != SQLITE_OK {
            return Err(self.last_error());
        }

        let c_stat = CString::new(status).unwrap();
        let c_id = CString::new(mandate_id).unwrap();
        unsafe {
            sqlite3_bind_text(stmt, 1, c_stat.as_ptr(), -1, None);
            sqlite3_bind_text(stmt, 2, c_id.as_ptr(), -1, None);
            sqlite3_step(stmt);
            sqlite3_finalize(stmt);
        }
        Ok(())
    }

    pub fn insert_audit_log(&self, entry: &AuditLogEntry) -> Result<(), String> {
        let sql = "INSERT INTO audit_logs (log_id, timestamp, mandate_id, user_id, merchant_id, requested_amount, decision, reason, details_json, razorpay_order_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);";
        let mut stmt: *mut Sqlite3Stmt = ptr::null_mut();
        let c_sql = CString::new(sql).unwrap();
        if unsafe { sqlite3_prepare_v2(self.db, c_sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) } != SQLITE_OK {
            return Err(self.last_error());
        }

        let c_log_id = CString::new(entry.log_id.as_str()).unwrap();
        let c_ts = CString::new(entry.timestamp.as_str()).unwrap();
        let c_mandate_id = CString::new(entry.mandate_id.as_str()).unwrap();
        let c_user_id = CString::new(entry.user_id.as_str()).unwrap();
        let c_merchant_id = CString::new(entry.merchant_id.as_str()).unwrap();
        let c_dec = CString::new(entry.decision.as_str()).unwrap();
        let c_reason = CString::new(entry.reason.as_str()).unwrap();
        let c_details = CString::new(entry.details_json.as_str()).unwrap();

        unsafe {
            sqlite3_bind_text(stmt, 1, c_log_id.as_ptr(), -1, None);
            sqlite3_bind_text(stmt, 2, c_ts.as_ptr(), -1, None);
            sqlite3_bind_text(stmt, 3, c_mandate_id.as_ptr(), -1, None);
            sqlite3_bind_text(stmt, 4, c_user_id.as_ptr(), -1, None);
            sqlite3_bind_text(stmt, 5, c_merchant_id.as_ptr(), -1, None);
            sqlite3_bind_int64(stmt, 6, entry.requested_amount);
            sqlite3_bind_text(stmt, 7, c_dec.as_ptr(), -1, None);
            sqlite3_bind_text(stmt, 8, c_reason.as_ptr(), -1, None);
            sqlite3_bind_text(stmt, 9, c_details.as_ptr(), -1, None);

            if let Some(ref rzp_id) = entry.razorpay_order_id {
                let c_rzp = CString::new(rzp_id.as_str()).unwrap();
                sqlite3_bind_text(stmt, 10, c_rzp.as_ptr(), -1, None);
            } else {
                sqlite3_bind_text(stmt, 10, ptr::null(), -1, None);
            }

            sqlite3_step(stmt);
            sqlite3_finalize(stmt);
        }
        Ok(())
    }

    pub fn query_audit_logs(
        &self,
        mandate_id: Option<&str>,
        decision: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AuditLogEntry>, String> {
        let mut sql = "SELECT log_id, timestamp, mandate_id, user_id, merchant_id, requested_amount, decision, reason, details_json, razorpay_order_id FROM audit_logs WHERE 1=1".to_string();
        if mandate_id.is_some() {
            sql.push_str(" AND mandate_id = ?");
        }
        if decision.is_some() {
            sql.push_str(" AND decision = ?");
        }
        sql.push_str(" ORDER BY timestamp DESC LIMIT ?;");

        let mut stmt: *mut Sqlite3Stmt = ptr::null_mut();
        let c_sql = CString::new(sql.as_str()).unwrap();
        if unsafe { sqlite3_prepare_v2(self.db, c_sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) } != SQLITE_OK {
            return Err(self.last_error());
        }

        let mut param_idx = 1;
        let _c_mandate = mandate_id.map(|m| CString::new(m).unwrap());
        let _c_dec = decision.map(|d| CString::new(d).unwrap());

        unsafe {
            if let Some(ref c_m) = _c_mandate {
                sqlite3_bind_text(stmt, param_idx, c_m.as_ptr(), -1, None);
                param_idx += 1;
            }
            if let Some(ref c_d) = _c_dec {
                sqlite3_bind_text(stmt, param_idx, c_d.as_ptr(), -1, None);
                param_idx += 1;
            }
            sqlite3_bind_int64(stmt, param_idx, limit as i64);
        }

        let mut list = Vec::new();
        while unsafe { sqlite3_step(stmt) } == SQLITE_ROW {
            let log_id = get_col_text(stmt, 0);
            let timestamp = get_col_text(stmt, 1);
            let m_id = get_col_text(stmt, 2);
            let user_id = get_col_text(stmt, 3);
            let merch_id = get_col_text(stmt, 4);
            let amount = unsafe { sqlite3_column_int64(stmt, 5) };
            let dec = get_col_text(stmt, 6);
            let reason = get_col_text(stmt, 7);
            let details = get_col_text(stmt, 8);
            let rzp_id = get_opt_col_text(stmt, 9);

            list.push(AuditLogEntry {
                log_id,
                timestamp,
                mandate_id: m_id,
                user_id,
                merchant_id: merch_id,
                requested_amount: amount,
                decision: dec,
                reason,
                details_json: details,
                razorpay_order_id: rzp_id,
            });
        }

        unsafe { sqlite3_finalize(stmt); }
        Ok(list)
    }

    fn last_error(&self) -> String {
        unsafe { CStr::from_ptr(sqlite3_errmsg(self.db)).to_string_lossy().into_owned() }
    }
}

impl Drop for DbConnection {
    fn drop(&mut self) {
        if !self.db.is_null() {
            unsafe { sqlite3_close(self.db); }
        }
    }
}

fn parse_mandate_row(stmt: *mut Sqlite3Stmt) -> MandateModel {
    let mandate_id = get_col_text(stmt, 0);
    let user_id = get_col_text(stmt, 1);
    let merchant_id = get_col_text(stmt, 2);
    let max_per_txn = unsafe { sqlite3_column_int64(stmt, 3) };
    let max_per_day = unsafe { sqlite3_column_int64(stmt, 4) };
    let max_total = unsafe { sqlite3_column_int64(stmt, 5) };
    let spent_today = unsafe { sqlite3_column_int64(stmt, 6) };
    let spent_total = unsafe { sqlite3_column_int64(stmt, 7) };
    let last_spent_date = get_col_text(stmt, 8);
    let expires_at = get_col_text(stmt, 9);
    let created_at = get_col_text(stmt, 10);
    let status_str = get_col_text(stmt, 11);

    MandateModel {
        mandate_id,
        user_id,
        merchant_id,
        max_per_txn,
        max_per_day,
        max_total,
        spent_today,
        spent_total,
        last_spent_date,
        expires_at,
        created_at,
        status: MandateStatus::from_str(&status_str),
    }
}

fn get_col_text(stmt: *mut Sqlite3Stmt, col: c_int) -> String {
    unsafe {
        let ptr = sqlite3_column_text(stmt, col);
        if ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(ptr as *const c_char).to_string_lossy().into_owned()
        }
    }
}

fn get_opt_col_text(stmt: *mut Sqlite3Stmt, col: c_int) -> Option<String> {
    unsafe {
        let ptr = sqlite3_column_text(stmt, col);
        if ptr.is_null() {
            None
        } else {
            let s = CStr::from_ptr(ptr as *const c_char).to_string_lossy().into_owned();
            if s.is_empty() { None } else { Some(s) }
        }
    }
}

pub fn init_db(db_path: &str) -> Result<(), String> {
    let conn = DbConnection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS mandates (
            mandate_id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            merchant_id TEXT NOT NULL,
            max_per_txn INTEGER NOT NULL,
            max_per_day INTEGER NOT NULL,
            max_total INTEGER NOT NULL,
            spent_today INTEGER NOT NULL DEFAULT 0,
            spent_total INTEGER NOT NULL DEFAULT 0,
            last_spent_date TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active'
        );",
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_mandates_user_merchant ON mandates(user_id, merchant_id);")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS audit_logs (
            log_id TEXT PRIMARY KEY,
            timestamp TEXT NOT NULL,
            mandate_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            merchant_id TEXT NOT NULL,
            requested_amount INTEGER NOT NULL,
            decision TEXT NOT NULL,
            reason TEXT NOT NULL,
            details_json TEXT NOT NULL,
            razorpay_order_id TEXT
        );",
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_audit_mandate ON audit_logs(mandate_id);")?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_logs(timestamp DESC);")?;
    Ok(())
}
