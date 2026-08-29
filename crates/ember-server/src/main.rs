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

    let mut bind = format!("127.0.0.1:{}", ember_net::DEFAULT_PORT);
    let mut max_players = 32usize;
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
            "--help" | "-h" => {
                println!("ember-server [--bind ADDR:PORT] [--max-players N]");
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
    ember_server::run(listener, ember_server::ServerConfig { max_players })
}
