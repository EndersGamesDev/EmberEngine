//! `fire-server [bind-addr] [--name HOST]` — bind defaults to 127.0.0.1:7781.
//!
//! TLS terminates at the tunnel in front of this, exactly as it does for
//! pong-server, so the listener here is plain TCP.
//!
//! The bind address stays POSITIONAL: the deploy scripts and the on-host
//! units already pass it that way, and a flag would have broken every one of
//! them for no gain.

use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    use std::io::Write;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut bind: Option<String> = None;
    let mut name: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--name" => {
                name = Some(args.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "--name needs a host name",
                    )
                })?);
            }
            "--help" | "-h" => {
                writeln!(
                    std::io::stdout().lock(),
                    "fire-server [BIND_ADDR] [--name HOST]\n\
                     \n\
                     The host name is also read from EMBER_HOST_NAME; --name wins.\n\
                     Without either the server runs unnamed and says so."
                )?;
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown argument: {other}"),
                ));
            }
            other if bind.is_none() => bind = Some(other.to_string()),
            other => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unexpected extra argument: {other}"),
                ));
            }
        }
    }

    // --name, else the environment, else unnamed. The environment path is
    // what the on-host units use: `host-name.sh` resolves the machine's name
    // once and exports it, so nothing has to thread a flag through a service
    // file.
    let host_name = name
        .or_else(|| std::env::var("EMBER_HOST_NAME").ok())
        .unwrap_or_default();
    let bind = bind.unwrap_or_else(|| "127.0.0.1:7781".to_string());
    let listener = TcpListener::bind(&bind)
        .map_err(|e| std::io::Error::new(e.kind(), format!("failed to bind {bind}: {e}")))?;
    fire_server::run(
        listener,
        fire_server::ServerConfig {
            host_name,
            ..Default::default()
        },
    )
}
