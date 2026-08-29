use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    use std::io::IsTerminal;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    let mut bind = "127.0.0.1:7778".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => bind = args.next().expect("--bind needs an address"),
            "--help" | "-h" => {
                println!("pong-server [--bind ADDR:PORT]");
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
    pong_server::run(listener, pong_server::ServerConfig::default())
}
