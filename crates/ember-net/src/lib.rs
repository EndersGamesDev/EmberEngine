//! Canonical game-neutral JSON/WebSocket bootstrap and lobby protocol.

/// Canonical JSON/WebSocket bootstrap and lobby protocol.
pub mod outer;

/// Windows `ERROR_IO_PENDING`, returned by timed overlapped socket reads.
const WINDOWS_IO_PENDING: i32 = 997;

/// True when a read error means that no data is ready yet rather than that the
/// connection has ended.
#[must_use]
pub fn is_transient_read(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::Interrupted
    ) || error.raw_os_error() == Some(WINDOWS_IO_PENDING)
}

/// Strip control characters and cap the number of Unicode scalar values.
///
/// Whitespace-only input becomes empty so protocol-specific wrappers can
/// substitute their own anonymous-player handle.
#[must_use]
pub fn sanitize(s: &str, max: usize) -> String {
    let cleaned: String = s.chars().filter(|c| !c.is_control()).take(max).collect();
    if cleaned.trim().is_empty() {
        String::new()
    } else {
        cleaned
    }
}

/// Sanitize a handle and substitute the caller's protocol-specific fallback.
#[must_use]
pub fn sanitize_handle(s: &str, max: usize, fallback: &str) -> String {
    let handle = sanitize(s, max);
    if handle.is_empty() {
        fallback.to_owned()
    } else {
        handle
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use super::{WINDOWS_IO_PENDING, is_transient_read, sanitize, sanitize_handle};

    #[test]
    fn windows_io_pending_is_a_transient_read() {
        let pending = Error::from_raw_os_error(WINDOWS_IO_PENDING);
        assert_eq!(pending.raw_os_error(), Some(WINDOWS_IO_PENDING));
        assert!(is_transient_read(&pending));
        assert!(is_transient_read(&Error::from(ErrorKind::Interrupted)));
        assert!(!is_transient_read(&Error::from(ErrorKind::ConnectionReset)));
    }

    #[test]
    fn sanitization_preserves_existing_protocol_semantics() {
        assert_eq!(sanitize("a\n\u{1b}b", 10), "ab");
        assert_eq!(sanitize("  a  ", 3), "  a");
        assert_eq!(sanitize("   ", 10), "");
        assert_eq!(sanitize("é日z", 2), "é日");
        assert_eq!(sanitize_handle("", 20, "driver"), "driver");
        assert_eq!(sanitize_handle("alice", 20, "driver"), "alice");
    }
}
