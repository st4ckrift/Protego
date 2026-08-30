import requests
import uuid
import logging
from typing import Dict, Any, Tuple, Optional
from gate.config import RAZORPAY_KEY_ID, RAZORPAY_KEY_SECRET

logger = logging.getLogger(__name__)

class RazorpayAPIError(Exception):
    def __init__(self, status_code: int, message: str, raw_response: Any = None):
        self.status_code = status_code
        self.message = message
        self.raw_response = raw_response
        super().__init__(f"Razorpay API Error ({status_code}): {message}")

def create_razorpay_order(
    amount: int,
    user_id: str,
    merchant_id: str,
    mandate_id: str,
    key_id: str = RAZORPAY_KEY_ID,
    key_secret: str = RAZORPAY_KEY_SECRET
) -> Tuple[str, Dict[str, Any]]:
    """
    Creates a Razorpay order in test mode.
    If valid Razorpay credentials are provided, calls https://api.razorpay.com/v1/orders.
    If credentials are missing or default placeholder, returns simulated test order.
    Raises RazorpayAPIError on API failure.
    """
    receipt_id = f"rcpt_gate_{uuid.uuid4().hex[:12]}"
    
    # Check if real keys are configured
    is_real_key = bool(key_id and key_secret and not key_id.startswith("rzp_test_your_") and not key_secret.startswith("your_test_"))
    
    if is_real_key:
        url = "https://api.razorpay.com/v1/orders"
        payload = {
            "amount": amount,
            "currency": "INR",
            "receipt": receipt_id,
            "notes": {
                "mandate_id": mandate_id,
                "user_id": user_id,
                "merchant_id": merchant_id,
                "gated_by": "Mandate-Spend-Gate"
            }
        }
        try:
            resp = requests.post(
                url,
                json=payload,
                auth=(key_id, key_secret),
                timeout=10
            )
            if resp.status_code == 200 or resp.status_code == 201:
                data = resp.json()
                logger.info(f"Successfully created Razorpay order: {data.get('id')}")
                return data.get("id"), data
            else:
                err_msg = resp.text
                try:
                    err_json = resp.json()
                    err_msg = err_json.get("error", {}).get("description", resp.text)
                except Exception:
                    pass
                logger.error(f"Razorpay order creation failed [{resp.status_code}]: {err_msg}")
                raise RazorpayAPIError(resp.status_code, err_msg, resp.text)
        except requests.RequestException as exc:
            logger.error(f"Network error calling Razorpay API: {exc}")
            raise RazorpayAPIError(503, f"Razorpay API connection error: {str(exc)}")

    # Fallback simulation mode for testing / offline demo when API keys are not provided
    simulated_order_id = f"order_test_{uuid.uuid4().hex[:14]}"
    simulated_response = {
        "id": simulated_order_id,
        "entity": "order",
        "amount": amount,
        "amount_paid": 0,
        "amount_due": amount,
        "currency": "INR",
        "receipt": receipt_id,
        "status": "created",
        "attempts": 0,
        "notes": {
            "mandate_id": mandate_id,
            "user_id": user_id,
            "merchant_id": merchant_id,
            "mode": "simulated_test_mode"
        },
        "created_at": int(requests.utils.datetime.now().timestamp()) if hasattr(requests.utils, "datetime") else 1700000000
    }
    return simulated_order_id, simulated_response
