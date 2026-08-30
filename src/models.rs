use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MandateStatus {
    Active,
    Expired,
    Revoked,
}

impl MandateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MandateStatus::Active => "active",
            MandateStatus::Expired => "expired",
            MandateStatus::Revoked => "revoked",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "expired" => MandateStatus::Expired,
            "revoked" => MandateStatus::Revoked,
            _ => MandateStatus::Active,
        }
    }
}

impl fmt::Display for MandateStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct MandateModel {
    pub mandate_id: String,
    pub user_id: String,
    pub merchant_id: String,
    pub max_per_txn: i64,
    pub max_per_day: i64,
    pub max_total: i64,
    pub spent_today: i64,
    pub spent_total: i64,
    pub last_spent_date: String,
    pub expires_at: String,
    pub created_at: String,
    pub status: MandateStatus,
}

impl MandateModel {
    pub fn to_json(&self, spent_today_current: Option<i64>) -> String {
        let current_today = spent_today_current.unwrap_or(self.spent_today);
        let day_pct = if self.max_per_day > 0 {
            (current_today as f64 / self.max_per_day as f64) * 100.0
        } else {
            0.0
        };
        let total_pct = if self.max_total > 0 {
            (self.spent_total as f64 / self.max_total as f64) * 100.0
        } else {
            0.0
        };

        format!(
            r#"{{"mandate_id":"{}","user_id":"{}","merchant_id":"{}","max_per_txn":{},"max_per_day":{},"max_total":{},"spent_today":{},"spent_total":{},"spent_today_current":{},"daily_utilization_pct":{:.1},"total_utilization_pct":{:.1},"last_spent_date":"{}","expires_at":"{}","created_at":"{}","status":"{}"}}"#,
            escape_json(&self.mandate_id),
            escape_json(&self.user_id),
            escape_json(&self.merchant_id),
            self.max_per_txn,
            self.max_per_day,
            self.max_total,
            self.spent_today,
            self.spent_total,
            current_today,
            day_pct,
            total_pct,
            escape_json(&self.last_spent_date),
            escape_json(&self.expires_at),
            escape_json(&self.created_at),
            self.status.as_str()
        )
    }
}

#[derive(Debug, Clone)]
pub struct AuthorizeRequest {
    pub user_id: String,
    pub merchant_id: String,
    pub amount: i64,
    pub mandate_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GateDecision {
    pub decision: String,
    pub reason: String,
    pub mandate_id: Option<String>,
    pub user_id: String,
    pub merchant_id: String,
    pub requested_amount: i64,
    pub limit: Option<i64>,
    pub already_spent: Option<i64>,
    pub razorpay_order_id: Option<String>,
    pub updated_mandate: Option<MandateModel>,
    pub message: Option<String>,
}

impl GateDecision {
    pub fn to_json(&self) -> String {
        let mut fields = vec![
            format!(r#""decision":"{}""#, escape_json(&self.decision)),
            format!(r#""reason":"{}""#, escape_json(&self.reason)),
            format!(
                r#""mandate_id":{}"#,
                self.mandate_id
                    .as_ref()
                    .map(|id| format!(r#""{}""#, escape_json(id)))
                    .unwrap_or_else(|| "null".to_string())
            ),
            format!(r#""user_id":"{}""#, escape_json(&self.user_id)),
            format!(r#""merchant_id":"{}""#, escape_json(&self.merchant_id)),
            format!(r#""requested_amount":{}"#, self.requested_amount),
        ];

        if let Some(lim) = self.limit {
            fields.push(format!(r#""limit":{}"#, lim));
        }
        if let Some(spent) = self.already_spent {
            fields.push(format!(r#""already_spent":{}"#, spent));
        }
        if let Some(ref rzp_id) = self.razorpay_order_id {
            fields.push(format!(r#""razorpay_order_id":"{}""#, escape_json(rzp_id)));
        }
        if let Some(ref m) = self.updated_mandate {
            fields.push(format!(r#""updated_mandate":{}"#, m.to_json(None)));
        }
        if let Some(ref msg) = self.message {
            fields.push(format!(r#""message":"{}""#, escape_json(msg)));
        }

        format!("{{{}}}", fields.join(","))
    }
}

#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub log_id: String,
    pub timestamp: String,
    pub mandate_id: String,
    pub user_id: String,
    pub merchant_id: String,
    pub requested_amount: i64,
    pub decision: String,
    pub reason: String,
    pub details_json: String,
    pub razorpay_order_id: Option<String>,
}

impl AuditLogEntry {
    pub fn to_json(&self) -> String {
        let rzp_id_str = self
            .razorpay_order_id
            .as_ref()
            .map(|id| format!(r#""{}""#, escape_json(id)))
            .unwrap_or_else(|| "null".to_string());

        format!(
            r#"{{"log_id":"{}","timestamp":"{}","mandate_id":"{}","user_id":"{}","merchant_id":"{}","requested_amount":{},"decision":"{}","reason":"{}","details":{},"razorpay_order_id":{}}}"#,
            escape_json(&self.log_id),
            escape_json(&self.timestamp),
            escape_json(&self.mandate_id),
            escape_json(&self.user_id),
            escape_json(&self.merchant_id),
            self.requested_amount,
            escape_json(&self.decision),
            escape_json(&self.reason),
            if self.details_json.is_empty() { "{}" } else { &self.details_json },
            rzp_id_str
        )
    }
}

pub fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
