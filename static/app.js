const API_BASE = "";

document.addEventListener("DOMContentLoaded", () => {
  loadMandates();
  loadAuditLogs();
});

function switchTab(tabId) {
  document.querySelectorAll(".tab-btn").forEach(btn => btn.classList.remove("active"));
  document.querySelectorAll(".tab-content").forEach(c => c.classList.remove("active"));
  
  const targetBtn = Array.from(document.querySelectorAll(".tab-btn")).find(b => b.getAttribute("onclick").includes(tabId));
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
  document.getElementById("mandate-count").innerText = mandates.length;
  if (!container) return;

  if (mandates.length === 0) {
    container.innerHTML = `<div class="placeholder-state"><p>No mandates found. Click Reseed Mandates.</p></div>`;
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

    return `
      <div class="mandate-card">
        <div class="mandate-card-header">
          <div>
            <span class="merchant-tag">${m.merchant_id.toUpperCase()}</span>
            <h3 style="margin-top:0.3rem; font-size:1rem;">${m.mandate_id}</h3>
            <p class="text-muted" style="font-size:0.8rem;">User: <strong>${m.user_id}</strong></p>
          </div>
          ${statusBadge}
        </div>

        <div style="font-size:0.85rem; margin-bottom:0.5rem;">
          <div>Single Txn Cap: <strong>${formatINR(m.max_per_txn)}</strong></div>
        </div>

        <div class="progress-bar-container">
          <div class="progress-label">
            <span>Daily Spend: ${formatINR(m.spent_today_current)} / ${formatINR(m.max_per_day)}</span>
            <span>${dayPct}%</span>
          </div>
          <div class="progress-track">
            <div class="progress-fill ${dayFillClass}" style="width: ${Math.min(100, dayPct)}%"></div>
          </div>
        </div>

        <div class="progress-bar-container">
          <div class="progress-label">
            <span>Lifetime Spend: ${formatINR(m.spent_total)} / ${formatINR(m.max_total)}</span>
            <span>${totalPct}%</span>
          </div>
          <div class="progress-track">
            <div class="progress-fill ${totalFillClass}" style="width: ${Math.min(100, totalPct)}%"></div>
          </div>
        </div>

        <div style="margin-top:0.75rem; font-size:0.75rem; color:var(--text-muted);">
          Expires: ${m.expires_at.split('T')[0]}
        </div>
      </div>
    `;
  }).join("");
}

async function loadAuditLogs() {
  try {
    const decision = document.getElementById("audit-filter-decision").value;
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
    tbody.innerHTML = `<tr><td colspan="7" style="text-align:center; padding:2rem;">No audit logs recorded yet.</td></tr>`;
    return;
  }

  tbody.innerHTML = logs.map(l => {
    let decBadge = `<span class="badge badge-success">APPROVED</span>`;
    if (l.decision === "denied") decBadge = `<span class="badge badge-danger">DENIED</span>`;
    if (l.decision === "error") decBadge = `<span class="badge badge-warning">ERROR</span>`;

    const rzpOrder = l.razorpay_order_id ? `<code class="font-mono" style="color:#38bdf8;">${l.razorpay_order_id}</code>` : `<span class="text-muted">—</span>`;
    const timeStr = new Date(l.timestamp).toLocaleTimeString();

    return `
      <tr>
        <td class="font-mono" style="font-size:0.8rem;">${timeStr}</td>
        <td class="font-mono"><strong>${l.mandate_id}</strong></td>
        <td>${l.user_id} <span class="text-muted">→</span> <strong>${l.merchant_id}</strong></td>
        <td class="font-mono">${formatINR(l.requested_amount)}</td>
        <td>${decBadge}</td>
        <td class="font-mono" style="font-size:0.8rem;">${l.reason}</td>
        <td>${rzpOrder}</td>
      </tr>
    `;
  }).join("");
}

async function handleChatSubmit(e) {
  if (e) e.preventDefault();
  const input = document.getElementById("chat-input");
  const prompt = input.value.strip ? input.value.strip() : input.value.trim();
  if (!prompt) return;

  appendChatMessage("user", prompt);
  input.value = "";

  try {
    const res = await fetch(`${API_BASE}/agent/chat`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ prompt: prompt })
    });
    const data = await res.json();
    appendChatMessage("agent", data.agent_message);
    renderGateInspector(data);
    loadMandates();
    loadAuditLogs();
  } catch (err) {
    appendChatMessage("agent", `⚠️ Error reaching Gate Service: ${err.message}`);
  }
}

function appendChatMessage(sender, messageText) {
  const container = document.getElementById("chat-history");
  const msgDiv = document.createElement("div");
  msgDiv.className = `chat-message ${sender}-message`;
  
  const icon = sender === "user" ? "👤" : "🛡️";
  const htmlContent = messageText.replace(/\n/g, "<br>").replace(/\*\*(.*?)\*\*/g, "<strong>$1</strong>").replace(/`(.*?)`/g, "<code>$1</code>");

  msgDiv.innerHTML = `
    <div class="msg-avatar">${icon}</div>
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
    <div style="margin-bottom:1rem;">
      <div style="font-weight:600; margin-bottom:0.25rem;">NLU Parsed Intent:</div>
      <div style="font-size:0.8rem; background:#0f172a; padding:0.5rem; border-radius:6px;">
        User: <code>${data.parsed_intent?.user_id}</code> | Merchant: <code>${data.parsed_intent?.merchant_id}</code> | Amount: <code>${data.parsed_intent?.amount_formatted}</code>
      </div>
    </div>

    <div style="font-weight:600; margin-bottom:0.5rem;">5 Mandatory Gate Checks:</div>

    <div class="check-step ${c1 ? 'pass' : 'fail'}">
      <span>1. Mandate Existence & Identity Match</span>
      <span>${c1 ? '✓ PASS' : '✗ FAIL'}</span>
    </div>

    <div class="check-step ${c2 ? 'pass' : 'fail'}">
      <span>2. Status Active & Unexpired Check</span>
      <span>${c2 ? '✓ PASS' : '✗ FAIL'}</span>
    </div>

    <div class="check-step ${c3 ? 'pass' : 'fail'}">
      <span>3. Per-Transaction Limit Check</span>
      <span>${c3 ? '✓ PASS' : '✗ FAIL'}</span>
    </div>

    <div class="check-step ${c4 ? 'pass' : 'fail'}">
      <span>4. Daily Spend Cap Check</span>
      <span>${c4 ? '✓ PASS' : '✗ FAIL'}</span>
    </div>

    <div class="check-step ${c5 ? 'pass' : 'fail'}">
      <span>5. Total Lifetime Cap Check</span>
      <span>${c5 ? '✓ PASS' : '✗ FAIL'}</span>
    </div>

    <div style="margin-top:1rem;">
      <div style="font-weight:600;">Razorpay Audit Payload:</div>
      <pre class="json-box">${JSON.stringify(dec, null, 2)}</pre>
    </div>
  `;
}

function runQuickDemo(type) {
  const input = document.getElementById("chat-input");
  if (type === 1) input.value = "Order groceries for ₹400 from Zepto for user_rahul";
  if (type === 2) input.value = "Order food for ₹500 from Swiggy for user_rahul";
  if (type === 3) input.value = "Buy electronics for ₹4500 from Blinkit for user_rahul";
  if (type === 4) input.value = "Order groceries for ₹300 from Blinkit for user_rahul";
  handleChatSubmit();
}

async function seedDb() {
  alert("Reseeding database with sample mandates...");
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
    console.error(err);
  }
}

function openNewMandateModal() {
  document.getElementById("modal-mandate").classList.add("active");
}

function closeNewMandateModal() {
  document.getElementById("modal-mandate").classList.remove("active");
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
      alert(`Mandate ${id} created successfully!`);
    } else {
      alert(`Error: ${data.error}`);
    }
  } catch (err) {
    alert(`Failed to create mandate: ${err.message}`);
  }
}
