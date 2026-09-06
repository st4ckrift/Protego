# Protego
> A deterministic spend-gate shield sitting between AI shopping agents and payment APIs to enforce pre-approved money bounds before transactions occur.

---

## The Problem, in Plain Language

Imagine handing a credit card with an unlimited spending cap to a personal shopping assistant and saying, *"Buy some groceries."* If the assistant misinterprets your request or gets tricked, it could spend ₹50,000 in seconds. 

Instead of an uncapped credit card, Protego works like a digital **Allowance Card**. You define strict spending rules in advance—for instance, *"Maximum ₹800 per order, up to ₹2,000 per day, only at Zepto."*

Before your AI shopping agent can place any order, it must ask Protego for permission. Protego evaluates the order against your rules in deterministic code. If the purchase fits your rules, Protego allows the payment to proceed to Razorpay. If even one rule is broken, Protego blocks the transaction instantly and explains exactly why and by how much.

---

## How It Works

Protego acts as a gatekeeper between the AI agent's purchase intent and Razorpay's payment infrastructure.

```
[ User Request ] ──> [ AI Agent ] ──> [ Protego Spend-Gate ] ──> [ Razorpay API ]
                                             │
                                    Runs 8 Specific Checks
                                             │
                                 ┌───────────┴───────────┐
                             ✅ Approved             🚫 Blocked
                      (Razorpay Order Created)   (Specific Reason + Numbers)
                                 │                       │
                                 └───────────┬───────────┘
                                             ▼
                                  [ Immutable Audit Log ]
```

### Validation Check Sequence

When `POST /gate/authorize` is called, Protego runs **8 specific sequential checks** inside a thread-safe SQLite transaction:

1. **Mandate Existence**: Verifies that the requested mandate ID exists (or auto-resolves the latest active mandate for the user/merchant pair).
2. **User Identity Match**: Confirms the mandate belongs to the requesting `user_id` (`user_mismatch`).
3. **Merchant Scope Match**: Confirms the mandate is scoped to the target `merchant_id` (`merchant_mismatch`).
4. **Revocation Check**: Ensures the mandate has not been revoked by the user (`mandate_revoked`).
5. **Expiration Check**: Checks if the mandate timestamp is expired (`mandate_expired`). Also auto-resets `spent_today` to `0` if the date has advanced.
6. **Per-Transaction Cap**: Verifies `amount <= max_per_txn` (`max_per_txn_exceeded`).
7. **Daily Spend Cap**: Verifies `spent_today + amount <= max_per_day` (`max_per_day_exceeded`).
8. **Lifetime Cap**: Verifies `spent_total + amount <= max_total` (`max_total_exceeded`).

---

## Architecture

Protego is built in Rust with **zero external dependencies** (`Cargo.toml` has an empty `[dependencies]` block). Every layer—from SQLite FFI bindings to the multithreaded HTTP server—is implemented against the Rust standard library.

```
src/
├── lib.rs              # Root library module declarations
├── config.rs           # Environment variable loader (.env parser with default fallbacks)
├── models.rs           # Data structures, JSON serializers, and MandateStatus enum
├── database.rs         # Safe Rust FFI wrapper over C sqlite3 library (WAL & busy_timeout)
├── gate_service.rs     # Core decision engine enforcing the 8 sequential checks & locking
├── audit_service.rs    # Engine for writing and fetching immutable audit trail entries
├── razorpay_client.rs  # Razorpay API client (shells out to curl or returns simulated orders)
├── nlu_parser.rs       # Deterministic keyword & digit parser for user spend prompts
├── agent_service.rs    # Agent coordinator that calls the gate and formats markdown replies
├── server_service.rs   # Multithreaded HTTP REST API & static file web server (std::net)
└── bin/
    ├── seed.rs         # Database seeder binary (`cargo run --bin seed`)
    ├── server.rs       # Server launcher binary (`cargo run --bin server`)
    └── agent.rs        # CLI agent runner binary (`cargo run --bin agent -- "..."`)
```

---

## Key Design Decisions & Technical Callouts

### 1. Concurrency & Race-Condition Safety
In agentic shopping, an agent might issue rapid concurrent payment requests. Protego uses SQLite `BEGIN IMMEDIATE` transaction locking with `PRAGMA busy_timeout=5000;`. This ensures two parallel authorization requests against the same mandate cannot overspend remaining daily limits. The test suite includes `test_concurrency_race_condition`, which spawns simultaneous parallel OS threads to prove that exactly one transaction succeeds and the other is denied.

### 2. Deterministic Rule-Based Intent Parsing
Protego does **not** rely on an LLM for money decisions or intent parsing. `nlu_parser.rs` uses deterministic keyword matching and currency digit extraction. Spending decisions must be predictable, auditable, and testable; keeping the NLU deterministic prevents non-deterministic LLM behavior from influencing the financial boundary.

### 3. Zero-Dependency Rust Implementation
The Rust codebase avoids third-party crates (`serde`, `axum`, `rusqlite`, `reqwest`).
- **SQLite**: Bound directly via C FFI (`extern "C"`) to `/usr/lib/libsqlite3.so`.
- **HTTP Server**: Implemented with `std::net::TcpListener` spawning a `std::thread` per connection.
- **JSON Handling**: Hand-crafted serialization and parsing routines in `models.rs` and `server_service.rs`.

*Trade-off note*: Writing low-level FFI and socket handlers provided total control over execution and locking semantics without crate bloat, though it required custom JSON parsers and manual C string memory management.

---

## Getting Started

### 1. Prerequisites
- Rust compiler & Cargo (`rustc 1.80+` / `cargo`)
- `libsqlite3` installed on system (`sqlite3.h` and `libsqlite3.so`)
- `curl` binary installed (used by `razorpay_client.rs` when real API keys are set)

### 2. Environment Configuration
Copy `.env.example` to `.env`:
```bash
cp .env.example .env
```

Default variables:
```env
PORT=8000
HOST=0.0.0.0
DB_PATH=mandate_gate.db
RAZORPAY_KEY_ID=rzp_test_your_key_id
RAZORPAY_KEY_SECRET=your_test_secret
OPENAI_API_KEY=your_openai_api_key
```
*Note*: If `RAZORPAY_KEY_ID` is left as placeholder or empty, Protego runs in **simulated mode**, generating realistic `order_test_...` IDs so the demo works fully offline without live Razorpay credentials.

### 3. Seed Database
Populate sample mandates (Zepto, Swiggy, Amazon, Expired Blinkit):
```bash
cargo run --bin seed
```

### 4. Run Server & Web Dashboard
Start the HTTP server on port 8000:
```bash
cargo run --bin server
```
Access the interactive web UI at **`http://localhost:8000`**.

### 5. Run CLI Agent Simulator
Run single prompts from the command line:
```bash
cargo run --bin agent -- "Order groceries for ₹400 from Zepto for user_rahul"
```

### 6. Run Test Suite
Execute unit and concurrency tests:
```bash
cargo test
```

---

## Using the Dashboard

When you open `http://localhost:8000` in your web browser, you will find three main tabs:

1. **AI Agent Simulator**:
   - Type shopping requests in plain English (e.g., *"Order groceries for ₹400 from Zepto for user_rahul"*).
   - Use the **Quick Demo Shortcuts** buttons to test Happy Path, Over Daily Cap, Wrong Merchant, or Expired Mandate in one click.
   - The right-hand panel (**Spend-Gate Execution Inspector**) shows the live pass/fail status of all gate checks and the raw JSON response payload.

2. **Active Mandates**:
   - View visual cards for all pre-approved mandates.
   - Monitor daily and lifetime spending progress bars.
   - Use the **+ Create New Mandate** button to add custom mandates on the fly.

3. **Immutable Audit Trail**:
   - View a complete history of every authorization request.
   - Filter logs by decision (`Approved`, `Denied`).
   - Inspect timestamp, requested amounts, decision reasons, and linked Razorpay Order IDs.

---

## Testing

The test suite in `tests/gate_test.rs` covers core decision logic and race safety:

- `test_approved_transaction`: Verifies valid transactions pass all checks, update mandate spend counters, and create audit entries.
- `test_max_per_txn_exceeded`: Tests denial when an order exceeds the single-transaction cap.
- `test_max_per_day_exceeded`: Tests denial when an order exceeds the remaining daily cap.
- `test_merchant_mismatch`: Verifies blocking when requesting a transaction at an unapproved merchant.
- `test_user_mismatch`: Verifies blocking when requesting spend against another user's mandate.
- `test_expired_mandate`: Confirms transactions against expired mandates are blocked.
- `test_revoked_mandate`: Confirms transactions against revoked mandates are blocked.
- `test_daily_reset_logic`: Tests that daily spend counters reset when `last_spent_date` advances.
- `test_concurrency_race_condition`: Spawns two parallel OS threads attempting simultaneous spend against a mandate with ₹500 remaining limit. Asserts that **exactly one succeeds** and **exactly one is denied**.

---

## Known Limitations & Honest Scope Notes

- **Simplified Timestamp Formatting**: `audit_service.rs` uses a simplified year calculation (`format!("{}-01-01T00:00:00Z", 1970 + secs / 31536000)`). Month and day components in ISO strings default to `-01-01T00:00:00Z`.
- **No LLM Integration**: Despite `Config` reading `OPENAI_API_KEY`, intent parsing in `nlu_parser.rs` is entirely deterministic (keyword and regex-style numeric extraction). No OpenAI API calls are currently wired up.
- **Curl Shell-Out**: When live Razorpay keys are configured, `razorpay_client.rs` calls `std::process::Command::new("curl")` to POST to `https://api.razorpay.com/v1/orders` rather than using a native Rust HTTP client crate.
- **Basic HTTP Request Parsing**: The custom HTTP server in `server_service.rs` reads up to 8192 bytes from incoming TCP connections, suitable for lightweight JSON payloads but not large body streams.

---

## What's Next

1. **Proper HTTP Client**: Replace the `curl` shell-out in `razorpay_client.rs` with standard library TCP/TLS sockets.
2. **Accurate Date Formatting**: Replace the simplified year-only timestamp function in `audit_service.rs` with full UTC month/day/time formatting.
3. **Optional LLM Parser**: Wire an optional LLM intent parser with strict schema validation while keeping the Spend-Gate enforcement deterministic.
4. **Mandate Rule DSL**: Build a user-friendly rule builder for setting time window restrictions and dynamic spend limits.
