//! Applying per-file style settings, such as those declared in `.editorconfig`.
//!
//! This module does not parse `.editorconfig`. The caller supplies a resolver
//! that maps a path to a [`FileStyle`], and [`StyleStep`] applies it using the
//! `clean`, `indent` and `endings` transformations.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use crate::step::{ContentStep, FileTarget};
use crate::{
    EndingsNormalizer, EndingsOptions, IndentNormalizer, IndentOptions, IndentStyle, LineEnding,
    WhitespaceCleaner, WhitespaceOptions,
};

/// Style settings for one file. `None` or `false` leaves that aspect alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileStyle {
    /// Indentation style and tab width.
    pub indent: Option<(IndentStyle, usize)>,
    /// Line terminator.
    pub end_of_line: Option<LineEnding>,
    /// Strip trailing whitespace from each line.
    pub trim_trailing_whitespace: bool,
    /// Ensure a non-empty file ends with a line terminator.
    pub insert_final_newline: bool,
}

impl FileStyle {
    /// True if applying this style can change nothing.
    pub fn is_empty(&self) -> bool {
        self.indent.is_none()
            && self.end_of_line.is_none()
            && !self.trim_trailing_whitespace
            && !self.insert_final_newline
    }
}

/// A content step that applies the [`FileStyle`] resolved for each file.
pub struct StyleStep<F: Fn(&Path) -> FileStyle> {
    resolve: F,
    recursive: bool,
    last: RefCell<Option<(PathBuf, FileStyle)>>,
}

impl<F: Fn(&Path) -> FileStyle> StyleStep<F> {
    /// Creates a step that asks `resolve` for each file's style.
    pub fn new(resolve: F, recursive: bool) -> Self {
        StyleStep {
            resolve,
            recursive,
            last: RefCell::new(None),
        }
    }

    /// Resolves the style for `path`, reusing the previous answer for the same
    /// path, since the runner asks in `accepts` and again in `transform`.
    fn style_for(&self, path: &Path) -> FileStyle {
        if let Some((cached, style)) = self.last.borrow().as_ref() {
            if cached == path {
                return style.clone();
            }
        }
        let style = (self.resolve)(path);
        *self.last.borrow_mut() = Some((path.to_path_buf(), style.clone()));
        style
    }
}

impl<F: Fn(&Path) -> FileStyle> ContentStep for StyleStep<F> {
    fn name(&self) -> &'static str {
        "editorconfig"
    }

    fn accepts(&self, file: &FileTarget) -> bool {
        crate::step::accepts_by_extension(file, &[] as &[&str], self.recursive)
            && !self.style_for(&file.path).is_empty()
    }

    fn transform(&self, text: &str, file: &FileTarget) -> Option<(String, usize)> {
        let style = self.style_for(&file.path);
        let any = Vec::new();

        let whitespace = WhitespaceCleaner::new(WhitespaceOptions {
            remove_trailing: style.trim_trailing_whitespace,
            insert_final_newline: style.insert_final_newline,
            trim_trailing_blank_lines: false,
            file_extensions: any.clone(),
            ..Default::default()
        });
        let indent = style.indent.map(|(style, width)| {
            IndentNormalizer::new(IndentOptions {
                style,
                width,
                file_extensions: any.clone(),
                ..Default::default()
            })
        });
        let endings = style.end_of_line.map(|style| {
            EndingsNormalizer::new(EndingsOptions {
                style,
                file_extensions: any.clone(),
                ..Default::default()
            })
        });

        // Line endings go last, so a newline added above takes the target style.
        let mut steps: Vec<&dyn ContentStep> = vec![&whitespace];
        steps.extend(indent.as_ref().map(|s| s as &dyn ContentStep));
        steps.extend(endings.as_ref().map(|s| s as &dyn ContentStep));

        let mut current = text.to_string();
        let mut units = 0;
        for step in steps {
            if let Some((next, n)) = step.transform(&current, file) {
                current = next;
                units += n;
            }
        }
        (current != text).then_some((current, units))
    }

    fn describe(&self, units: usize, dry_run: bool) -> String {
        if dry_run {
            format!("Would apply EditorConfig ({} change(s)) to", units)
        } else {
            format!("Applied EditorConfig ({} change(s)) to", units)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(style: FileStyle, text: &str) -> String {
        let step = StyleStep::new(move |_: &Path| style.clone(), true);
        let target = FileTarget::file(Path::new("x.txt"));
        step.transform(text, &target)
            .map(|(s, _)| s)
            .unwrap_or_else(|| text.to_string())
    }

    #[test]
    fn test_empty_style_accepts_nothing() {
        let step = StyleStep::new(|_: &Path| FileStyle::default(), true);
        assert!(!step.accepts(&FileTarget::file(Path::new("a.rs"))));
    }

    #[test]
    fn test_any_file_with_a_style_is_accepted() {
        let step = StyleStep::new(
            |_: &Path| FileStyle {
                trim_trailing_whitespace: true,
                ..Default::default()
            },
            true,
        );
        assert!(step.accepts(&FileTarget::file(Path::new("Makefile"))));
        assert!(step.accepts(&FileTarget::file(Path::new(".env"))));
    }

    #[test]
    fn test_trim_and_final_newline() {
        let style = FileStyle {
            trim_trailing_whitespace: true,
            insert_final_newline: true,
            ..Default::default()
        };
        assert_eq!(run(style, "a  \nb"), "a\nb\n");
    }

    #[test]
    fn test_final_newline_takes_target_line_ending() {
        let style = FileStyle {
            insert_final_newline: true,
            end_of_line: Some(LineEnding::Crlf),
            ..Default::default()
        };
        assert_eq!(run(style, "a\nb"), "a\r\nb\r\n");
    }

    #[test]
    fn test_indent_is_applied_only_when_set() {
        let tabs = FileStyle {
            indent: Some((IndentStyle::Tabs, 4)),
            ..Default::default()
        };
        assert_eq!(run(tabs, "    x\n"), "\tx\n");
        let trim_only = FileStyle {
            trim_trailing_whitespace: true,
            ..Default::default()
        };
        assert_eq!(run(trim_only, "    x  \n"), "    x\n");
    }
}
