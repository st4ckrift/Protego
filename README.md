# Protego

> A deterministic spend-gate sitting between AI shopping agents and Razorpay, enforcing pre-approved spending mandates before any payment is authorized.

---

## What it does

When an AI agent tries to place an order, Protego intercepts the request and runs it through a sequential 8-check authorization pipeline before anything reaches Razorpay. Every decision — approved or denied — is written to an immutable audit log with the exact reason and amounts.

```
[ User request ] → [ AI agent ] → [ Protego spend-gate ] → [ Razorpay API ]
                                          │
                                  8 sequential checks
                                          │
                              ┌───────────┴───────────┐
                          Approved                  Blocked
                   (Razorpay order created)   (Reason + amounts)
                              └───────────┬───────────┘
                                          ▼
                                 [ Immutable audit log ]
```

**Gate check sequence** — run inside a thread-safe SQLite transaction on every `POST /gate/authorize`:

| # | Check | Failure reason |
|---|-------|----------------|
| 1 | Mandate exists (or resolves by user+merchant) | `mandate_not_found` |
| 2 | `user_id` matches mandate owner | `user_mismatch` |
| 3 | `merchant_id` matches mandate scope | `merchant_mismatch` |
| 4 | Mandate has not been revoked | `mandate_revoked` |
| 5 | Mandate has not expired (auto-resets daily counter) | `mandate_expired` |
| 6 | `amount ≤ max_per_txn` | `max_per_txn_exceeded` |
| 7 | `spent_today + amount ≤ max_per_day` | `max_per_day_exceeded` |
| 8 | `spent_total + amount ≤ max_total` | `max_total_exceeded` |

---

## Architecture

Built in Rust against the standard library only — no third-party crates.

```
src/
├── lib.rs              # Module declarations
├── config.rs           # .env loader with fallback defaults
├── models.rs           # Data structures, JSON serializers, MandateStatus enum
├── database.rs         # Rust FFI wrapper over C sqlite3 (WAL + busy_timeout)
├── gate_service.rs     # 8-check authorization engine with BEGIN IMMEDIATE locking
├── audit_service.rs    # Immutable audit trail writer and reader
├── razorpay_client.rs  # Razorpay API client (curl shell-out or simulated orders)
├── nlu_parser.rs       # Deterministic keyword + digit parser for spend prompts
├── agent_service.rs    # Agent coordinator — calls gate, formats response
├── server_service.rs   # Multithreaded HTTP server + static file serving (std::net)
└── bin/
    ├── server.rs       # Server binary
    ├── seed.rs         # Database seeder binary
    └── agent.rs        # CLI agent runner binary
```

**Key decisions:**

- **SQLite `BEGIN IMMEDIATE`** — prevents race conditions when concurrent agent requests target the same mandate. The concurrency test (`test_concurrency_race_condition`) proves exactly one of two simultaneous requests succeeds.
- **Deterministic NLU** — `nlu_parser.rs` uses keyword matching and digit extraction; no LLM is involved in spending decisions so behavior is predictable and testable.
- **Zero dependencies** — SQLite is bound via `extern "C"` FFI, the HTTP server uses `std::net::TcpListener`, and JSON is serialized/parsed by hand.

---

## Prerequisites

| Requirement | Version |
|-------------|---------|
| Rust + Cargo | 1.80 or later |
| libsqlite3 | system-installed (`libsqlite3.so` + `sqlite3.h`) |
| curl | any recent version (used for live Razorpay calls) |

**Install libsqlite3 on Debian/Ubuntu:**
```bash
sudo apt install libsqlite3-dev
```

**Install libsqlite3 on macOS:**
```bash
brew install sqlite
```

---

## Setup

### 1. Clone the repository

```bash
git clone <repo-url>
cd Hackathon
```

### 2. Configure environment

```bash
cp .env.example .env
```

Edit `.env` with your values:

```env
# Server
PORT=8000
HOST=0.0.0.0
DB_PATH=mandate_gate.db

# Razorpay (leave as placeholder to run in simulated mode)
RAZORPAY_KEY_ID=rzp_test_your_key_id
RAZORPAY_KEY_SECRET=your_test_secret

# OpenAI (optional — deterministic NLU is used if unset)
OPENAI_API_KEY=your_openai_api_key
```

> If `RAZORPAY_KEY_ID` is left as the placeholder value or empty, Protego runs in **simulated mode** and generates realistic `order_test_...` IDs. The full demo works without live credentials.

### 3. Build the project

```bash
cargo build
```

### 4. Seed the database

Populates sample mandates for Zepto, Swiggy, Amazon, and an expired Blinkit mandate:

```bash
cargo run --bin seed
```

### 5. Start the server

```bash
cargo run --bin server
```

The server starts on `http://localhost:8000` (or whichever `PORT` is set in `.env`).

---

## Usage

### Web dashboard

Open `http://localhost:8000` in a browser. Three tabs are available:

**AI agent simulator**
- Type a spend command in plain English, e.g. `Order groceries for ₹400 from Zepto for user_rahul`
- Use the quick demo buttons to test specific scenarios (happy path, over daily cap, wrong merchant, expired mandate) in one click
- The right panel shows the live pass/fail result of each gate check and the raw Razorpay JSON payload

**Active mandates**
- Visual cards showing all pre-approved mandates
- Daily and lifetime spend progress bars with color-coded utilization
- Create new mandates on the fly with the **New mandate** button

**Audit trail**
- Full history of every authorization request with timestamps, amounts, decisions, and reasons
- Filter by Approved / Denied / Error
- Linked Razorpay order IDs for approved transactions

### CLI agent

Run a single spend command from the terminal:

```bash
cargo run --bin agent -- "Order groceries for ₹400 from Zepto for user_rahul"
```

```bash
cargo run --bin agent -- "Order food for ₹500 from Swiggy for user_rahul"
```

### REST API

**Authorize a transaction**
```bash
POST /gate/authorize
Content-Type: application/json

{
  "mandate_id": "mandate_zepto_rahul",
  "user_id": "user_rahul",
  "merchant_id": "zepto",
  "amount": 40000
}
```
> Amounts are in paise (₹1 = 100 paise).

**List all mandates**
```bash
GET /gate/mandates
```

**Create a mandate**
```bash
POST /gate/mandates
Content-Type: application/json

{
  "mandate_id": "mandate_uber_01",
  "user_id": "user_priya",
  "merchant_id": "uber",
  "max_per_txn": 100000,
  "max_per_day": 300000,
  "max_total": 1000000
}
```

**Fetch audit log**
```bash
GET /gate/audit?limit=50
GET /gate/audit?limit=50&decision=denied
```

**Process agent prompt**
```bash
POST /agent/process
Content-Type: application/json

{
  "prompt": "Order groceries for ₹400 from Zepto for user_rahul"
}
```

---

## Running tests

```bash
cargo test
```

The test suite in `tests/gate_test.rs` covers:

| Test | What it verifies |
|------|-----------------|
| `test_approved_transaction` | Valid transaction passes all checks, spend counters update, audit entry created |
| `test_max_per_txn_exceeded` | Blocks when amount exceeds per-transaction cap |
| `test_max_per_day_exceeded` | Blocks when order would exceed remaining daily cap |
| `test_merchant_mismatch` | Blocks when merchant is not in mandate scope |
| `test_user_mismatch` | Blocks when requesting spend against another user's mandate |
| `test_expired_mandate` | Blocks transactions against expired mandates |
| `test_revoked_mandate` | Blocks transactions against revoked mandates |
| `test_daily_reset_logic` | Daily counter resets when `last_spent_date` advances |
| `test_concurrency_race_condition` | Two parallel threads target the same mandate — exactly one succeeds, one is denied |

---

## Known limitations

- **Simplified timestamp formatting** — `audit_service.rs` uses a year-only approximation (`1970 + secs / 31536000`); month/day components in ISO strings default to `-01-01T00:00:00Z`.
- **curl shell-out** — live Razorpay calls use `std::process::Command::new("curl")` rather than a native HTTP client.
- **Fixed request buffer** — the HTTP server reads up to 8192 bytes per connection; large request bodies are not supported.
- **No LLM wiring** — `OPENAI_API_KEY` is loaded from config but no OpenAI calls are currently made; intent parsing remains deterministic.

---

## Roadmap

- Replace `curl` shell-out with native TLS sockets in `razorpay_client.rs`
- Full UTC date formatting (month, day, time) in `audit_service.rs`
- Optional LLM intent parser with strict schema validation, keeping gate enforcement deterministic
- Mandate rule DSL for time-windowed and dynamic spend limits
