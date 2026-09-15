//! Resolving `.editorconfig` properties into a [`FileStyle`].

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use ec4rs::property::{EndOfLine, FinalNewline, IndentStyle, TabWidth, TrimTrailingWs};
use reformat_core::{FileStyle, LineEnding};

static WARNED: AtomicBool = AtomicBool::new(false);

/// The style `.editorconfig` declares for `path`.
///
/// Indentation is resolved only when `indent` is set: `indent_style = space`
/// under `[*]` would otherwise rewrite the tabs a `Makefile` requires.
/// `insert_final_newline = false` is ignored rather than stripping newlines.
pub fn style_of(path: &Path, indent: bool) -> FileStyle {
    let mut props = match ec4rs::properties_of(path) {
        Ok(props) => props,
        Err(e) => {
            if !WARNED.swap(true, Ordering::Relaxed) {
                log::warn!(
                    "Could not read .editorconfig for '{}': {}",
                    path.display(),
                    e
                );
            }
            return FileStyle::default();
        }
    };
    props.use_fallbacks();

    let width = match props.get::<TabWidth>() {
        Ok(TabWidth::Value(w)) if w > 0 => Some(w),
        _ => None,
    };

    FileStyle {
        trim_trailing_whitespace: matches!(
            props.get::<TrimTrailingWs>(),
            Ok(TrimTrailingWs::Value(true))
        ),
        insert_final_newline: matches!(props.get::<FinalNewline>(), Ok(FinalNewline::Value(true))),
        end_of_line: match props.get::<EndOfLine>() {
            Ok(EndOfLine::Lf) => Some(LineEnding::Lf),
            Ok(EndOfLine::CrLf) => Some(LineEnding::Crlf),
            Ok(EndOfLine::Cr) => Some(LineEnding::Cr),
            Err(_) => None,
        },
        indent: match (indent, props.get::<IndentStyle>(), width) {
            (true, Ok(IndentStyle::Tabs), Some(w)) => Some((reformat_core::IndentStyle::Tabs, w)),
            (true, Ok(IndentStyle::Spaces), Some(w)) => {
                Some((reformat_core::IndentStyle::Spaces, w))
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_properties_map_to_style() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join(".editorconfig"),
            "root = true\n\n[*]\ntrim_trailing_whitespace = true\ninsert_final_newline = true\n\
             end_of_line = crlf\nindent_style = space\nindent_size = 2\n\n\
             [Makefile]\nindent_style = tab\n",
        )
        .unwrap();

        let rs = style_of(&tmp.path().join("a.rs"), true);
        assert!(rs.trim_trailing_whitespace);
        assert!(rs.insert_final_newline);
        assert_eq!(rs.end_of_line, Some(LineEnding::Crlf));
        assert_eq!(rs.indent, Some((reformat_core::IndentStyle::Spaces, 2)));

        assert_eq!(style_of(&tmp.path().join("a.rs"), false).indent, None);
        // Tabs with a width inherited from `indent_size = 2` in `[*]`.
        assert_eq!(
            style_of(&tmp.path().join("Makefile"), true).indent,
            Some((reformat_core::IndentStyle::Tabs, 2))
        );
    }

    #[test]
    fn test_no_editorconfig_means_empty_style() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".editorconfig"), "root = true\n").unwrap();
        assert!(style_of(&tmp.path().join("a.rs"), true).is_empty());
    }
}
