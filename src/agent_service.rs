use crate::config::Config;
use crate::database::DbConnection;
use crate::models::{AuthorizeRequest, GateDecision};
use crate::nlu_parser::parse_intent_fallback;
use crate::gate_service::{authorize_transaction, format_inr};

pub fn process_agent_request(prompt: &str, user_id_override: Option<&str>, cfg: &Config) -> (String, GateDecision) {
    let mut intent = parse_intent_fallback(prompt);
    if let Some(uid) = user_id_override {
        intent.user_id = uid.to_string();
    }

    let conn = DbConnection::open(&cfg.db_path).unwrap();

    if intent.amount_paise <= 0 {
        let dec = GateDecision {
            decision: "error".to_string(),
            reason: "invalid_amount".to_string(),
            mandate_id: None,
            user_id: intent.user_id.clone(),
            merchant_id: intent.merchant_id.clone(),
            requested_amount: 0,
            limit: None,
            already_spent: None,
            razorpay_order_id: None,
            updated_mandate: None,
            message: Some("Invalid transaction amount in prompt.".to_string()),
        };
        let msg = "I couldn't identify a valid transaction amount from your request. Please specify an amount, e.g., 'Spend ₹500 at Zepto'.".to_string();
        return (msg, dec);
    }

    let req = AuthorizeRequest {
        user_id: intent.user_id.clone(),
        merchant_id: intent.merchant_id.clone(),
        amount: intent.amount_paise,
        mandate_id: None,
    };

    let decision = authorize_transaction(&conn, &req, cfg);

    let message = if decision.decision == "approved" {
        format!(
            "✅ **Transaction Approved by Spend-Gate**\n- Merchant: `{}`\n- Amount: `{}`\n- Mandate ID: `{}`\n- Razorpay Order ID: `{}`\n\nThe payment authorization passed all 5 mandate checks and the Razorpay test order has been generated.",
            intent.merchant_id.to_uppercase(),
            format_inr(intent.amount_paise),
            decision.mandate_id.as_deref().unwrap_or("none"),
            decision.razorpay_order_id.as_deref().unwrap_or("none")
        )
    } else if decision.decision == "denied" {
        format!(
            "🚫 **Transaction Blocked by Spend-Gate**\n- Reason: `{}`\n- Merchant: `{}`\n- Requested Amount: `{}`\n- Mandate ID: `{}`\n\n**Detail:** {}",
            decision.reason,
            intent.merchant_id.to_uppercase(),
            format_inr(intent.amount_paise),
            decision.mandate_id.as_deref().unwrap_or("none"),
            decision.message.as_deref().unwrap_or("Denied")
        )
    } else {
        format!(
            "⚠️ **Gate Service Error**\n- Reason: `{}`\n- Detail: {}",
            decision.reason,
            decision.message.as_deref().unwrap_or("Error")
        )
    };

    (message, decision)
}
