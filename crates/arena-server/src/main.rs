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
    let mut name: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => bind = args.next().expect("--bind needs an address"),
            "--name" => name = Some(args.next().expect("--name needs a host name")),
            "--help" | "-h" => {
                writeln!(
                    std::io::stdout().lock(),
                    "arena-server [--bind ADDR:PORT] [--name HOST]\n\
                     \n\
                     The host name is also read from EMBER_HOST_NAME; --name wins.\n\
                     Without either the server runs unnamed and says so."
                )?;
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
    // --name, else the environment, else unnamed. The environment path is
    // what the on-host units use: `host-name.sh` resolves the machine's
    // name once and exports it, so nothing has to thread a flag through a
    // service file.
    let host_name = name
        .or_else(|| std::env::var("EMBER_HOST_NAME").ok())
        .unwrap_or_default();
    let listener = TcpListener::bind(&bind)
        .map_err(|e| std::io::Error::new(e.kind(), format!("failed to bind {bind}: {e}")))?;
    arena_server::run(
        listener,
        arena_server::ServerConfig {
            host_name,
            ..Default::default()
        },
    )
}
