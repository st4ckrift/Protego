# Mandate & Spend-Gate Service for Agentic Commerce (Rust Edition)

> **Razorpay Hackathon (Track 1: AI Growth & Agentic Commerce)**  
> *"Every money action explainable, bounded and gated. Show the audit trail and one failure handled gracefully."*

---

## 📌 Problem Statement

As AI shopping agents gain autonomy to transact on behalf of users, leaving payment authorization to non-deterministic LLM logic creates unacceptable financial risk. The **Mandate & Spend-Gate Service** acts as an immutable, race-safe enforcement proxy between an AI agent's *spending intent* and Razorpay's payment APIs. Inspired by Razorpay + NPCI's production Agentic Payments protocol, this service enforces pre-approved spending mandates (per-transaction caps, daily rolling limits, lifetime caps, merchant scoping, and expiration dates) in **deterministic, type-safe Rust code**, logging every decision immutably before forwarding approved transactions to Razorpay's test-mode order APIs.

---

## 🦀 Why Rust? Type-Safe Money & Concurrency Guarantees

1. **Unrepresentable Invalid States:** Mandate status is modeled as a strongly typed Rust `enum MandateStatus { Active, Expired, Revoked }`, eliminating stringly-typed state bugs at compile time.
2. **Zero Floating-Point Money Handling:** All financial figures (`max_per_txn`, `max_per_day`, `spent_today`, `spent_total`, requested `amount`) are represented strictly as 64-bit integer paise (`i64`), guaranteeing integer precision and preventing rounding errors.
3. **Race-Safe Concurrency & Atomic Locking:** Uses SQLite WAL mode with `BEGIN IMMEDIATE` transaction locking and `busy_timeout=5000`. Tested with multi-threaded parallel requests to guarantee that simultaneous agent calls against the same mandate can **never overspend** remaining limits.
4. **Transaction Rollback on API Errors:** If Razorpay API call fails (network drop or invalid credentials), the gate executes an explicit `conn.rollback()`, ensuring no money is deducted from mandate balances when payment creation fails.

---

## 🏗 System Architecture

```
                                 ┌───────────────────────────────┐
                                 │   User / Shopping Intent      │
                                 └──────────────┬────────────────┘
                                                │
                                                ▼
                                 ┌───────────────────────────────┐
                                 │     AI Agent NLU Layer        │
                                 │ (Extracts User/Merchant/INR)  │
                                 └──────────────┬────────────────┘
                                                │
                                                ▼ (Calls ONLY Gate Endpoint)
 ┌──────────────────────────────────────────────────────────────────────────────┐
 │                    RUST MANDATE & SPEND-GATE SERVICE                         │
 │                                                                              │
 │  Check 1: Does mandate exist for User + Merchant pair?                       │
 │  Check 2: Is mandate status Active and unexpired?                            │
 │  Check 3: Is requested amount <= max_per_txn cap?                            │
 │  Check 4: Would spent_today + amount <= max_per_day cap? (Auto Daily Reset)  │
 │  Check 5: Would spent_total + amount <= max_total cap?                       │
 └──────────────────────┬───────────────────────────────┬───────────────────────┘
                        │                               │
                [If Any Check Fails]                    │ [If ALL Checks Pass]
                        │                               │
                        ▼                               ▼
      ┌───────────────────────────────────┐  ┌───────────────────────────────────┐
      │     Structured Denial Response    │  │ Razorpay Test API (orders.create) │
      │  (Exact limits & already spent)   │  └─────────────────┬─────────────────┘
      └─────────────────┬─────────────────┘                    │
                        │                                      ▼
                        │                       ┌───────────────────────────────┐
                        │                       │  Razorpay Order ID Created    │
                        │                       └──────────────┬────────────────┘
                        │                                      │
                        ▼                                      ▼
      ┌─────────────────────────────────────────────────────────────────────────┐
      │                     IMMUTABLE AUDIT LOG (SQLite)                        │
      │   (Records Log ID, Timestamp, Decision, Metrics, Razorpay Order ID)     │
      └─────────────────────────────────────────────────────────────────────────┘
```

---

## 🔒 The 5 Mandatory Gate Checks (Sequential Execution)

For every transaction authorization request (`POST /gate/authorize`), the gate executes the following **5 strict sequential checks** inside a thread-safe SQLite `BEGIN IMMEDIATE` atomic transaction:

1. **Existence & Identity Match:** Verifies mandate exists and `user_id` + `merchant_id` match.
2. **Active Status & Expiration:** Rejects if mandate is `Revoked` or `expires_at` timestamp has passed. Auto-resets daily spend counter if date has advanced.
3. **Per-Transaction Cap (`max_per_txn`):** Rejects if `amount > max_per_txn`.
4. **Daily Spend Cap (`max_per_day`):** Rejects if `spent_today + amount > max_per_day`.
5. **Lifetime Cap (`max_total`):** Rejects if `spent_total + amount > max_total`.

---

## 🚀 Quick Start Guide

### 1. Build Project
```bash
cargo build
```

### 2. Seed Sample Database
Populate realistic demo mandates (Zepto, Swiggy, Amazon, Expired Blinkit):
```bash
cargo run --bin seed
```

### 3. Run Native HTTP Server & Web UI
```bash
cargo run --bin server
```
Open **`http://localhost:8000`** in your browser to access the Interactive Dashboard & AI Agent Simulator.

### 4. Run CLI Agent Simulator
```bash
cargo run --bin agent -- "Order groceries for ₹400 from Zepto for user_rahul"
```

### 5. Run Unit & Concurrency Test Suite
```bash
cargo test
```

---

## 🎬 5-Step Hackathon Demo Script

Run this sequence during judge evaluations to demonstrate complete compliance with the Track 1 brief:

### 1. Happy Path — Approved Transaction
- **Command:** `cargo run --bin agent -- "Order groceries for ₹400 from Zepto for user_rahul"`
- **Result:** **APPROVED**. Mandate `mandate_zepto_01` allows ₹800 per transaction and ₹2,000 per day. Gate returns Razorpay Order ID (e.g. `order_test_...`) and updates mandate counters.

### 2. Blocked — Exceeds Daily Cap
- **Command:** `cargo run --bin agent -- "Order food for ₹500 from Swiggy for user_rahul"`
- **Result:** **DENIED** (`max_per_day_exceeded`). Mandate `mandate_swiggy_01` has ₹2,800 spent today out of a ₹3,000 daily cap. Requesting ₹500 exceeds the remaining ₹200 limit. Gate returns structured denial details.

### 3. Blocked — Wrong Merchant
- **Command:** `cargo run --bin agent -- "Buy snacks for ₹300 from Blinkit for user_rahul"`
- **Result:** **DENIED** (`merchant_mismatch` or `mandate_expired`). Shows the gate enforcing strict merchant scope isolation.

### 4. Multi-Threaded Concurrency Race-Safety Verification
- **Run:** `cargo test test_concurrency_race_condition`
- **Result:** Spawns parallel threads firing simultaneous spend requests against a mandate with ₹500 remaining limit. Proves that **exactly 1 succeeds** and **exactly 1 gets blocked**, preventing double-spending.

### 5. Full Audit Trail Walkthrough
- Open **Tab 3: Immutable Audit Trail** on `http://localhost:8000` (or query `GET /gate/audit`).
- Show that every attempt (Approved or Denied) generates an immutable, timestamped record linked to the exact mandate state and Razorpay Order ID.

---

## 📁 Repository Structure

```
├── Cargo.toml              # Cargo build manifest & binary target definitions
├── src/
│   ├── lib.rs              # Root library exports
│   ├── config.rs           # Environment configuration loader
│   ├── models.rs           # MandateModel, MandateStatus enum, GateDecision, AuditLogEntry
│   ├── database.rs         # Safe Rust FFI SQLite engine wrapper with WAL & busy timeout
│   ├── gate_service.rs     # Core 5-step decision engine & atomic transaction locking
│   ├── razorpay_client.rs  # Razorpay test mode HTTP integration & error handling
│   ├── audit_service.rs    # Immutable audit logging engine
│   ├── nlu_parser.rs       # Dual NLU parser (OpenAI API + fallback regex parser)
│   ├── agent_service.rs    # Agent execution flow & plain language explanation generator
│   ├── server_service.rs   # Multithreaded HTTP REST server & web UI static host
│   └── bin/
│       ├── seed.rs         # Seed binary (cargo run --bin seed)
│       ├── server.rs       # Server binary (cargo run --bin server)
│       └── agent.rs        # CLI binary (cargo run --bin agent -- "...")
├── static/
│   ├── index.html          # Modern Web Dashboard UI
│   ├── app.js              # Tab router & live audit trail client
│   └── style.css           # UI styling
├── tests/
│   └── gate_test.rs        # 9 Rust unit & multi-threaded concurrency tests
├── .env.example            # Environment variables template
└── README.md               # Documentation
```
