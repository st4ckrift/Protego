import re
import json
import logging
import requests
from typing import Dict, Any, Optional
from gate.config import OPENAI_API_KEY

logger = logging.getLogger(__name__)

KNOWN_MERCHANTS = ["zepto", "swiggy", "amazon", "blinkit", "zomato", "bigbasket", "uber", "flipkart"]

def parse_intent_fallback(text: str) -> Dict[str, Any]:
    """
    Deterministic rule-based & regex parser to extract spend intent from prompt.
    Does not require external LLM API keys.
    """
    text_lower = text.lower()

    # Extract user_id
    user_id = "user_rahul"  # default demo user
    user_match = re.search(r"user[_\s]?([a-zA-Z0-9]+)", text_lower)
    if user_match:
        user_id = f"user_{user_match.group(1)}"
    elif "priya" in text_lower:
        user_id = "user_priya"
    elif "rahul" in text_lower:
        user_id = "user_rahul"

    # Extract merchant_id
    merchant_id = "zepto"  # default
    for m in KNOWN_MERCHANTS:
        if m in text_lower:
            merchant_id = m
            break
    else:
        # Check prepositions "from X", "at X", "on X"
        prep_match = re.search(r"(?:from|at|on)\s+([a-zA-Z0-9]+)", text_lower)
        if prep_match:
            merchant_id = prep_match.group(1)

    # Extract amount
    amount_paise = 0
    # Patterns: ₹800, rs 800, 800 rupees, 800 inr, under 800, for 800
    curr_match = re.search(r"(?:₹|rs\.?|inr|rupees)\s*([\d,]+(?:\.\d{1,2})?)", text_lower)
    if not curr_match:
        curr_match = re.search(r"([\d,]+(?:\.\d{1,2})?)\s*(?:₹|rs\.?|inr|rupees)", text_lower)
    if not curr_match:
        curr_match = re.search(r"(?:under|for|worth|amount of|upto|up to)\s*([\d,]+(?:\.\d{1,2})?)", text_lower)
    if not curr_match:
        # Fallback to first standalone number
        curr_match = re.search(r"\b(\d+)\b", text_lower)

    if curr_match:
        val_str = curr_match.group(1).replace(",", "")
        try:
            rupees = float(val_str)
            amount_paise = int(round(rupees * 100))
        except ValueError:
            amount_paise = 0

    return {
        "user_id": user_id,
        "merchant_id": merchant_id,
        "amount": amount_paise,
        "intent_summary": text,
        "parser": "deterministic"
    }

def parse_intent_llm(text: str) -> Dict[str, Any]:
    """
    Parses intent using OpenAI API if OPENAI_API_KEY is available.
    Falls back to deterministic parser if API fails or key missing.
    """
    if not OPENAI_API_KEY or OPENAI_API_KEY.startswith("your_"):
        return parse_intent_fallback(text)

    try:
        headers = {
            "Authorization": f"Bearer {OPENAI_API_KEY}",
            "Content-Type": "application/json"
        }
        prompt_payload = {
            "model": "gpt-4o-mini",
            "response_format": {"type": "json_object"},
            "messages": [
                {
                    "role": "system",
                    "content": (
                        "You are an NLU parser for an agentic commerce gate. Extract details from user spend prompts into JSON with keys: "
                        "'user_id' (string, default 'user_rahul'), 'merchant_id' (string, lowercase), 'amount_rupees' (number). "
                        "Return ONLY valid JSON."
                    )
                },
                {"role": "user", "content": text}
            ]
        }
        resp = requests.post("https://api.openai.com/v1/chat/completions", json=prompt_payload, headers=headers, timeout=5)
        if resp.status_code == 200:
            content = resp.json()["choices"][0]["message"]["content"]
            parsed = json.loads(content)
            rupees = float(parsed.get("amount_rupees", 0))
            return {
                "user_id": str(parsed.get("user_id", "user_rahul")),
                "merchant_id": str(parsed.get("merchant_id", "zepto")).lower(),
                "amount": int(round(rupees * 100)),
                "intent_summary": text,
                "parser": "openai_gpt4o_mini"
            }
    except Exception as exc:
        logger.warning(f"OpenAI NLU failed, falling back to deterministic parser: {exc}")
        
    return parse_intent_fallback(text)
