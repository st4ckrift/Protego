import sys
from datetime import datetime, date, timezone
from gate.database import init_db, get_connection

def seed_database():
    init_db()
    conn = get_connection()
    cursor = conn.cursor()

    # Clear existing data for fresh seed
    cursor.execute("DELETE FROM mandates;")
    cursor.execute("DELETE FROM audit_logs;")

    today_str = date.today().isoformat()

    sample_mandates = [
        (
            "mandate_zepto_01",
            "user_rahul",
            "zepto",
            80000,     # max ₹800 per txn
            200000,    # max ₹2,000 per day
            1000000,   # max ₹10,000 total
            50000,     # spent ₹500 today
            150000,    # spent ₹1,500 total
            today_str,
            "2027-12-31T23:59:59Z",
            datetime.now(timezone.utc).isoformat(),
            "active"
        ),
        (
            "mandate_swiggy_01",
            "user_rahul",
            "swiggy",
            150000,    # max ₹1,500 per txn
            300000,    # max ₹3,000 per day
            1500000,   # max ₹15,000 total
            280000,    # spent ₹2,800 today (leaving ₹200 cap)
            500000,    # spent ₹5,000 total
            today_str,
            "2027-12-31T23:59:59Z",
            datetime.now(timezone.utc).isoformat(),
            "active"
        ),
        (
            "mandate_amazon_01",
            "user_priya",
            "amazon",
            500000,    # max ₹5,000 per txn
            1000000,   # max ₹10,000 per day
            5000000,   # max ₹50,000 total
            0,
            0,
            today_str,
            "2027-12-31T23:59:59Z",
            datetime.now(timezone.utc).isoformat(),
            "active"
        ),
        (
            "mandate_expired_01",
            "user_rahul",
            "blinkit",
            100000,    # max ₹1,000 per txn
            200000,    # max ₹2,000 per day
            500000,    # max ₹5,000 total
            0,
            0,
            today_str,
            "2025-01-01T00:00:00Z", # Past date
            "2024-01-01T00:00:00Z",
            "expired"
        )
    ]

    cursor.executemany("""
    INSERT INTO mandates (
        mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total,
        spent_today, spent_total, last_spent_date, expires_at, created_at, status
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
    """, sample_mandates)

    conn.commit()
    conn.close()
    print("Database successfully seeded with 4 sample mandates.")

if __name__ == "__main__":
    seed_database()
