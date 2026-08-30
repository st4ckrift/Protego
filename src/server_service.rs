use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::thread;

use crate::config::Config;
use crate::database::{init_db, DbConnection};
use crate::models::{AuthorizeRequest, escape_json};
use crate::gate_service::{authorize_transaction, get_today_date_str};
use crate::audit_service::get_audit_logs;
use crate::agent_service::process_agent_request;

pub fn start_server(cfg: Config) -> Result<(), String> {
    init_db(&cfg.db_path)?;
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = TcpListener::bind(&addr).map_err(|e| format!("Failed to bind server at {}: {}", addr, e))?;
    println!("🛡️  Spend-Gate Server (Rust Native) running at http://{}", addr);

    let cfg_arc = Arc::new(cfg);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cfg_clone = Arc::clone(&cfg_arc);
                thread::spawn(move || {
                    handle_connection(stream, &cfg_clone);
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, cfg: &Config) {
    let mut buffer = [0u8; 8192];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let req_str = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = req_str.lines();
    let first_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0];
    let full_path = parts[1];

    if method == "OPTIONS" {
        send_response(&mut stream, 200, "text/plain", "OK", true);
        return;
    }

    let (path, query) = match full_path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (full_path, ""),
    };

    let mut body = "";
    if let Some(pos) = req_str.find("\r\n\r\n") {
        body = &req_str[pos + 4..];
    }

    if method == "GET" {
        match path {
            "/" | "/index.html" => serve_file(&mut stream, "static/index.html", "text/html"),
            "/style.css" => serve_file(&mut stream, "static/style.css", "text/css"),
            "/app.js" => serve_file(&mut stream, "static/app.js", "application/javascript"),
            "/gate/mandates" => {
                let conn = DbConnection::open(&cfg.db_path).unwrap();
                let today_str = get_today_date_str();
                let mandates = conn.query_all_mandates().unwrap_or_default();
                let json_items: Vec<String> = mandates
                    .iter()
                    .map(|m| {
                        let cur_today = if m.last_spent_date == today_str { m.spent_today } else { 0 };
                        m.to_json(Some(cur_today))
                    })
                    .collect();
                let resp_body = format!(r#"{{"status":"success","mandates":[{}]}}"#, json_items.join(","));
                send_response(&mut stream, 200, "application/json", &resp_body, true);
            }
            "/gate/audit" => {
                let conn = DbConnection::open(&cfg.db_path).unwrap();
                let mandate_id = extract_query_param(query, "mandate_id");
                let decision = extract_query_param(query, "decision");
                let limit = extract_query_param(query, "limit").and_then(|l| l.parse().ok()).unwrap_or(50);

                let logs = get_audit_logs(&conn, mandate_id.as_deref(), decision.as_deref(), limit).unwrap_or_default();
                let json_logs: Vec<String> = logs.iter().map(|l| l.to_json()).collect();
                let resp_body = format!(r#"{{"status":"success","count":{},"logs":[{}]}}"#, logs.len(), json_logs.join(","));
                send_response(&mut stream, 200, "application/json", &resp_body, true);
            }
            _ => send_response(&mut stream, 404, "application/json", r#"{"error":"Not Found"}"#, true),
        }
    } else if method == "POST" {
        match path {
            "/gate/authorize" => {
                let user_id = extract_json_field(body, "user_id").unwrap_or_default();
                let merchant_id = extract_json_field(body, "merchant_id").unwrap_or_default();
                let mandate_id = extract_json_field(body, "mandate_id");
                let amount = extract_json_num(body, "amount").unwrap_or(0);

                if user_id.is_empty() || merchant_id.is_empty() || amount <= 0 {
                    send_response(&mut stream, 400, "application/json", r#"{"error":"Missing user_id, merchant_id, or amount"}"#, true);
                    return;
                }

                let req = AuthorizeRequest {
                    user_id,
                    merchant_id,
                    amount,
                    mandate_id,
                };

                let conn = DbConnection::open(&cfg.db_path).unwrap();
                let dec = authorize_transaction(&conn, &req, cfg);
                let status_code = if dec.decision == "approved" { 200 } else { 400 };
                send_response(&mut stream, status_code, "application/json", &dec.to_json(), true);
            }
            "/gate/mandates" => {
                let mandate_id = extract_json_field(body, "mandate_id").unwrap_or_default();
                let user_id = extract_json_field(body, "user_id").unwrap_or_default();
                let merchant_id = extract_json_field(body, "merchant_id").unwrap_or_default();
                let max_per_txn = extract_json_num(body, "max_per_txn").unwrap_or(0);
                let max_per_day = extract_json_num(body, "max_per_day").unwrap_or(0);
                let max_total = extract_json_num(body, "max_total").unwrap_or(0);

                if mandate_id.is_empty() || user_id.is_empty() || merchant_id.is_empty() {
                    send_response(&mut stream, 400, "application/json", r#"{"error":"Missing mandate configuration fields"}"#, true);
                    return;
                }

                let conn = DbConnection::open(&cfg.db_path).unwrap();
                let today_str = get_today_date_str();
                let sql = format!(
                    "INSERT INTO mandates (mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total, spent_today, spent_total, last_spent_date, expires_at, created_at, status) VALUES ('{}', '{}', '{}', {}, {}, {}, 0, 0, '{}', '2027-12-31T23:59:59Z', '2026-08-30T00:00:00Z', 'active');",
                    escape_json(&mandate_id), escape_json(&user_id), escape_json(&merchant_id), max_per_txn, max_per_day, max_total, today_str
                );
                match conn.execute(&sql) {
                    Ok(_) => send_response(&mut stream, 201, "application/json", &format!(r#"{{"status":"success","mandate_id":"{}"}}"#, mandate_id), true),
                    Err(e) => send_response(&mut stream, 400, "application/json", &format!(r#"{{"error":"{}"}}"#, escape_json(&e)), true),
                }
            }
            "/agent/chat" => {
                let prompt = extract_json_field(body, "prompt").unwrap_or_default();
                let user_override = extract_json_field(body, "user_id");

                if prompt.is_empty() {
                    send_response(&mut stream, 400, "application/json", r#"{"error":"Missing prompt"}"#, true);
                    return;
                }

                let (agent_msg, decision) = process_agent_request(&prompt, user_override.as_deref(), cfg);
                let resp_body = format!(
                    r#"{{"status":"completed","prompt":"{}","agent_message":"{}","parsed_intent":{{"user_id":"{}","merchant_id":"{}","amount_paise":{},"amount_formatted":"₹{:.2}"}},"gate_decision":{}}}"#,
                    escape_json(&prompt),
                    escape_json(&agent_msg),
                    escape_json(&decision.user_id),
                    escape_json(&decision.merchant_id),
                    decision.requested_amount,
                    (decision.requested_amount as f64) / 100.0,
                    decision.to_json()
                );
                send_response(&mut stream, 200, "application/json", &resp_body, true);
            }
            _ => send_response(&mut stream, 404, "application/json", r#"{"error":"Not Found"}"#, true),
        }
    }
}

fn serve_file(stream: &mut TcpStream, file_path: &str, mime_type: &str) {
    if Path::new(file_path).exists() {
        if let Ok(content) = std::fs::read_to_string(file_path) {
            send_response(stream, 200, mime_type, &content, false);
            return;
        }
    }
    send_response(stream, 404, "text/plain", "File Not Found", false);
}

fn send_response(stream: &mut TcpStream, status_code: u16, content_type: &str, body: &str, cors: bool) {
    let status_text = match status_code {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };

    let mut response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        status_code,
        status_text,
        content_type,
        body.len()
    );

    if cors {
        response.push_str("Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n");
    }

    response.push_str("\r\n");
    response.push_str(body);

    let _ = stream.write_all(response.as_bytes());
}

fn extract_query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn extract_json_field(json: &str, key: &str) -> Option<String> {
    let pattern = format!(r#""{}":"#, key);
    if let Some(pos) = json.find(&pattern) {
        let start = pos + pattern.len();
        let rest = &json[start..];
        let trimmed = rest.trim_start();
        if trimmed.starts_with('"') {
            let inner = &trimmed[1..];
            if let Some(end) = inner.find('"') {
                return Some(inner[..end].to_string());
            }
        }
    }
    None
}

fn extract_json_num(json: &str, key: &str) -> Option<i64> {
    let pattern = format!(r#""{}":"#, key);
    if let Some(pos) = json.find(&pattern) {
        let start = pos + pattern.len();
        let rest = &json[start..];
        let trimmed = rest.trim_start();
        let num_str: String = trimmed.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
        if let Ok(val) = num_str.parse::<i64>() {
            return Some(val);
        }
    }
    None
}
