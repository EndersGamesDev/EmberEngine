//! Native entry point.
//!
//! ```text
//! league-app                     practice duel against bots
//! league-app squad               practice squad match (3v3) against bots
//! league-app online URL LOBBY [create] [mode1|mode3] [password] [handle]
//! ```
//!
//! Everything lives in the library so the wasm build (`--lib`, a cdylib)
//! and this binary run exactly the same game.

use league::online_game::Config;

fn main() -> Result<(), String> {
    ember_engine::init_diagnostics();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => {
            league::run_local(1);
            Ok(())
        }
        Some("squad") => {
            league::run_local(3);
            Ok(())
        }
        Some("online") => {
            let Some(url) = args.get(1) else {
                return Err(
                    "usage: league-app online URL LOBBY [create] [mode1|mode3] [password] [handle]"
                        .into(),
                );
            };
            let lobby = args.get(2).cloned().unwrap_or_else(|| "lane".into());
            let create = args.get(3).is_some_and(|a| a == "create");
            let mode = args
                .get(4)
                .map_or(3, |m| if m == "mode1" || m == "1" { 1 } else { 3 });
            let password = args.get(5).filter(|p| *p != "-").cloned();
            let handle = args
                .get(6)
                .cloned()
                .unwrap_or_else(|| std::env::var("USERNAME").unwrap_or_else(|_| "player".into()));
            let cfg = Config {
                ws: url.clone(),
                handle,
                lobby,
                password,
                create,
                mode,
            };
            tracing::info!(url, lobby = cfg.lobby, "connecting");
            league::run_online(&cfg)
        }
        Some(other) => Err(format!("unknown command: {other}")),
    }
}
