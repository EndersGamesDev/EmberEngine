//! `league-server [BIND_ADDR] [--name HOST]` — bind defaults to
//! 127.0.0.1:7783. TLS terminates at the tunnel in front of this, exactly
//! as it does for arena- and fire-server, so the listener here is plain
//! TCP. The bind address stays POSITIONAL like fire's; a flag for it would
//! have broken the deploy scripts for no gain.

use std::io::{self, Write};
use std::net::TcpListener;

fn main() -> io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let mut bind = None;
    let mut name = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--name" => {
                let Some(n) = args.next() else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--name needs a host name",
                    ));
                };
                name = Some(n);
            }
            "--help" | "-h" => {
                writeln!(
                    io::stdout().lock(),
                    "league-server [BIND_ADDR] [--name HOST]\n\n \
                     The host name is also read from EMBER_HOST_NAME; --name wins.\n \
                     Without either the server runs unnamed and says so."
                )?;
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {other}"),
                ));
            }
            other => {
                if bind.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unexpected extra argument: {other}"),
                    ));
                }
                bind = Some(other.to_string());
            }
        }
    }

    let host_name = name
        .or_else(|| std::env::var("EMBER_HOST_NAME").ok())
        .unwrap_or_default();
    let bind = bind.unwrap_or_else(|| "127.0.0.1:7783".to_string());
    let listener = TcpListener::bind(&bind)
        .map_err(|e| io::Error::new(e.kind(), format!("failed to bind {bind}: {e}")))?;
    // the first line a human sees when they attach to the console
    drop(io::stdout().flush());
    league_server::run(
        &listener,
        league_server::ServerConfig {
            host_name,
            ..Default::default()
        },
    )
}
