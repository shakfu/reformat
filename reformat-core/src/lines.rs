//! Line splitting that preserves each line's original terminator.
//!
//! `str::lines()` discards the terminator, so the usual
//! `content.lines()` / `join("\n")` round trip silently rewrites every CRLF
//! file as LF. Transformers that only mean to touch the *content* of a line
//! must not change how lines are separated, so they split with
//! [`split_lines`] and write the original terminator back.

/// An iterator over `(body, terminator)` pairs.
///
/// `terminator` is one of `"\r\n"`, `"\n"`, `"\r"`, or `""` for a final line
/// that ends without one. Concatenating every `body` and `terminator` in order
/// reproduces the input exactly.
pub struct SplitLines<'a> {
    rest: &'a str,
}

impl<'a> Iterator for SplitLines<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }

        match self.rest.find(['\n', '\r']) {
            // Final line, no terminator.
            None => {
                let body = self.rest;
                self.rest = "";
                Some((body, ""))
            }
            Some(idx) => {
                let bytes = self.rest.as_bytes();
                // CR and LF are ASCII, so byte indexing here is char-safe.
                let term_len = if bytes[idx] == b'\r' && bytes.get(idx + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                let body = &self.rest[..idx];
                let terminator = &self.rest[idx..idx + term_len];
                self.rest = &self.rest[idx + term_len..];
                Some((body, terminator))
            }
        }
    }
}

/// Splits `text` into lines, keeping each line's terminator separate from its
/// body. Recognises LF, CRLF and lone CR.
pub fn split_lines(text: &str) -> SplitLines<'_> {
    SplitLines { rest: text }
}

/// The first line terminator in `text`, or LF if it has none.
pub fn first_terminator(text: &str) -> &'static str {
    match split_lines(text).map(|(_, t)| t).find(|t| !t.is_empty()) {
        Some("\r\n") => "\r\n",
        Some("\r") => "\r",
        _ => "\n",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(text: &str) -> Vec<(&str, &str)> {
        split_lines(text).collect()
    }

    /// The defining property: splitting and rejoining is the identity.
    fn assert_roundtrip(text: &str) {
        let joined: String = split_lines(text)
            .flat_map(|(b, t)| [b, t])
            .collect::<Vec<_>>()
            .concat();
        assert_eq!(joined, text, "round trip changed the input");
    }

    #[test]
    fn test_lf() {
        assert_eq!(collect("a\nb\n"), [("a", "\n"), ("b", "\n")]);
        assert_roundtrip("a\nb\n");
    }

    #[test]
    fn test_crlf() {
        assert_eq!(collect("a\r\nb\r\n"), [("a", "\r\n"), ("b", "\r\n")]);
        assert_roundtrip("a\r\nb\r\n");
    }

    #[test]
    fn test_lone_cr() {
        assert_eq!(collect("a\rb\r"), [("a", "\r"), ("b", "\r")]);
        assert_roundtrip("a\rb\r");
    }

    #[test]
    fn test_mixed() {
        assert_eq!(
            collect("a\r\nb\nc\rd"),
            [("a", "\r\n"), ("b", "\n"), ("c", "\r"), ("d", "")]
        );
        assert_roundtrip("a\r\nb\nc\rd");
    }

    #[test]
    fn test_no_final_terminator() {
        assert_eq!(collect("a\nb"), [("a", "\n"), ("b", "")]);
        assert_roundtrip("a\nb");
    }

    #[test]
    fn test_empty_and_blank_lines() {
        assert_eq!(collect(""), []);
        assert_eq!(collect("\n"), [("", "\n")]);
        assert_eq!(collect("a\n\nb"), [("a", "\n"), ("", "\n"), ("b", "")]);
        assert_roundtrip("");
        assert_roundtrip("\n");
        assert_roundtrip("a\n\nb");
    }

    #[test]
    fn test_first_terminator() {
        assert_eq!(first_terminator("a\r\nb\n"), "\r\n");
        assert_eq!(first_terminator("a\rb"), "\r");
        assert_eq!(first_terminator("a\nb\r\n"), "\n");
        assert_eq!(first_terminator("no terminator"), "\n");
    }

    #[test]
    fn test_multibyte_content_roundtrips() {
        assert_roundtrip("caf\u{e9}\r\nna\u{ef}ve\n\u{4e2d}\u{6587}");
    }
}
