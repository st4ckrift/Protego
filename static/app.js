const API_BASE = "";

document.addEventListener("DOMContentLoaded", () => {
  loadMandates();
  loadAuditLogs();
});

/* ===========================
   TAB NAVIGATION
   =========================== */
function switchTab(tabId) {
  document.querySelectorAll(".tab-btn").forEach(btn => {
    btn.classList.remove("active");
    btn.setAttribute("aria-selected", "false");
  });
  document.querySelectorAll(".tab-content").forEach(c => c.classList.remove("active"));

  const targetBtn = Array.from(document.querySelectorAll(".tab-btn")).find(
    b => b.getAttribute("onclick") && b.getAttribute("onclick").includes(tabId)
  );
  if (targetBtn) {
    targetBtn.classList.add("active");
    targetBtn.setAttribute("aria-selected", "true");
  }

  const targetSection = document.getElementById(`tab-${tabId}`);
  if (targetSection) targetSection.classList.add("active");

  if (tabId === "mandates") loadMandates();
  if (tabId === "audit") loadAuditLogs();
}

/* ===========================
   CURRENCY HELPERS
   =========================== */
function formatINR(paise) {
  return "₹" + (paise / 100).toLocaleString("en-IN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2
  });
}

/* ===========================
   MANDATES
   =========================== */
async function loadMandates() {
  try {
    const res = await fetch(`${API_BASE}/gate/mandates`);
    const data = await res.json();
    if (data.status === "success") {
      renderMandates(data.mandates);
    }
  } catch (err) {
    console.error("Failed to load mandates", err);
  }
}

function renderMandates(mandates) {
  const container = document.getElementById("mandates-grid");
  const countEl = document.getElementById("mandate-count");
  if (countEl) countEl.textContent = mandates.length;
  if (!container) return;

  if (mandates.length === 0) {
    container.innerHTML = `
      <div class="placeholder-state" style="grid-column: 1 / -1;">
        <div class="icon-frame" aria-hidden="true">
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect width="20" height="14" x="2" y="5" rx="2"/>
            <line x1="2" x2="22" y1="10" y2="10"/>
          </svg>
        </div>
        <h3>No mandates found</h3>
        <p>Click <strong>Reseed mandates</strong> to generate sample data.</p>
      </div>`;
    return;
  }

  container.innerHTML = mandates.map(m => {
    const dayPct  = m.daily_utilization_pct  || 0;
    const totalPct = m.total_utilization_pct || 0;

    const dayFillClass   = dayPct   >= 100 ? "fill-danger" : dayPct   > 70 ? "fill-warn" : "fill-safe";
    const totalFillClass = totalPct >= 100 ? "fill-danger" : totalPct > 70 ? "fill-warn" : "fill-safe";

    const statusBadge =
      m.status === "expired" ? `<span class="badge badge-danger">Expired</span>` :
      m.status === "revoked" ? `<span class="badge badge-warning">Revoked</span>` :
                               `<span class="badge badge-success">Active</span>`;

    const expiryFormatted = m.expires_at ? m.expires_at.split("T")[0] : "—";

    return `
      <div class="mandate-card" role="listitem">
        <div class="mandate-card-header">
          <div>
            <span class="merchant-tag">${m.merchant_id.toUpperCase()}</span>
            <div class="mandate-title">${m.mandate_id}</div>
            <div class="mandate-user-id">User: <strong class="font-mono">${m.user_id}</strong></div>
          </div>
          ${statusBadge}
        </div>

        <div class="mandate-cap-row">
          <span class="cap-label">Per-txn cap</span>
          <span class="cap-value">${formatINR(m.max_per_txn)}</span>
        </div>

        <div class="progress-bar-container">
          <div class="progress-label">
            <span>Daily: ${formatINR(m.spent_today_current)} / ${formatINR(m.max_per_day)}</span>
            <span class="font-mono tabular-num">${dayPct}%</span>
          </div>
          <div class="progress-track" role="progressbar" aria-valuenow="${dayPct}" aria-valuemin="0" aria-valuemax="100" aria-label="Daily spend ${dayPct}%">
            <div class="progress-fill ${dayFillClass}" style="width: ${Math.min(100, dayPct)}%"></div>
          </div>
        </div>

        <div class="progress-bar-container">
          <div class="progress-label">
            <span>Lifetime: ${formatINR(m.spent_total)} / ${formatINR(m.max_total)}</span>
            <span class="font-mono tabular-num">${totalPct}%</span>
          </div>
          <div class="progress-track" role="progressbar" aria-valuenow="${totalPct}" aria-valuemin="0" aria-valuemax="100" aria-label="Lifetime spend ${totalPct}%">
            <div class="progress-fill ${totalFillClass}" style="width: ${Math.min(100, totalPct)}%"></div>
          </div>
        </div>

        <div class="mandate-meta">
          <span>Expires <span class="font-mono">${expiryFormatted}</span></span>
        </div>
      </div>
    `;
  }).join("");
}

/* ===========================
   AUDIT LOGS
   =========================== */
async function loadAuditLogs() {
  try {
    const filterEl = document.getElementById("audit-filter-decision");
    const decision = filterEl ? filterEl.value : "";
    let url = `${API_BASE}/gate/audit?limit=50`;
    if (decision) url += `&decision=${decision}`;

    const res  = await fetch(url);
    const data = await res.json();
    if (data.status === "success") {
      renderAuditLogs(data.logs);
    }
  } catch (err) {
    console.error("Failed to load audit logs", err);
  }
}

function renderAuditLogs(logs) {
  const tbody = document.getElementById("audit-table-body");
  if (!tbody) return;

  if (logs.length === 0) {
    tbody.innerHTML = `<tr><td colspan="7" class="audit-empty">No records match the current filter.</td></tr>`;
    return;
  }

  tbody.innerHTML = logs.map(l => {
    const decBadge =
      l.decision === "denied" ? `<span class="badge badge-danger">Denied</span>` :
      l.decision === "error"  ? `<span class="badge badge-warning">Error</span>` :
                                `<span class="badge badge-success">Approved</span>`;

    const rzpOrder = l.razorpay_order_id
      ? `<code class="font-mono rzp-order-id">${l.razorpay_order_id}</code>`
      : `<span class="text-muted">—</span>`;

    const timestamp = l.created_at
      ? l.created_at.replace("T", " ").split(".")[0]
      : "—";

    return `
      <tr>
        <td class="font-mono audit-ts tabular-num">${timestamp}</td>
        <td><strong class="font-mono">${l.mandate_id || "—"}</strong></td>
        <td>
          <div class="audit-user">${l.user_id}</div>
          <div class="audit-merchant">${l.merchant_id}</div>
        </td>
        <td class="font-mono tabular-num audit-amount">${formatINR(l.amount)}</td>
        <td>${decBadge}</td>
        <td class="audit-reason"><code class="font-mono">${l.reason}</code></td>
        <td>${rzpOrder}</td>
      </tr>
    `;
  }).join("");
}

/* ===========================
   CHAT / AGENT
   =========================== */
async function handleChatSubmit(e) {
  if (e) e.preventDefault();
  const input  = document.getElementById("chat-input");
  const prompt = input.value.trim();
  if (!prompt) return;

  appendChatMessage("user", `<p>${escapeHtml(prompt)}</p>`);
  input.value = "";

  const loadingId = "msg-loading-" + Date.now();
  appendChatMessage("system", `<p class="text-muted"><em>Evaluating gate authorization...</em></p>`, loadingId);

  try {
    const res  = await fetch(`${API_BASE}/agent/process`, {
      method:  "POST",
      headers: { "Content-Type": "application/json" },
      body:    JSON.stringify({ prompt })
    });
    const data = await res.json();

    // Remove loading indicator
    const loadingEl = document.getElementById(loadingId);
    if (loadingEl) loadingEl.remove();

    if (data.status === "success") {
      renderGateInspector(data);

      const gateDec = data.gate_decision || {};
      let responseHtml = `<p>${data.agent_explanation || ""}</p>`;

      if (gateDec.decision === "approved") {
        responseHtml += `
          <div class="msg-outcome msg-outcome-approved">
            <strong>Authorization confirmed</strong><br>
            Razorpay order: <code class="font-mono">${gateDec.razorpay_order_id}</code><br>
            Amount: <strong class="font-mono tabular-num">${formatINR(gateDec.amount)}</strong>
          </div>`;
      } else {
        responseHtml += `
          <div class="msg-outcome msg-outcome-denied">
            <strong>Transaction blocked</strong><br>
            Reason: <code class="font-mono">${gateDec.reason || "execution failure"}</code>
          </div>`;
      }

      appendChatMessage("system", responseHtml);
      loadMandates();
      loadAuditLogs();
    } else {
      appendChatMessage("system", `<p class="err-text">Error: ${escapeHtml(data.error || "Failed to process request")}</p>`);
    }
  } catch (err) {
    console.error(err);
    const loadingEl = document.getElementById(loadingId);
    if (loadingEl) loadingEl.remove();
    appendChatMessage("system", `<p class="err-text">Connection failed. Please try again.</p>`);
  }
}

function appendChatMessage(role, htmlContent, id) {
  const container = document.getElementById("chat-history");
  if (!container) return;

  const msgDiv = document.createElement("div");
  msgDiv.className = `chat-message ${role}-message`;
  if (id) msgDiv.id = id;

  const avatarSvg = role === "user"
    ? `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>`
    : `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>`;

  msgDiv.innerHTML = `
    <div class="msg-avatar" aria-hidden="true">${avatarSvg}</div>
    <div class="msg-content">${htmlContent}</div>
  `;
  container.appendChild(msgDiv);
  container.scrollTop = container.scrollHeight;
}

/* ===========================
   GATE INSPECTOR RENDERER
   =========================== */
function renderGateInspector(data) {
  const badge = document.getElementById("inspect-status-badge");
  const body  = document.getElementById("inspect-content");
  if (!body) return;

  const dec    = data.gate_decision || {};
  const reason = dec.reason || "unknown";

  if (dec.decision === "approved") {
    badge.className  = "badge badge-success";
    badge.textContent = "Approved";
  } else if (dec.decision === "denied") {
    badge.className  = "badge badge-danger";
    badge.textContent = "Denied";
  } else {
    badge.className  = "badge badge-warning";
    badge.textContent = "Error";
  }

  // derive sequential gate check results from failure reason
  const c1 = !["mandate_not_found", "user_mismatch", "merchant_mismatch"].includes(reason);
  const c2 = c1 && !["mandate_expired", "mandate_revoked"].includes(reason);
  const c3 = c2 && reason !== "max_per_txn_exceeded";
  const c4 = c3 && reason !== "max_per_day_exceeded";
  const c5 = c4 && reason !== "max_total_exceeded";

  const intent = data.parsed_intent || {};

  body.innerHTML = `
    <div class="nlu-intent-box">
      <div class="gate-section-title">NLU parsed intent</div>
      <div class="nlu-intent-row">
        <span>User</span><code>${intent.user_id || "—"}</code>
        <span>·</span>
        <span>Merchant</span><code>${intent.merchant_id || "—"}</code>
        <span>·</span>
        <span>Amount</span><code>${intent.amount_formatted || "—"}</code>
      </div>
    </div>

    <div class="gate-section-title">Sequential gate checks</div>

    ${[
      { n: 1, label: "Mandate &amp; identity validation", pass: c1 },
      { n: 2, label: "Active &amp; expiry status",        pass: c2 },
      { n: 3, label: "Per-transaction ceiling",          pass: c3 },
      { n: 4, label: "Daily rolling spend limit",        pass: c4 },
      { n: 5, label: "Lifetime mandate budget",          pass: c5 }
    ].map(({ n, label, pass }) => `
      <div class="check-step ${pass ? "pass" : "fail"}">
        <span class="check-step-title">${n}. ${label}</span>
        <span class="check-pill">${pass ? "Pass" : "Fail"}</span>
      </div>
    `).join("")}

    <div class="gate-section-title gate-section-title-payload">Razorpay audit payload</div>
    <pre class="json-box">${escapeHtml(JSON.stringify(dec, null, 2))}</pre>
  `;
}

/* ===========================
   QUICK DEMO SHORTCUTS
   =========================== */
const DEMO_PROMPTS = {
  1: "Order groceries for ₹400 from Zepto for user_rahul",
  2: "Order food for ₹500 from Swiggy for user_rahul",
  3: "Buy electronics for ₹4500 from Blinkit for user_rahul",
  4: "Order groceries for ₹300 from Blinkit for user_rahul"
};

function runQuickDemo(type) {
  const input = document.getElementById("chat-input");
  if (!input || !DEMO_PROMPTS[type]) return;
  input.value = DEMO_PROMPTS[type];
  handleChatSubmit();
}

/* ===========================
   SEED DB
   =========================== */
async function seedDb() {
  const btn = document.getElementById("btn-seed");
  if (btn) { btn.disabled = true; btn.textContent = "Seeding…"; }
  try {
    await fetch(`${API_BASE}/gate/mandates`, {
      method:  "POST",
      headers: { "Content-Type": "application/json" },
      body:    JSON.stringify({
        mandate_id:  "mandate_seed_reset_" + Date.now(),
        user_id:     "user_rahul",
        merchant_id: "zepto",
        max_per_txn: 80000,
        max_per_day: 200000,
        max_total:   1000000
      })
    });
    await loadMandates();
    await loadAuditLogs();
  } catch (err) {
    console.error("Failed to seed mandates", err);
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.innerHTML = `
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M3 12a9 9 0 0 1 15-6.7L21 8"/>
          <path d="M21 3v5h-5"/>
          <path d="M21 12a9 9 0 0 1-15 6.7L3 16"/>
          <path d="M8 16H3v5"/>
        </svg>
        Reseed mandates`;
    }
  }
}

/* ===========================
   MANDATE MODAL
   =========================== */
function openNewMandateModal() {
  const modal = document.getElementById("modal-mandate");
  if (modal) {
    modal.classList.add("active");
    // Focus first input for keyboard accessibility
    requestAnimationFrame(() => {
      const first = modal.querySelector("input");
      if (first) first.focus();
    });
  }
}

function closeNewMandateModal() {
  const modal = document.getElementById("modal-mandate");
  if (modal) {
    modal.classList.remove("active");
    clearMandateFormError();
  }
}

async function submitNewMandate(e) {
  e.preventDefault();
  clearMandateFormError();

  const id       = document.getElementById("m-id").value.trim();
  const user     = document.getElementById("m-user").value.trim();
  const merchant = document.getElementById("m-merchant").value.trim();
  const txn      = parseInt(document.getElementById("m-max-txn").value) * 100;
  const day      = parseInt(document.getElementById("m-max-day").value) * 100;
  const total    = parseInt(document.getElementById("m-max-total").value) * 100;

  if (!id || !user || !merchant || !txn || !day || !total) {
    showMandateFormError("All fields are required.");
    return;
  }

  if (txn > day) {
    showMandateFormError("Per-transaction cap cannot exceed the daily cap.");
    return;
  }

  if (day > total) {
    showMandateFormError("Daily cap cannot exceed the lifetime cap.");
    return;
  }

  try {
    const res  = await fetch(`${API_BASE}/gate/mandates`, {
      method:  "POST",
      headers: { "Content-Type": "application/json" },
      body:    JSON.stringify({ mandate_id: id, user_id: user, merchant_id: merchant, max_per_txn: txn, max_per_day: day, max_total: total })
    });
    const data = await res.json();
    if (res.ok) {
      closeNewMandateModal();
      document.getElementById("form-new-mandate").reset();
      loadMandates();
    } else {
      showMandateFormError(data.error || "Failed to create mandate. Please try again.");
    }
  } catch (err) {
    console.error("Failed to create mandate", err);
    showMandateFormError("Connection failed. Please try again.");
  }
}

function showMandateFormError(msg) {
  const el = document.getElementById("mandate-form-error");
  if (!el) return;
  el.textContent = msg;
  el.hidden = false;
}

function clearMandateFormError() {
  const el = document.getElementById("mandate-form-error");
  if (el) el.hidden = true;
}

/* ===========================
   UTILITIES
   =========================== */
function escapeHtml(str) {
  if (typeof str !== "string") return String(str || "");
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

// Close modal on overlay click
document.addEventListener("click", e => {
  const overlay = document.getElementById("modal-mandate");
  if (e.target === overlay) closeNewMandateModal();
});

// Keyboard: Escape closes modal
document.addEventListener("keydown", e => {
  if (e.key === "Escape") {
    const overlay = document.getElementById("modal-mandate");
    if (overlay && overlay.classList.contains("active")) closeNewMandateModal();
  }
});
