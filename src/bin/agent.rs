use std::env;
use mandate_gate::config::Config;
use mandate_gate::database::init_db;
use mandate_gate::agent_service::process_agent_request;

fn main() -> Result<(), String> {
    let cfg = Config::load();
    init_db(&cfg.db_path)?;

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let prompt = args[1..].join(" ");
        println!("\n🤖 Processing prompt: '{}'\n{}", prompt, "=".repeat(50));
        let (msg, dec) = process_agent_request(&prompt, None, &cfg);
        println!("{}", msg);
        println!("\n--- Gate Decision Payload ---");
        println!("{}", dec.to_json());
        return Ok(());
    }

    println!("==================================================");
    println!(" 🛡️  Agentic Commerce Spend-Gate CLI Simulator (Rust)  🛡️");
    println!("==================================================");
    println!("Type your shopping request (e.g., 'Order ₹500 groceries from Zepto for user_rahul')");
    println!("Usage: cargo run --bin agent -- \"<your command>\"\n");

    Ok(())
}
