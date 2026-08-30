use std::env;
use std::fs;
use std::path::Path;

pub struct Config {
    pub db_path: String,
    pub port: u16,
    pub host: String,
    pub razorpay_key_id: String,
    pub razorpay_key_secret: String,
    pub openai_api_key: String,
}

impl Config {
    pub fn load() -> Self {
        // Simple dotenv loader
        if Path::new(".env").exists() {
            if let Ok(content) = fs::read_to_string(".env") {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        if let Some((key, val)) = trimmed.split_once('=') {
                            if env::var(key.trim()).is_err() {
                                env::set_var(key.trim(), val.trim());
                            }
                        }
                    }
                }
            }
        }

        Self {
            db_path: env::var("DB_PATH").unwrap_or_else(|_| "mandate_gate.db".to_string()),
            port: env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8000),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            razorpay_key_id: env::var("RAZORPAY_KEY_ID").unwrap_or_default(),
            razorpay_key_secret: env::var("RAZORPAY_KEY_SECRET").unwrap_or_default(),
            openai_api_key: env::var("OPENAI_API_KEY").unwrap_or_default(),
        }
    }
}
