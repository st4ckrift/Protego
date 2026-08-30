import sqlite3
import json
import uuid
import logging
from datetime import datetime, timezone
from typing import List, Dict, Any, Optional
from gate.database import get_connection
from gate.models import AuditLogEntry

logger = logging.getLogger(__name__)

def log_audit_event(
    mandate_id: str,
    user_id: str,
    merchant_id: str,
    requested_amount: int,
    decision: str,
    reason: str,
    details: Dict[str, Any],
    razorpay_order_id: Optional[str] = None,
    conn: Optional[sqlite3.Connection] = None
) -> AuditLogEntry:
    """
    Inserts an immutable audit log entry into the database.
    """
    log_id = f"log_{uuid.uuid4().hex[:12]}"
    timestamp = datetime.now(timezone.utc).isoformat()
    
    close_conn_needed = False
    if conn is None:
        conn = get_connection()
        close_conn_needed = True

    try:
        cursor = conn.cursor()
        cursor.execute("""
        INSERT INTO audit_logs (
            log_id, timestamp, mandate_id, user_id, merchant_id,
            requested_amount, decision, reason, details_json, razorpay_order_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
        """, (
            log_id,
            timestamp,
            mandate_id,
            user_id,
            merchant_id,
            requested_amount,
            decision,
            reason,
            json.dumps(details),
            razorpay_order_id
        ))
        if close_conn_needed:
            conn.commit()
    finally:
        if close_conn_needed:
            conn.close()

    entry = AuditLogEntry(
        log_id=log_id,
        timestamp=timestamp,
        mandate_id=mandate_id,
        user_id=user_id,
        merchant_id=merchant_id,
        requested_amount=requested_amount,
        decision=decision,
        reason=reason,
        details=details,
        razorpay_order_id=razorpay_order_id
    )
    logger.info(f"Audit log created: [{decision.upper()}] mandate={mandate_id} reason={reason}")
    return entry

def get_audit_logs(
    mandate_id: Optional[str] = None,
    user_id: Optional[str] = None,
    merchant_id: Optional[str] = None,
    decision: Optional[str] = None,
    limit: int = 50
) -> List[Dict[str, Any]]:
    """
    Queries audit logs with optional filters.
    """
    conn = get_connection()
    try:
        cursor = conn.cursor()
        query = "SELECT * FROM audit_logs WHERE 1=1"
        params = []
        
        if mandate_id:
            query += " AND mandate_id = ?"
            params.append(mandate_id)
        if user_id:
            query += " AND user_id = ?"
            params.append(user_id)
        if merchant_id:
            query += " AND merchant_id = ?"
            params.append(merchant_id)
        if decision:
            query += " AND decision = ?"
            params.append(decision)
            
        query += " ORDER BY timestamp DESC LIMIT ?"
        params.append(limit)
        
        cursor.execute(query, params)
        rows = cursor.fetchall()
        
        result = []
        for row in rows:
            row_dict = dict(row)
            try:
                row_dict["details"] = json.loads(row_dict.pop("details_json"))
            except Exception:
                row_dict["details"] = {}
            result.append(row_dict)
        return result
    finally:
        conn.close()
