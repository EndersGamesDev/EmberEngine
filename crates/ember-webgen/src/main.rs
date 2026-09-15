//! Command-line entry point for release-artifact web generation.

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    if env::args_os().nth(1).as_deref() == Some(OsStr::new("--map-diagnostics")) {
        return print_mapped_diagnostics();
    }
    let out = output_argument()?;
    let source_sha = source_sha()?;
    if out.file_name().and_then(|name| name.to_str()) != Some(source_sha.as_str()) {
        return Err("output directory must end with the source SHA".into());
    }
    let game_ids = ember_webgen::game_ids(Path::new("web/games.json"))?;
    let bundle = ember_webgen::render(&source_sha, &game_ids)?;
    ember_webgen::write(&out, &bundle)?;
    Ok(())
}

fn print_mapped_diagnostics() -> Result<(), Box<dyn Error + Send + Sync>> {
    if env::args_os().len() != 2 {
        return Err("usage: ember-webgen --map-diagnostics".into());
    }
    let mut diagnostics = String::new();
    io::stdin().read_to_string(&mut diagnostics)?;
    let mapped = ember_webgen::map_diagnostics(&diagnostics)?;
    io::stdout().lock().write_all(mapped.as_bytes())?;
    Ok(())
}

fn output_argument() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let mut arguments = env::args_os().skip(1);
    if arguments.next().as_deref() != Some(OsStr::new("--out")) {
        return Err("usage: ember-webgen --out target/web-generated/SOURCE_SHA".into());
    }
    let out = arguments.next().ok_or("--out needs a path")?;
    if arguments.next().is_some() {
        return Err("unexpected argument after --out path".into());
    }
    Ok(out.into())
}

fn source_sha() -> Result<String, Box<dyn Error + Send + Sync>> {
    let value = match env::var("EMBER_SOURCE_SHA") {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => {
            let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
            if !output.status.success() {
                return Err("git rev-parse HEAD failed".into());
            }
            String::from_utf8(output.stdout)?.trim().into()
        }
        Err(error) => return Err(error.into()),
    };
    if value.is_empty() {
        return Err("source SHA is empty".into());
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err("source SHA contains an unsafe path character".into());
    }
    Ok(value)
}
