//! Markdown formatter integration tests.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`check_reports_an_unformatted_file_without_writing_it`] | fmt | Formatter check mode reports every path that would change, exits with the linter's findings status, and leaves the named file byte-for-byte alone. # Panics Panics only when the temporary fixture cannot be created, read, written, or executed, or when the command violates the asserted result contract. |

use std::fs;
use std::process::Command;

/// Formatter check mode reports every path that would change, exits with the
/// linter's findings status, and leaves the named file byte-for-byte alone.
///
/// # Panics
///
/// Panics only when the temporary fixture cannot be created, read, written, or
/// executed, or when the command violates the asserted result contract.
///
/// ´claim:fmt:check-mode-reports-without-writing´
/// ´test:integration:check-reports-an-unformatted-file-without-writing-it´
#[test]
fn check_reports_an_unformatted_file_without_writing_it() {
    let root = tempfile::tempdir().expect("temporary directory");
    let path = root.path().join("unformatted.md");
    let before = "A paragraph that is\nhard wrapped.\n";
    fs::write(&path, before).expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_linter"))
        .args(["fmt", "--check"])
        .arg(&path)
        .output()
        .expect("run formatter check");

    assert_eq!(output.status.code(), Some(3));
    assert_eq!(fs::read_to_string(&path).expect("read fixture"), before);

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object on stdout");
    assert_eq!(report["check"], true);
    assert_eq!(report["files_scanned"], 1);
    assert_eq!(report["files_changed"], 1);
    assert_eq!(
        report["changed"][0]["path"],
        path.to_string_lossy().as_ref()
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(path.to_string_lossy().as_ref()),
        "the per-file diagnostic lists the path"
    );
}
