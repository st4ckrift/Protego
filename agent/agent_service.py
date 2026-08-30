import logging
from typing import Dict, Any
from agent.nlu_parser import parse_intent_llm
from gate.models import AuthorizeRequest, GateDecision
from gate.gate_service import authorize_transaction, format_inr

logger = logging.getLogger(__name__)

def process_agent_request(user_prompt: str, user_id_override: str = None) -> Dict[str, Any]:
    """
    Agent workflow:
    1. Parse natural language intent into structured parameters.
    2. Forward request ONLY to the Mandate Spend-Gate service (never to Razorpay directly).
    3. Return plain language explanation of the gate's decision.
    """
    intent = parse_intent_llm(user_prompt)
    
    user_id = user_id_override if user_id_override else intent["user_id"]
    merchant_id = intent["merchant_id"]
    amount = intent["amount"]

    if amount <= 0:
        return {
            "status": "error",
            "prompt": user_prompt,
            "agent_message": "I couldn't identify a valid transaction amount from your request. Please specify an amount, e.g., 'Spend ₹500 at Zepto'.",
            "gate_decision": None,
            "parsed_intent": intent
        }

    # Prepare gate authorization request
    auth_req = AuthorizeRequest(
        user_id=user_id,
        merchant_id=merchant_id,
        amount=amount
    )

    # Call Gate Service
    decision: GateDecision = authorize_transaction(auth_req)
    dec_dict = decision.to_dict()

    # Build human explanation
    if decision.decision == "approved":
        agent_msg = (
            f"✅ **Transaction Approved by Spend-Gate**\n"
            f"- Merchant: `{merchant_id.upper()}`\n"
            f"- Amount: `{format_inr(amount)}`\n"
            f"- Mandate ID: `{decision.mandate_id}`\n"
            f"- Razorpay Order ID: `{decision.razorpay_order_id}`\n\n"
            f"The payment authorization passed all 5 mandate checks and the Razorpay test order has been generated."
        )
    elif decision.decision == "denied":
        agent_msg = (
            f"🚫 **Transaction Blocked by Spend-Gate**\n"
            f"- Reason: `{decision.reason}`\n"
            f"- Merchant: `{merchant_id.upper()}`\n"
            f"- Requested Amount: `{format_inr(amount)}`\n"
            f"- Mandate ID: `{decision.mandate_id}`\n\n"
            f"**Detail:** {decision.message}"
        )
    else:
        agent_msg = (
            f"⚠️ **Gate Service Error**\n"
            f"- Reason: `{decision.reason}`\n"
            f"- Detail: {decision.message}"
        )

    return {
        "status": "completed",
        "prompt": user_prompt,
        "agent_message": agent_msg,
        "parsed_intent": {
            "user_id": user_id,
            "merchant_id": merchant_id,
            "amount_paise": amount,
            "amount_formatted": format_inr(amount),
            "parser_used": intent.get("parser", "unknown")
        },
        "gate_decision": dec_dict
    }
