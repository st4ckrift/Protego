from dataclasses import dataclass, asdict
from typing import Optional, Dict, Any
from datetime import datetime

@dataclass
class MandateModel:
    mandate_id: str
    user_id: str
    merchant_id: str
    max_per_txn: int    # in paise
    max_per_day: int    # in paise
    max_total: int      # in paise
    spent_today: int    # in paise
    spent_total: int    # in paise
    last_spent_date: str # YYYY-MM-DD
    expires_at: str     # ISO format string
    created_at: str     # ISO format string
    status: str         # 'active', 'expired', 'revoked'

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)

@dataclass
class AuthorizeRequest:
    user_id: str
    merchant_id: str
    amount: int  # in paise
    mandate_id: Optional[str] = None

@dataclass
class GateDecision:
    decision: str  # "approved" or "denied" or "error"
    reason: str
    mandate_id: Optional[str]
    user_id: str
    merchant_id: str
    requested_amount: int
    limit: Optional[int] = None
    already_spent: Optional[int] = None
    razorpay_order_id: Optional[str] = None
    updated_mandate: Optional[Dict[str, Any]] = None
    message: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        res = {
            "decision": self.decision,
            "reason": self.reason,
            "mandate_id": self.mandate_id,
            "user_id": self.user_id,
            "merchant_id": self.merchant_id,
            "requested_amount": self.requested_amount,
        }
        if self.limit is not None:
            res["limit"] = self.limit
        if self.already_spent is not None:
            res["already_spent"] = self.already_spent
        if self.razorpay_order_id:
            res["razorpay_order_id"] = self.razorpay_order_id
        if self.updated_mandate:
            res["updated_mandate"] = self.updated_mandate
        if self.message:
            res["message"] = self.message
        return res

@dataclass
class AuditLogEntry:
    log_id: str
    timestamp: str
    mandate_id: str
    user_id: str
    merchant_id: str
    requested_amount: int
    decision: str
    reason: str
    details: Dict[str, Any]
    razorpay_order_id: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "log_id": self.log_id,
            "timestamp": self.timestamp,
            "mandate_id": self.mandate_id,
            "user_id": self.user_id,
            "merchant_id": self.merchant_id,
            "requested_amount": self.requested_amount,
            "decision": self.decision,
            "reason": self.reason,
            "details": self.details,
            "razorpay_order_id": self.razorpay_order_id,
        }
