import unittest
import sqlite3
import os
import sys
import tempfile
from datetime import datetime, date, timezone
from pathlib import Path

# Ensure root directory is in python path
BASE_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(BASE_DIR))

import gate.database
from gate.database import init_db, get_connection
from gate.models import AuthorizeRequest, GateDecision
from gate.gate_service import authorize_transaction
from gate.audit_service import get_audit_logs
from gate.razorpay_client import RazorpayAPIError
from unittest.mock import patch

class TestSpendGateLogic(unittest.TestCase):

    def setUp(self):
        # Create a temporary SQLite database for isolated unit tests
        self.db_fd, self.db_path = tempfile.mkstemp(suffix=".db")
        os.close(self.db_fd)
        
        # Override DB_PATH in gate.database
        gate.database.DB_PATH = self.db_path
        init_db(self.db_path)
        
        # Insert test mandates
        conn = get_connection(self.db_path)
        cursor = conn.cursor()
        today_str = date.today().isoformat()

        cursor.executemany("""
        INSERT INTO mandates (
            mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total,
            spent_today, spent_total, last_spent_date, expires_at, created_at, status
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
        """, [
            (
                "m_active_01",
                "user_rahul",
                "zepto",
                80000,    # max ₹800 per txn
                200000,   # max ₹2,000 per day
                1000000,  # max ₹10,000 total
                150000,   # spent ₹1,500 today (leaving ₹500 remaining)
                150000,   # spent ₹1,500 total
                today_str,
                "2028-12-31T23:59:59Z",
                datetime.now(timezone.utc).isoformat(),
                "active"
            ),
            (
                "m_expired_01",
                "user_rahul",
                "blinkit",
                100000,
                200000,
                500000,
                0,
                0,
                today_str,
                "2020-01-01T00:00:00Z", # expired
                "2020-01-01T00:00:00Z",
                "expired"
            ),
            (
                "m_revoked_01",
                "user_rahul",
                "swiggy",
                100000,
                200000,
                500000,
                0,
                0,
                today_str,
                "2028-12-31T23:59:59Z",
                datetime.now(timezone.utc).isoformat(),
                "revoked"
            ),
            (
                "m_old_date_01",
                "user_priya",
                "amazon",
                500000,   # max ₹5,000 per txn
                500000,   # max ₹5,000 per day
                1000000,
                450000,   # spent ₹4,500 YESTERDAY
                450000,
                "2020-01-01", # Past date for daily reset test
                "2028-12-31T23:59:59Z",
                datetime.now(timezone.utc).isoformat(),
                "active"
            )
        ])
        conn.commit()
        conn.close()

    def tearDown(self):
        if os.path.exists(self.db_path):
            os.remove(self.db_path)

    def test_approved_transaction(self):
        """Test valid transaction under all caps passes and updates counters atomically."""
        req = AuthorizeRequest(
            user_id="user_rahul",
            merchant_id="zepto",
            amount=20000, # ₹200 (spent_today becomes ₹1,700)
            mandate_id="m_active_01"
        )
        decision = authorize_transaction(req)
        
        self.assertEqual(decision.decision, "approved")
        self.assertEqual(decision.reason, "passed_all_checks")
        self.assertIsNotNone(decision.razorpay_order_id)
        self.assertEqual(decision.updated_mandate["spent_today"], 170000)
        self.assertEqual(decision.updated_mandate["spent_total"], 170000)

        # Verify audit log
        logs = get_audit_logs(mandate_id="m_active_01")
        self.assertGreaterEqual(len(logs), 1)
        self.assertEqual(logs[0]["decision"], "approved")
        self.assertEqual(logs[0]["razorpay_order_id"], decision.razorpay_order_id)

    def test_max_per_txn_exceeded(self):
        """Test requesting amount greater than single transaction cap is denied."""
        req = AuthorizeRequest(
            user_id="user_rahul",
            merchant_id="zepto",
            amount=100000, # ₹1,000 (cap is ₹800)
            mandate_id="m_active_01"
        )
        decision = authorize_transaction(req)
        
        self.assertEqual(decision.decision, "denied")
        self.assertEqual(decision.reason, "max_per_txn_exceeded")
        self.assertEqual(decision.limit, 80000)

    def test_max_per_day_exceeded(self):
        """Test requesting amount exceeding remaining daily cap is denied."""
        req = AuthorizeRequest(
            user_id="user_rahul",
            merchant_id="zepto",
            amount=70000, # ₹700 (under max_per_txn of ₹800, but spent_today is ₹1,500 and max_per_day is ₹2,000 -> remaining is ₹500)
            mandate_id="m_active_01"
        )
        decision = authorize_transaction(req)
        
        self.assertEqual(decision.decision, "denied")
        self.assertEqual(decision.reason, "max_per_day_exceeded")
        self.assertEqual(decision.limit, 200000)
        self.assertEqual(decision.already_spent, 150000)

    def test_merchant_mismatch(self):
        """Test transaction at wrong merchant is denied."""
        req = AuthorizeRequest(
            user_id="user_rahul",
            merchant_id="swiggy", # Mandate is for zepto
            amount=20000,
            mandate_id="m_active_01"
        )
        decision = authorize_transaction(req)
        
        self.assertEqual(decision.decision, "denied")
        self.assertEqual(decision.reason, "merchant_mismatch")

    def test_user_mismatch(self):
        """Test transaction for wrong user is denied."""
        req = AuthorizeRequest(
            user_id="user_priya", # Mandate belongs to user_rahul
            merchant_id="zepto",
            amount=20000,
            mandate_id="m_active_01"
        )
        decision = authorize_transaction(req)
        
        self.assertEqual(decision.decision, "denied")
        self.assertEqual(decision.reason, "user_mismatch")

    def test_expired_mandate(self):
        """Test transaction against expired mandate is denied."""
        req = AuthorizeRequest(
            user_id="user_rahul",
            merchant_id="blinkit",
            amount=10000,
            mandate_id="m_expired_01"
        )
        decision = authorize_transaction(req)
        
        self.assertEqual(decision.decision, "denied")
        self.assertEqual(decision.reason, "mandate_expired")

    def test_revoked_mandate(self):
        """Test transaction against revoked mandate is denied."""
        req = AuthorizeRequest(
            user_id="user_rahul",
            merchant_id="swiggy",
            amount=10000,
            mandate_id="m_revoked_01"
        )
        decision = authorize_transaction(req)
        
        self.assertEqual(decision.decision, "denied")
        self.assertEqual(decision.reason, "mandate_revoked")

    def test_daily_reset_logic(self):
        """Test that daily spend counter resets to 0 when current date > last_spent_date."""
        req = AuthorizeRequest(
            user_id="user_priya",
            merchant_id="amazon",
            amount=400000, # ₹4,000 (spent ₹4,500 on past date, but daily reset gives fresh ₹5,000 limit)
            mandate_id="m_old_date_01"
        )
        decision = authorize_transaction(req)
        
        self.assertEqual(decision.decision, "approved")
        self.assertEqual(decision.updated_mandate["spent_today"], 400000)

    @patch("gate.gate_service.create_razorpay_order")
    def test_razorpay_failure_graceful_handling(self, mock_create_order):
        """Test that Razorpay API errors roll back mandate update and log error gracefully."""
        mock_create_order.side_effect = RazorpayAPIError(401, "Invalid Razorpay API Credentials")

        req = AuthorizeRequest(
            user_id="user_rahul",
            merchant_id="zepto",
            amount=20000,
            mandate_id="m_active_01"
        )
        decision = authorize_transaction(req)
        
        self.assertEqual(decision.decision, "error")
        self.assertEqual(decision.reason, "razorpay_api_error")
        
        # Verify database mandate was NOT updated
        conn = get_connection(self.db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT spent_today FROM mandates WHERE mandate_id = 'm_active_01'")
        spent_today = cursor.fetchone()["spent_today"]
        conn.close()
        self.assertEqual(spent_today, 150000) # Unchanged!

if __name__ == "__main__":
    unittest.main()
