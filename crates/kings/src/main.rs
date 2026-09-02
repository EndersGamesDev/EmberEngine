//! Native entry point. Everything lives in the library so the wasm build
//! (`--lib`, a cdylib) and this binary run exactly the same game.
//!
//!     kings-app                                              # hotseat
//!     kings-app online URL create|join LOBBY [PASSWORD|-] [HANDLE]

use std::process::ExitCode;

fn main() -> ExitCode {
    ember_engine::init_diagnostics();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("online") {
        kings::run_local();
        return ExitCode::SUCCESS;
    }
    let (Some(ws), Some(action), Some(lobby)) = (args.get(1), args.get(2), args.get(3)) else {
        tracing::error!("usage: kings-app online URL create|join LOBBY [PASSWORD|-] [HANDLE]");
        return ExitCode::FAILURE;
    };
    let create = match action.as_str() {
        "create" => true,
        "join" => false,
        other => {
            tracing::error!("expected create or join, got {other}");
            return ExitCode::FAILURE;
        }
    };
    let cfg = kings::online_game::Config {
        ws: ws.clone(),
        handle: args
            .get(5)
            .cloned()
            .or_else(|| std::env::var("USERNAME").ok())
            .unwrap_or_else(|| "player".into()),
        lobby: lobby.clone(),
        password: args.get(4).filter(|p| !p.is_empty() && *p != "-").cloned(),
        create,
    };
    if let Err(e) = kings::run_online(cfg) {
        tracing::error!(error = %e, "online mode failed");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
