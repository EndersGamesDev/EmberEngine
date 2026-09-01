// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! The declared surface's reversible path display.
//!
//! A repository-relative path is an opaque byte sequence, and TOML carries text,
//! so the declaration writes a path in a reversible display: the bytes a reader
//! can read stand for themselves, every other byte is a percent escape, and
//! decoding is total on the display's own image. That display has exactly one
//! spelling per value — a byte with a literal form may not be written as an
//! escape and a hexadecimal escape is uppercase — and it is how a path *value* is
//! written and reported.
//!
//! # The display outlives every matcher
//!
//! Two languages met here while the pre-grammar surface stood: this display, and
//! the regular-expression matcher the inclusion and exclusion rows were written
//! in. They were never one language. The display says what a path *is*; a matcher
//! says which paths a row *selects*, and which matcher language the surface
//! speaks is a question the ratified grammar answers for itself
//! (´gram:isolation:declaration´). The matcher that stood beside this display has
//! retired with the rows that read it, and the display is unchanged by its going:
//! a path value is written this way under any pattern language at all.
//!
//! The display is also stated as a function over arbitrary bytes rather than over
//! paths alone, because a path is not the only thing a report has to render
//! without decoding it. A licence header's declared text is the other: it is
//! compared as bytes and must be quotable in a message whatever those bytes are,
//! and a lossy conversion at the message would report a text the file does not
//! carry.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`the_display_round_trips_every_path_byte`] | pattern | Every byte a path can carry survives a round trip through the display, including the bytes no filesystem convention expects — a newline, a byte beyond ASCII, and the percent that introduces an escape — so a declaration can name any file the walker can reach rather than only the ones whose names happen to be text. |
//! | [`the_display_admits_one_spelling_per_path`] | pattern | The display has exactly one spelling per path: a byte that stands for itself may not be written as an escape, and a hexadecimal escape is uppercase. Without that a declared row could be written two ways, and duplicate detection over declared bytes would miss the pair. |
//! | [`the_display_decodes_only_to_a_relative_path`] | pattern | A display decodes only to a path a walk from the repository root could have produced: never absolute, never carrying a NUL or an empty component, and never carrying a traversal component. A row naming a path outside the tree is refused where it is written rather than silently matching nothing. |
//!
//! The index is a generated projection and stands empty until the projection
//! writer fills it.

use std::fmt;

/// The bytes that stand for themselves in the reversible path display.
///
/// Everything else is a percent escape, so the set is the whole of what a reader
/// meets unescaped: the ASCII alphanumerics, the separator, and the three
/// punctuation bytes a path in this repository actually uses.
const fn is_literal_display_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.')
}

/// One uppercase hexadecimal digit's value, or `None` when it is not one.
const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Render one byte as two uppercase hexadecimal digits.
fn hex_digits(byte: u8) -> [u8; 2] {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";

    [
        DIGITS[usize::from(byte >> 4)],
        DIGITS[usize::from(byte & 0x0f)],
    ]
}

/// Why a declared path display does not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathDefect {
    /// A byte stands unescaped that the display escapes.
    UnescapedByte {
        /// The byte as it stood.
        byte: u8,
    },
    /// A percent escape is not followed by two uppercase hexadecimal digits.
    MalformedEscape,
    /// A byte with a literal form was written as an escape, so the display is not canonical.
    NoncanonicalEscape {
        /// The byte the escape encodes.
        byte: u8,
    },
    /// The decoded bytes carry a NUL.
    EmbeddedNul,
    /// The decoded path is absolute rather than repository-relative.
    AbsolutePath,
    /// The decoded path carries an empty component, or is empty entire.
    EmptyComponent,
    /// The decoded path carries a `.` or `..` component.
    TraversalComponent,
}

impl serde::Serialize for PathDefect {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl fmt::Display for PathDefect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnescapedByte { byte } => {
                write!(
                    formatter,
                    "the byte {byte:#04x} stands unescaped where the display escapes it"
                )
            }
            Self::MalformedEscape => {
                formatter.write_str("a percent escape is not two uppercase hexadecimal digits")
            }
            Self::NoncanonicalEscape { byte } => {
                write!(
                    formatter,
                    "the byte {byte:#04x} has a literal form and may not be escaped"
                )
            }
            Self::EmbeddedNul => formatter.write_str("the decoded path carries a NUL byte"),
            Self::AbsolutePath => formatter.write_str("the decoded path is absolute"),
            Self::EmptyComponent => {
                formatter.write_str("the decoded path carries an empty component")
            }
            Self::TraversalComponent => {
                formatter.write_str("the decoded path carries a `.` or `..` component")
            }
        }
    }
}

/// A repository-root-relative path, held as the bytes it actually is.
///
/// The type exists so that a path which has been validated once is not validated
/// again downstream, and so that no stage of the declared surface is tempted into
/// a lossy conversion: the display is computed on demand and the comparison is
/// always over bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BytePath(Vec<u8>);

impl BytePath {
    /// Take a walked path's bytes, validating them as a relative path.
    ///
    /// The walker produces these rather than a human, so the same constraints
    /// apply for the same reason: a path the declaration could not have written
    /// is a path no row could match, and discovering that at the matcher would
    /// report the wrong fault.
    ///
    /// # Errors
    ///
    /// Returns the defect when the bytes are not a repository-relative path.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Result<Self, PathDefect> {
        let bytes = bytes.into();

        validate_relative(&bytes)?;

        Ok(Self(bytes))
    }

    /// Decode a declared display into the path bytes it stands for.
    ///
    /// # Errors
    ///
    /// Returns the defect when the display is not canonical, or when the bytes
    /// it decodes to are not a repository-relative path.
    pub fn decode(text: &str) -> Result<Self, PathDefect> {
        let source = text.as_bytes();
        let mut bytes = Vec::with_capacity(source.len());
        let mut position = 0;

        while position < source.len() {
            let byte = source[position];

            if byte == b'%' {
                let high = source.get(position + 1).copied().and_then(hex_value);
                let low = source.get(position + 2).copied().and_then(hex_value);

                let (Some(high), Some(low)) = (high, low) else {
                    return Err(PathDefect::MalformedEscape);
                };

                let decoded = (high << 4) | low;

                if is_literal_display_byte(decoded) {
                    return Err(PathDefect::NoncanonicalEscape { byte: decoded });
                }

                bytes.push(decoded);
                position += 3;
            } else if is_literal_display_byte(byte) {
                bytes.push(byte);
                position += 1;
            } else {
                return Err(PathDefect::UnescapedByte { byte });
            }
        }

        validate_relative(&bytes)?;

        Ok(Self(bytes))
    }

    /// Render the path in the reversible display the declaration writes.
    #[must_use]
    pub fn display(&self) -> String {
        reversible(&self.0)
    }

    /// The path's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for BytePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display())
    }
}

impl serde::Serialize for BytePath {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.display())
    }
}

/// Hold decoded bytes to the shape of a repository-relative path.
fn validate_relative(bytes: &[u8]) -> Result<(), PathDefect> {
    if bytes.contains(&0) {
        return Err(PathDefect::EmbeddedNul);
    }

    if bytes.first() == Some(&b'/') {
        return Err(PathDefect::AbsolutePath);
    }

    if bytes.is_empty() {
        return Err(PathDefect::EmptyComponent);
    }

    for component in bytes.split(|&byte| byte == b'/') {
        if component.is_empty() {
            return Err(PathDefect::EmptyComponent);
        }

        if component == b"." || component == b".." {
            return Err(PathDefect::TraversalComponent);
        }
    }

    Ok(())
}

/// Render arbitrary bytes in the reversible display.
///
/// The display is a path's, and it is stated here as a function over bytes
/// because a path is not the only thing a report has to render without decoding
/// it. A licence header's declared text is the other: it is compared as bytes
/// and must be quotable in a message whatever those bytes are, and a lossy
/// conversion at the message would report a text the file does not carry.
#[must_use]
pub fn reversible(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());

    for &byte in bytes {
        if is_literal_display_byte(byte) {
            text.push(byte as char);
        } else {
            let digits = hex_digits(byte);

            text.push('%');
            text.push(digits[0] as char);
            text.push(digits[1] as char);
        }
    }

    text
}

#[cfg(test)]
mod tests {
    use super::{BytePath, PathDefect};

    /// Decode a display that is expected to be well-formed.
    fn decoded(text: &str) -> BytePath {
        BytePath::decode(text).expect("the display is well-formed")
    }

    /// Every byte a path can carry survives a round trip through the display,
    /// including the bytes no filesystem convention expects — a newline, a byte
    /// beyond ASCII, and the percent that introduces an escape — so a
    /// declaration can name any file the walker can reach rather than only the
    /// ones whose names happen to be text.
    ///
    /// ´claim:pattern:the-display-round-trips-every-path-byte´
    /// ´test:unit:the-display-round-trips-every-path-byte´
    #[test]
    fn the_display_round_trips_every_path_byte() {
        let awkward: Vec<u8> = vec![b'a', b'/', b'b', b'\n', 0x80, b'%', b' ', 0xff, b'c'];
        let path = BytePath::from_bytes(awkward.clone()).expect("a relative path");

        assert_eq!(path.display(), "a/b%0A%80%25%20%FFc");
        assert_eq!(decoded(&path.display()).as_bytes(), awkward.as_slice());
    }

    /// The display has exactly one spelling per path: a byte that stands for
    /// itself may not be written as an escape, and a hexadecimal escape is
    /// uppercase. Without that a declared row could be written two ways, and
    /// duplicate detection over declared bytes would miss the pair.
    ///
    /// ´claim:pattern:the-display-admits-one-spelling-per-path´
    /// ´test:unit:the-display-admits-one-spelling-per-path´
    #[test]
    fn the_display_admits_one_spelling_per_path() {
        assert_eq!(
            BytePath::decode("%41GENTS.md"),
            Err(PathDefect::NoncanonicalEscape { byte: b'A' })
        );
        assert_eq!(BytePath::decode("a%2fb"), Err(PathDefect::MalformedEscape));
        assert_eq!(
            BytePath::decode("a b"),
            Err(PathDefect::UnescapedByte { byte: b' ' })
        );
        assert_eq!(BytePath::decode("a%2"), Err(PathDefect::MalformedEscape));
    }

    /// A display decodes only to a path a walk from the repository root could
    /// have produced: never absolute, never carrying a NUL or an empty
    /// component, and never carrying a traversal component. A row naming a path
    /// outside the tree is refused where it is written rather than silently
    /// matching nothing.
    ///
    /// ´claim:pattern:the-display-decodes-only-to-a-relative-path´
    /// ´test:unit:the-display-decodes-only-to-a-relative-path´
    #[test]
    fn the_display_decodes_only_to_a_relative_path() {
        assert_eq!(
            BytePath::decode("/etc/passwd"),
            Err(PathDefect::AbsolutePath)
        );
        assert_eq!(BytePath::decode("a//b"), Err(PathDefect::EmptyComponent));
        assert_eq!(BytePath::decode("a/"), Err(PathDefect::EmptyComponent));
        assert_eq!(BytePath::decode(""), Err(PathDefect::EmptyComponent));
        assert_eq!(
            BytePath::decode("a/../b"),
            Err(PathDefect::TraversalComponent)
        );
        assert_eq!(BytePath::decode("./a"), Err(PathDefect::TraversalComponent));
        assert_eq!(BytePath::decode("a%00b"), Err(PathDefect::EmbeddedNul));
    }
}
