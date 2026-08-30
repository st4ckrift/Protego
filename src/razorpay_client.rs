use std::fmt;

#[derive(Debug, Clone)]
pub struct RazorpayError {
    pub status_code: u16,
    pub message: String,
}

impl fmt::Display for RazorpayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Razorpay API Error ({}): {}", self.status_code, self.message)
    }
}

impl std::error::Error for RazorpayError {}

#[derive(Debug, Clone)]
pub struct RazorpayOrder {
    pub id: String,
    pub amount: i64,
    pub currency: String,
    pub status: String,
}

pub fn create_razorpay_order(
    amount: i64,
    user_id: &str,
    merchant_id: &str,
    mandate_id: &str,
    key_id: &str,
    key_secret: &str,
) -> Result<RazorpayOrder, RazorpayError> {
    let is_real_key = !key_id.is_empty()
        && !key_secret.is_empty()
        && !key_id.starts_with("rzp_test_your_")
        && !key_secret.starts_with("your_test_");

    if is_real_key {
        // Build curl command for Razorpay API call
        use std::process::Command;
        let auth_str = format!("{}:{}", key_id, key_secret);
        let payload = format!(
            r#"{{"amount":{},"currency":"INR","receipt":"rcpt_gate_{}","notes":{{"mandate_id":"{}","user_id":"{}","merchant_id":"{}"}}}}"#,
            amount,
            generate_random_hex(6),
            mandate_id,
            user_id,
            merchant_id
        );

        let output = Command::new("curl")
            .arg("-s")
            .arg("-X").arg("POST")
            .arg("https://api.razorpay.com/v1/orders")
            .arg("-u").arg(&auth_str)
            .arg("-H").arg("Content-Type: application/json")
            .arg("-d").arg(&payload)
            .output();

        match output {
            Ok(out) => {
                let resp_str = String::from_utf8_lossy(&out.stdout).to_string();
                if resp_str.contains(r#""id":"order_"#) {
                    if let Some(id) = extract_json_string(&resp_str, "id") {
                        return Ok(RazorpayOrder {
                            id,
                            amount,
                            currency: "INR".to_string(),
                            status: "created".to_string(),
                        });
                    }
                }
                let err_msg = extract_json_string(&resp_str, "description")
                    .unwrap_or_else(|| resp_str.clone());
                return Err(RazorpayError {
                    status_code: 400,
                    message: err_msg,
                });
            }
            Err(e) => {
                return Err(RazorpayError {
                    status_code: 503,
                    message: format!("Network error reaching Razorpay: {}", e),
                });
            }
        }
    }

    // Simulated test order mode
    let simulated_id = format!("order_test_{}", generate_random_hex(7));
    Ok(RazorpayOrder {
        id: simulated_id,
        amount,
        currency: "INR".to_string(),
        status: "created".to_string(),
    })
}

fn generate_random_hex(len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
    let s = format!("{:x}{:x}", nanos, std::process::id());
    s.chars().take(len).collect()
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let search = format!(r#""{}":"#, key);
    if let Some(pos) = json.find(&search) {
        let start = pos + search.len();
        let rest = &json[start..];
        if rest.starts_with('"') {
            let inner = &rest[1..];
            if let Some(end) = inner.find('"') {
                return Some(inner[..end].to_string());
            }
        }
    }
    None
}
