import sqlite3
import json
import logging
from datetime import datetime, date
from typing import Optional, List, Dict, Any, Tuple
import gate.config

logger = logging.getLogger(__name__)

DB_PATH = gate.config.DB_PATH

def get_connection(db_path: Optional[str] = None) -> sqlite3.Connection:
    if db_path is None:
        db_path = DB_PATH
    conn = sqlite3.connect(db_path, timeout=10.0, check_same_thread=False)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL;")
    conn.execute("PRAGMA foreign_keys=ON;")
    return conn

def init_db(db_path: Optional[str] = None):
    if db_path is None:
        db_path = DB_PATH
    conn = get_connection(db_path)
    cursor = conn.cursor()
    
    # Mandates table
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS mandates (
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
    );
    """)
    
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_mandates_user_merchant ON mandates(user_id, merchant_id);")

    # Audit Logs table
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS audit_logs (
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
    );
    """)
    
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_audit_mandate ON audit_logs(mandate_id);")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_logs(timestamp DESC);")
    
    conn.commit()
    conn.close()
    logger.info(f"Database initialized at {db_path}")
