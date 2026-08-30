import sqlite3
import json
import logging
from datetime import datetime, date, timezone
from typing import Optional, Dict, Any, List
from gate.database import get_connection
from gate.models import AuthorizeRequest, GateDecision, MandateModel
from gate.audit_service import log_audit_event
from gate.razorpay_client import create_razorpay_order, RazorpayAPIError

logger = logging.getLogger(__name__)

def parse_iso_datetime(dt_str: str) -> datetime:
    """Parses ISO format string to UTC datetime."""
    try:
        if dt_str.endswith("Z"):
            dt_str = dt_str[:-1] + "+00:00"
        return datetime.fromisoformat(dt_str)
    except Exception:
        return datetime.now(timezone.utc)

def format_inr(paise: int) -> str:
    """Formats paise integer into INR string, e.g., 45000 -> ₹450.00"""
    rupees = paise / 100.0
    return f"₹{rupees:,.2f}"

def authorize_transaction(req: AuthorizeRequest) -> GateDecision:
    """
    Core Gate decision function executing the 5 strict sequential mandate checks.
    Uses SQLite BEGIN IMMEDIATE transaction for atomic limit checks and updates.
    """
    conn = get_connection()
    today_str = date.today().isoformat()
    now_utc = datetime.now(timezone.utc)
    
    try:
        # Begin exclusive immediate lock for thread-safe concurrency control
        conn.execute("BEGIN IMMEDIATE")
        cursor = conn.cursor()

        # Step 1: Find Mandate
        if req.mandate_id:
            cursor.execute("SELECT * FROM mandates WHERE mandate_id = ?", (req.mandate_id,))
            row = cursor.fetchone()
            if not row:
                reason = "mandate_not_found"
                msg = f"Mandate '{req.mandate_id}' was not found in the database."
                log_audit_event(
                    mandate_id=req.mandate_id or "unknown",
                    user_id=req.user_id,
                    merchant_id=req.merchant_id,
                    requested_amount=req.amount,
                    decision="denied",
                    reason=reason,
                    details={"requested_mandate_id": req.mandate_id, "message": msg},
                    conn=conn
                )
                conn.commit()
                return GateDecision(
                    decision="denied",
                    reason=reason,
                    mandate_id=req.mandate_id,
                    user_id=req.user_id,
                    merchant_id=req.merchant_id,
                    requested_amount=req.amount,
                    message=msg
                )
            mandate_dict = dict(row)
        else:
            # Auto-lookup active mandate for user_id + merchant_id
            cursor.execute(
                "SELECT * FROM mandates WHERE user_id = ? AND merchant_id = ? AND status = 'active' ORDER BY created_at DESC LIMIT 1",
                (req.user_id, req.merchant_id)
            )
            row = cursor.fetchone()
            if not row:
                reason = "mandate_not_found"
                msg = f"No active mandate found for user '{req.user_id}' at merchant '{req.merchant_id}'."
                log_audit_event(
                    mandate_id="none",
                    user_id=req.user_id,
                    merchant_id=req.merchant_id,
                    requested_amount=req.amount,
                    decision="denied",
                    reason=reason,
                    details={"message": msg},
                    conn=conn
                )
                conn.commit()
                return GateDecision(
                    decision="denied",
                    reason=reason,
                    mandate_id=None,
                    user_id=req.user_id,
                    merchant_id=req.merchant_id,
                    requested_amount=req.amount,
                    message=msg
                )
            mandate_dict = dict(row)

        mandate_id = mandate_dict["mandate_id"]

        # Check user_id matching
        if mandate_dict["user_id"] != req.user_id:
            reason = "user_mismatch"
            msg = f"Mandate '{mandate_id}' belongs to user '{mandate_dict['user_id']}', but request was for user '{req.user_id}'."
            log_audit_event(
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                decision="denied",
                reason=reason,
                details={"mandate_user": mandate_dict["user_id"], "requested_user": req.user_id},
                conn=conn
            )
            conn.commit()
            return GateDecision(
                decision="denied",
                reason=reason,
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                message=msg
            )

        # Check merchant_id matching
        if mandate_dict["merchant_id"] != req.merchant_id:
            reason = "merchant_mismatch"
            msg = f"Mandate '{mandate_id}' is scoped to merchant '{mandate_dict['merchant_id']}', but request attempted transaction at merchant '{req.merchant_id}'."
            log_audit_event(
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                decision="denied",
                reason=reason,
                details={"mandate_merchant": mandate_dict["merchant_id"], "requested_merchant": req.merchant_id},
                conn=conn
            )
            conn.commit()
            return GateDecision(
                decision="denied",
                reason=reason,
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                message=msg
            )

        # Step 2: Active / Expiry check
        if mandate_dict["status"] == "revoked":
            reason = "mandate_revoked"
            msg = f"Mandate '{mandate_id}' has been revoked by the user."
            log_audit_event(
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                decision="denied",
                reason=reason,
                details={"status": "revoked"},
                conn=conn
            )
            conn.commit()
            return GateDecision(
                decision="denied",
                reason=reason,
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                message=msg
            )

        expiry_dt = parse_iso_datetime(mandate_dict["expires_at"])
        if mandate_dict["status"] == "expired" or now_utc > expiry_dt:
            # Mark expired in DB
            cursor.execute("UPDATE mandates SET status = 'expired' WHERE mandate_id = ?", (mandate_id,))
            reason = "mandate_expired"
            msg = f"Mandate '{mandate_id}' expired at {mandate_dict['expires_at']}."
            log_audit_event(
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                decision="denied",
                reason=reason,
                details={"expires_at": mandate_dict["expires_at"]},
                conn=conn
            )
            conn.commit()
            return GateDecision(
                decision="denied",
                reason=reason,
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                message=msg
            )

        # Automatic Daily Reset Check
        spent_today = mandate_dict["spent_today"]
        if mandate_dict["last_spent_date"] != today_str:
            spent_today = 0

        max_per_txn = mandate_dict["max_per_txn"]
        max_per_day = mandate_dict["max_per_day"]
        max_total = mandate_dict["max_total"]
        spent_total = mandate_dict["spent_total"]

        # Step 3: Per-transaction limit check
        if req.amount > max_per_txn:
            reason = "max_per_txn_exceeded"
            msg = (
                f"Transaction denied: Requested amount {format_inr(req.amount)} "
                f"exceeds single transaction limit {format_inr(max_per_txn)}."
            )
            log_audit_event(
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                decision="denied",
                reason=reason,
                details={
                    "limit": max_per_txn,
                    "requested": req.amount,
                    "max_per_txn": max_per_txn
                },
                conn=conn
            )
            conn.commit()
            return GateDecision(
                decision="denied",
                reason=reason,
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                limit=max_per_txn,
                already_spent=0,
                message=msg
            )

        # Step 4: Daily cap limit check
        if spent_today + req.amount > max_per_day:
            reason = "max_per_day_exceeded"
            remaining_today = max_per_day - spent_today
            msg = (
                f"Transaction denied: Requested {format_inr(req.amount)} exceeds remaining daily cap "
                f"{format_inr(max(0, remaining_today))} (spent {format_inr(spent_today)} / {format_inr(max_per_day)} today)."
            )
            log_audit_event(
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                decision="denied",
                reason=reason,
                details={
                    "limit": max_per_day,
                    "already_spent": spent_today,
                    "requested": req.amount,
                    "remaining": max(0, remaining_today)
                },
                conn=conn
            )
            conn.commit()
            return GateDecision(
                decision="denied",
                reason=reason,
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                limit=max_per_day,
                already_spent=spent_today,
                message=msg
            )

        # Step 5: Total lifetime limit check
        if spent_total + req.amount > max_total:
            reason = "max_total_exceeded"
            remaining_total = max_total - spent_total
            msg = (
                f"Transaction denied: Requested {format_inr(req.amount)} exceeds remaining total mandate cap "
                f"{format_inr(max(0, remaining_total))} (spent {format_inr(spent_total)} / {format_inr(max_total)} total)."
            )
            log_audit_event(
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                decision="denied",
                reason=reason,
                details={
                    "limit": max_total,
                    "already_spent": spent_total,
                    "requested": req.amount,
                    "remaining": max(0, remaining_total)
                },
                conn=conn
            )
            conn.commit()
            return GateDecision(
                decision="denied",
                reason=reason,
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                limit=max_total,
                already_spent=spent_total,
                message=msg
            )

        # ALL CHECKS PASSED -> Attempt Razorpay order creation
        try:
            rzp_order_id, rzp_resp = create_razorpay_order(
                amount=req.amount,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                mandate_id=mandate_id
            )
        except RazorpayAPIError as rzp_err:
            # Handle Razorpay API failure gracefully
            conn.rollback()  # Do not deduct mandate balance if Razorpay API fails!
            reason = "razorpay_api_error"
            msg = f"Razorpay API Error: {rzp_err.message}"
            log_audit_event(
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                decision="error",
                reason=reason,
                details={"error_code": rzp_err.status_code, "error_message": rzp_err.message}
            )
            return GateDecision(
                decision="error",
                reason=reason,
                mandate_id=mandate_id,
                user_id=req.user_id,
                merchant_id=req.merchant_id,
                requested_amount=req.amount,
                message=msg
            )

        # ATOMIC METRIC UPDATE IN DB
        new_spent_today = spent_today + req.amount
        new_spent_total = spent_total + req.amount

        cursor.execute("""
        UPDATE mandates
        SET spent_today = ?, spent_total = ?, last_spent_date = ?
        WHERE mandate_id = ?
        """, (new_spent_today, new_spent_total, today_str, mandate_id))

        updated_mandate_dict = {
            "mandate_id": mandate_id,
            "user_id": req.user_id,
            "merchant_id": req.merchant_id,
            "max_per_txn": max_per_txn,
            "max_per_day": max_per_day,
            "max_total": max_total,
            "spent_today": new_spent_today,
            "spent_total": new_spent_total,
            "last_spent_date": today_str,
            "expires_at": mandate_dict["expires_at"],
            "status": mandate_dict["status"]
        }

        # LOG IMMUTABLE AUDIT EVENT FOR APPROVED TRANSACTION
        log_audit_event(
            mandate_id=mandate_id,
            user_id=req.user_id,
            merchant_id=req.merchant_id,
            requested_amount=req.amount,
            decision="approved",
            reason="passed_all_checks",
            details={
                "max_per_txn": max_per_txn,
                "max_per_day": max_per_day,
                "max_total": max_total,
                "previous_spent_today": spent_today,
                "new_spent_today": new_spent_today,
                "previous_spent_total": spent_total,
                "new_spent_total": new_spent_total,
                "razorpay_order": rzp_resp
            },
            razorpay_order_id=rzp_order_id,
            conn=conn
        )

        conn.commit()

        success_msg = (
            f"Transaction approved: {format_inr(req.amount)} charged under mandate '{mandate_id}'. "
            f"Razorpay Order ID: {rzp_order_id}."
        )

        return GateDecision(
            decision="approved",
            reason="passed_all_checks",
            mandate_id=mandate_id,
            user_id=req.user_id,
            merchant_id=req.merchant_id,
            requested_amount=req.amount,
            razorpay_order_id=rzp_order_id,
            updated_mandate=updated_mandate_dict,
            message=success_msg
        )

    except Exception as exc:
        conn.rollback()
        logger.error(f"Unexpected error in gate authorization: {exc}", exc_info=True)
        return GateDecision(
            decision="error",
            reason="internal_server_error",
            mandate_id=req.mandate_id,
            user_id=req.user_id,
            merchant_id=req.merchant_id,
            requested_amount=req.amount,
            message=f"Internal gate error: {str(exc)}"
        )
    finally:
        conn.close()

def get_all_mandates() -> List[Dict[str, Any]]:
    """Returns all mandates with calculated utilization percentages."""
    conn = get_connection()
    today_str = date.today().isoformat()
    try:
        cursor = conn.cursor()
        cursor.execute("SELECT * FROM mandates ORDER BY created_at DESC")
        rows = cursor.fetchall()
        result = []
        for row in rows:
            m = dict(row)
            # Adjust spent_today if date changed
            spent_today = m["spent_today"] if m["last_spent_date"] == today_str else 0
            
            day_util = round((spent_today / m["max_per_day"]) * 100, 1) if m["max_per_day"] > 0 else 0
            total_util = round((m["spent_total"] / m["max_total"]) * 100, 1) if m["max_total"] > 0 else 0
            
            m["spent_today_current"] = spent_today
            m["daily_utilization_pct"] = day_util
            m["total_utilization_pct"] = total_util
            result.append(m)
        return result
    finally:
        conn.close()
