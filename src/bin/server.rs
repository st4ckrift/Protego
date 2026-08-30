use mandate_gate::config::Config;
use mandate_gate::server_service::start_server;

fn main() -> Result<(), String> {
    let cfg = Config::load();
    start_server(cfg)
}
