use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    use std::io::{IsTerminal, Write};
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
                writeln!(std::io::stdout().lock(), "pong-server [--bind ADDR:PORT]")?;
                return Ok(());
            }
            other => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown argument: {other}"),
                ));
            }
        }
    }
    let listener = TcpListener::bind(&bind)
        .map_err(|e| std::io::Error::new(e.kind(), format!("failed to bind {bind}: {e}")))?;
    pong_server::run(listener, pong_server::ServerConfig::default())
}
