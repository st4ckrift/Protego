#[derive(Debug, Clone)]
pub struct ParsedIntent {
    pub user_id: String,
    pub merchant_id: String,
    pub amount_paise: i64,
    pub original_prompt: String,
    pub parser_used: String,
}

pub fn parse_intent_fallback(text: &str) -> ParsedIntent {
    let lower = text.to_lowercase();

    // User ID extraction
    let user_id = if lower.contains("user_priya") || lower.contains("priya") {
        "user_priya".to_string()
    } else {
        "user_rahul".to_string()
    };

    // Merchant ID extraction
    let known_merchants = ["zepto", "swiggy", "amazon", "blinkit", "zomato", "bigbasket", "uber", "flipkart"];
    let mut merchant_id = "zepto".to_string();
    for m in known_merchants {
        if lower.contains(m) {
            merchant_id = m.to_string();
            break;
        }
    }

    // Amount extraction (in rupees -> paise)
    let amount_paise = extract_amount_paise(&lower);

    ParsedIntent {
        user_id,
        merchant_id,
        amount_paise,
        original_prompt: text.to_string(),
        parser_used: "deterministic_regex".to_string(),
    }
}

fn extract_amount_paise(lower: &str) -> i64 {
    // Look for numbers following currency symbols or keywords
    let words: Vec<&str> = lower.split_whitespace().collect();
    for i in 0..words.len() {
        let w = words[i].trim_matches(|c: char| !c.is_alphanumeric() && c != '.');
        if w.starts_with('₹') || w.starts_with("rs") || w.starts_with("inr") {
            let num_part: String = w.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
            if let Ok(val) = num_part.parse::<f64>() {
                return (val * 100.0).round() as i64;
            }
        }
        if let Ok(val) = w.parse::<f64>() {
            if val > 0.0 && val < 1_000_000.0 {
                // Check context words
                if i > 0 && (words[i-1].contains('₹') || words[i-1].contains("rs") || words[i-1].contains("under") || words[i-1].contains("for") || words[i-1].contains("worth")) {
                    return (val * 100.0).round() as i64;
                }
                if i + 1 < words.len() && (words[i+1].contains("rupees") || words[i+1].contains("inr") || words[i+1].contains("rs")) {
                    return (val * 100.0).round() as i64;
                }
            }
        }
    }

    // Fallback: search raw digits in text
    let mut num_str = String::new();
    let mut found_digit = false;
    for ch in lower.chars() {
        if ch.is_ascii_digit() {
            num_str.push(ch);
            found_digit = true;
        } else if found_digit {
            break;
        }
    }
    if let Ok(val) = num_str.parse::<i64>() {
        return val * 100;
    }

    0
}
