const API_BASE = "";

document.addEventListener("DOMContentLoaded", () => {
  loadMandates();
  loadAuditLogs();
});

function switchTab(tabId) {
  document.querySelectorAll(".tab-btn").forEach(btn => btn.classList.remove("active"));
  document.querySelectorAll(".tab-content").forEach(c => c.classList.remove("active"));
  
  const targetBtn = Array.from(document.querySelectorAll(".tab-btn")).find(b => b.getAttribute("onclick") && b.getAttribute("onclick").includes(tabId));
  if (targetBtn) targetBtn.classList.add("active");
  
  const targetSection = document.getElementById(`tab-${tabId}`);
  if (targetSection) targetSection.classList.add("active");

  if (tabId === "mandates") loadMandates();
  if (tabId === "audit") loadAuditLogs();
}

function formatINR(paise) {
  return "₹" + (paise / 100).toLocaleString('en-IN', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

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
  if (countEl) countEl.innerText = mandates.length;
  if (!container) return;

  if (mandates.length === 0) {
    container.innerHTML = `
      <div class="placeholder-state" style="grid-column: 1 / -1;">
        <div class="icon-frame">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect width="20" height="14" x="2" y="5" rx="2"/>
            <line x1="2" x2="22" y1="10" y2="10"/>
          </svg>
        </div>
        <h3>No Mandates Found</h3>
        <p>Click <strong>Reseed Mandates</strong> above to generate sample mandates.</p>
      </div>`;
    return;
  }

  container.innerHTML = mandates.map(m => {
    const dayPct = m.daily_utilization_pct || 0;
    const totalPct = m.total_utilization_pct || 0;

    let dayFillClass = "fill-safe";
    if (dayPct > 70) dayFillClass = "fill-warn";
    if (dayPct >= 100) dayFillClass = "fill-danger";

    let totalFillClass = "fill-safe";
    if (totalPct > 70) totalFillClass = "fill-warn";
    if (totalPct >= 100) totalFillClass = "fill-danger";

    let statusBadge = `<span class="badge badge-success">ACTIVE</span>`;
    if (m.status === "expired") statusBadge = `<span class="badge badge-danger">EXPIRED</span>`;
    if (m.status === "revoked") statusBadge = `<span class="badge badge-warning">REVOKED</span>`;

    const expiryFormatted = m.expires_at ? m.expires_at.split('T')[0] : 'N/A';

    return `
      <div class="mandate-card">
        <div class="mandate-card-header">
          <div>
            <span class="merchant-tag">${m.merchant_id.toUpperCase()}</span>
            <div class="mandate-title">${m.mandate_id}</div>
            <div class="mandate-user-id">User: <strong class="font-mono" style="color:var(--text-secondary);">${m.user_id}</strong></div>
          </div>
          ${statusBadge}
        </div>

        <div style="font-size:0.825rem; margin-bottom:0.75rem; color:var(--text-muted);">
          Single Txn Cap: <strong style="color:var(--text-primary); font-family:var(--font-mono);">${formatINR(m.max_per_txn)}</strong>
        </div>

        <div class="progress-bar-container">
          <div class="progress-label">
            <span>Daily: ${formatINR(m.spent_today_current)} / ${formatINR(m.max_per_day)}</span>
            <span class="font-mono">${dayPct}%</span>
          </div>
          <div class="progress-track">
            <div class="progress-fill ${dayFillClass}" style="width: ${Math.min(100, dayPct)}%"></div>
          </div>
        </div>

        <div class="progress-bar-container">
          <div class="progress-label">
            <span>Lifetime: ${formatINR(m.spent_total)} / ${formatINR(m.max_total)}</span>
            <span class="font-mono">${totalPct}%</span>
          </div>
          <div class="progress-track">
            <div class="progress-fill ${totalFillClass}" style="width: ${Math.min(100, totalPct)}%"></div>
          </div>
        </div>

        <div style="margin-top:0.85rem; font-size:0.75rem; color:var(--text-subtle); display:flex; justify-content:space-between;">
          <span>Expires: <span class="font-mono">${expiryFormatted}</span></span>
        </div>
      </div>
    `;
  }).join("");
}

async function loadAuditLogs() {
  try {
    const filterEl = document.getElementById("audit-filter-decision");
    const decision = filterEl ? filterEl.value : "";
    let url = `${API_BASE}/gate/audit?limit=50`;
    if (decision) url += `&decision=${decision}`;

    const res = await fetch(url);
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
    tbody.innerHTML = `<tr><td colspan="7" style="text-align:center; padding:2.5rem; color:var(--text-muted);">No audit records found matching current criteria.</td></tr>`;
    return;
  }

  tbody.innerHTML = logs.map(l => {
    let decBadge = `<span class="badge badge-success">APPROVED</span>`;
    if (l.decision === "denied") decBadge = `<span class="badge badge-danger">DENIED</span>`;
    if (l.decision === "error") decBadge = `<span class="badge badge-warning">ERROR</span>`;

    const rzpOrder = l.razorpay_order_id ? `<code class="font-mono" style="color:#38bdf8;">${l.razorpay_order_id}</code>` : `<span class="text-muted">—</span>`;
    const timestamp = l.created_at ? l.created_at.replace('T', ' ').split('.')[0] : 'N/A';

    return `
      <tr>
        <td class="font-mono" style="font-size:0.775rem; color:var(--text-muted);">${timestamp}</td>
        <td><strong class="font-mono" style="color:var(--text-primary);">${l.mandate_id || 'N/A'}</strong></td>
        <td>
          <div><strong style="color:var(--text-primary);">${l.user_id}</strong></div>
          <div style="font-size:0.75rem; color:var(--text-muted);">${l.merchant_id}</div>
        </td>
        <td class="font-mono" style="font-weight:600; color:var(--text-primary);">${formatINR(l.amount)}</td>
        <td>${decBadge}</td>
        <td style="font-size:0.8rem; color:var(--text-muted);"><code class="font-mono" style="color:var(--text-secondary);">${l.reason}</code></td>
        <td>${rzpOrder}</td>
      </tr>
    `;
  }).join("");
}

async function handleChatSubmit(e) {
  if (e) e.preventDefault();
  const input = document.getElementById("chat-input");
  const prompt = input.value.trim();
  if (!prompt) return;

  appendChatMessage("user", prompt);
  input.value = "";

  appendChatMessage("system", `<em>Processing gate authorization request...</em>`);

  try {
    const res = await fetch(`${API_BASE}/agent/process`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ prompt: prompt })
    });
    const data = await res.json();
    
    // Remove processing status indicator
    const history = document.getElementById("chat-history");
    if (history.lastChild && history.lastChild.innerText.includes("Processing gate authorization request...")) {
      history.removeChild(history.lastChild);
    }

    if (data.status === "success") {
      renderGateInspector(data);

      const gateDec = data.gate_decision || {};
      let responseHtml = `<p>${data.agent_explanation || ''}</p>`;

      if (gateDec.decision === "approved") {
        responseHtml += `
          <div style="margin-top:0.5rem; padding:0.5rem 0.75rem; background:var(--success-bg); border-radius:6px; font-size:0.8rem; color:#34d399;">
            <strong>Gate Approval Verified</strong><br>
            Razorpay Order ID: <code class="font-mono" style="color:#ffffff;">${gateDec.razorpay_order_id}</code><br>
            Amount Authorized: <strong class="font-mono">${formatINR(gateDec.amount)}</strong>
          </div>
        `;
      } else {
        responseHtml += `
          <div style="margin-top:0.5rem; padding:0.5rem 0.75rem; background:var(--danger-bg); border-radius:6px; font-size:0.8rem; color:#f87171;">
            <strong>Transaction Denied</strong><br>
            Reason: <code class="font-mono" style="color:#ffffff;">${gateDec.reason || 'Execution failure'}</code>
          </div>
        `;
      }

      appendChatMessage("system", responseHtml);
      loadMandates();
      loadAuditLogs();
    } else {
      appendChatMessage("system", `<p style="color:#f87171;">Error: ${data.error || 'Failed to process agent request'}</p>`);
    }
  } catch (err) {
    console.error(err);
    appendChatMessage("system", `<p style="color:#f87171;">Network Error: ${err.message}</p>`);
  }
}

function appendChatMessage(role, htmlContent) {
  const container = document.getElementById("chat-history");
  if (!container) return;

  const msgDiv = document.createElement("div");
  msgDiv.className = `chat-message ${role}-message`;

  const avatarContent = role === "user" 
    ? `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>`
    : `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>`;

  msgDiv.innerHTML = `
    <div class="msg-avatar">${avatarContent}</div>
    <div class="msg-content">
      ${htmlContent}
    </div>
  `;
  container.appendChild(msgDiv);
  container.scrollTop = container.scrollHeight;
}

function renderGateInspector(data) {
  const badge = document.getElementById("inspect-status-badge");
  const body = document.getElementById("inspect-content");
  if (!body) return;

  const dec = data.gate_decision || {};

  if (dec.decision === "approved") {
    badge.className = "badge badge-success";
    badge.innerText = "DECISION: APPROVED";
  } else if (dec.decision === "denied") {
    badge.className = "badge badge-danger";
    badge.innerText = "DECISION: DENIED";
  } else {
    badge.className = "badge badge-warning";
    badge.innerText = "DECISION: ERROR";
  }

  const reason = dec.reason || "unknown";

  // Build sequential 5 checks status
  const c1 = !["mandate_not_found", "user_mismatch", "merchant_mismatch"].includes(reason);
  const c2 = c1 && !["mandate_expired", "mandate_revoked"].includes(reason);
  const c3 = c2 && reason !== "max_per_txn_exceeded";
  const c4 = c3 && reason !== "max_per_day_exceeded";
  const c5 = c4 && reason !== "max_total_exceeded";

  body.innerHTML = `
    <div style="margin-bottom:1rem; background:var(--bg-inset); padding:0.75rem; border-radius:var(--radius-md); border:1px solid var(--border-color-subtle);">
      <div style="font-weight:600; font-size:0.775rem; text-transform:uppercase; letter-spacing:0.04em; color:var(--text-subtle); margin-bottom:0.35rem;">NLU Parsed Intent</div>
      <div style="font-size:0.8rem; color:var(--text-secondary);">
        User: <code class="font-mono" style="color:var(--text-primary);">${data.parsed_intent?.user_id || 'N/A'}</code> &bull; 
        Merchant: <code class="font-mono" style="color:var(--text-primary);">${data.parsed_intent?.merchant_id || 'N/A'}</code> &bull; 
        Amount: <code class="font-mono" style="color:var(--text-primary);">${data.parsed_intent?.amount_formatted || 'N/A'}</code>
      </div>
    </div>

    <div style="font-weight:600; font-size:0.775rem; text-transform:uppercase; letter-spacing:0.04em; color:var(--text-subtle); margin-bottom:0.5rem;">Sequential Gate Verification</div>

    <div class="check-step ${c1 ? 'pass' : 'fail'}">
      <span class="check-step-title">1. Mandate & Identity Validation</span>
      <span class="check-pill">${c1 ? 'PASS' : 'FAIL'}</span>
    </div>

    <div class="check-step ${c2 ? 'pass' : 'fail'}">
      <span class="check-step-title">2. Active & Expiry Status Check</span>
      <span class="check-pill">${c2 ? 'PASS' : 'FAIL'}</span>
    </div>

    <div class="check-step ${c3 ? 'pass' : 'fail'}">
      <span class="check-step-title">3. Per-Transaction Ceiling Cap</span>
      <span class="check-pill">${c3 ? 'PASS' : 'FAIL'}</span>
    </div>

    <div class="check-step ${c4 ? 'pass' : 'fail'}">
      <span class="check-step-title">4. Daily Rolling Spend Limit</span>
      <span class="check-pill">${c4 ? 'PASS' : 'FAIL'}</span>
    </div>

    <div class="check-step ${c5 ? 'pass' : 'fail'}">
      <span class="check-step-title">5. Lifetime Mandate Budget</span>
      <span class="check-pill">${c5 ? 'PASS' : 'FAIL'}</span>
    </div>

    <div style="margin-top:1rem;">
      <div style="font-weight:600; font-size:0.775rem; text-transform:uppercase; letter-spacing:0.04em; color:var(--text-subtle); margin-bottom:0.25rem;">Razorpay Audit Payload</div>
      <pre class="json-box">${JSON.stringify(dec, null, 2)}</pre>
    </div>
  `;
}

function runQuickDemo(type) {
  const input = document.getElementById("chat-input");
  if (!input) return;
  if (type === 1) input.value = "Order groceries for ₹400 from Zepto for user_rahul";
  if (type === 2) input.value = "Order food for ₹500 from Swiggy for user_rahul";
  if (type === 3) input.value = "Buy electronics for ₹4500 from Blinkit for user_rahul";
  if (type === 4) input.value = "Order groceries for ₹300 from Blinkit for user_rahul";
  handleChatSubmit();
}

async function seedDb() {
  try {
    const res = await fetch(`${API_BASE}/gate/mandates`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        mandate_id: "mandate_seed_reset_" + Date.now(),
        user_id: "user_rahul",
        merchant_id: "zepto",
        max_per_txn: 80000,
        max_per_day: 200000,
        max_total: 1000000
      })
    });
    loadMandates();
    loadAuditLogs();
  } catch (err) {
    console.error("Failed to seed mandates", err);
  }
}

function openNewMandateModal() {
  const modal = document.getElementById("modal-mandate");
  if (modal) modal.classList.add("active");
}

function closeNewMandateModal() {
  const modal = document.getElementById("modal-mandate");
  if (modal) modal.classList.remove("active");
}

async function submitNewMandate(e) {
  e.preventDefault();
  const id = document.getElementById("m-id").value;
  const user = document.getElementById("m-user").value;
  const merchant = document.getElementById("m-merchant").value;
  const txn = parseInt(document.getElementById("m-max-txn").value) * 100;
  const day = parseInt(document.getElementById("m-max-day").value) * 100;
  const total = parseInt(document.getElementById("m-max-total").value) * 100;

  try {
    const res = await fetch(`${API_BASE}/gate/mandates`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        mandate_id: id,
        user_id: user,
        merchant_id: merchant,
        max_per_txn: txn,
        max_per_day: day,
        max_total: total
      })
    });
    const data = await res.json();
    if (res.ok) {
      closeNewMandateModal();
      loadMandates();
    } else {
      console.error("Error creating mandate", data);
    }
  } catch (err) {
    console.error("Failed to create mandate", err);
  }
}
