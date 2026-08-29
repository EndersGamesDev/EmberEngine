use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    // RUST_LOG still works: EnvFilter reads it, defaulting to info.
    use std::io::IsTerminal;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        // No color codes when logging to a file (nohup deployment).
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    let defaults = ember_server::ServerConfig::default();
    let mut bind = format!("127.0.0.1:{}", ember_net::DEFAULT_PORT);
    let mut max_players = 32usize;
    let mut max_conns_per_ip = defaults.max_conns_per_ip;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => bind = args.next().expect("--bind needs an address"),
            "--max-players" => {
                max_players = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("--max-players needs a number")
            }
            // Escape hatch for the one predictable false positive: several
            // clients driven from a single dev box all reach the server
            // through one tunnel address, so they share an IP.
            "--max-conns-per-ip" => {
                max_conns_per_ip = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("--max-conns-per-ip needs a number")
            }
            "--help" | "-h" => {
                println!(
                    "ember-server [--bind ADDR:PORT] [--max-players N] [--max-conns-per-ip N]"
                );
                return Ok(());
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }

    let listener =
        TcpListener::bind(&bind).unwrap_or_else(|e| panic!("failed to bind {bind}: {e}"));
    ember_server::run(
        listener,
        ember_server::ServerConfig {
            max_players,
            max_conns_per_ip,
            ..defaults
        },
    )
}
