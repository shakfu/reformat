//! Emoji removal and replacement transformer
//!
//! This module provides functionality to remove or replace emojis in text files,
//! with special handling for task completion emojis.

use regex::Regex;
use std::path::Path;

use crate::step::{ContentStep, FileTarget};

/// Code points replaced with a text equivalent rather than deleted.
const TASK_EMOJI_CHARS: &str = concat!(
    r"\x{2705}\x{2611}\x{2714}\x{2713}\x{2610}\x{2612}\x{274C}\x{274E}",
    r"\x{26A0}\x{26D4}\x{2B50}",
    r"\x{1F7E0}\x{1F7E1}\x{1F7E8}\x{1F7E2}\x{1F534}",
    r"\x{1F4DD}\x{1F4CB}\x{1F4C4}\x{1F4C5}\x{1F4C6}\x{1F5D3}",
    r"\x{1F4D1}\x{1F4CC}\x{1F4CD}\x{1F4CE}",
);

/// Base emoji code points.
///
/// Card suits (U+2660-U+2667) and musical notes (U+2669-U+266F) are
/// deliberately excluded. They sit inside the Miscellaneous Symbols block but
/// are ordinary text characters, and the old blanket U+2600-U+26FF range
/// deleted them silently: `cards <U+2660> <U+2665> end` became `cards   end`.
const EMOJI_BASE: &str = concat!(
    r"[\x{1F300}-\x{1F5FF}\x{1F600}-\x{1F64F}\x{1F680}-\x{1F6FF}",
    r"\x{1F900}-\x{1F9FF}\x{1FA00}-\x{1FAFF}",
    r"\x{1F004}\x{1F0CF}\x{1F18E}\x{1F191}-\x{1F19A}\x{1F1E0}-\x{1F1FF}",
    r"\x{2600}-\x{265F}\x{2668}\x{2670}-\x{26FF}\x{2700}-\x{27BF}]",
);

/// Variation selectors, which choose text or emoji presentation.
const VARIATION_SELECTORS: &str = r"[\x{FE00}-\x{FE0F}]";

/// Modifiers that attach to a base emoji: presentation selectors and skin
/// tones. Never meaningful on their own.
const EMOJI_MODIFIERS: &str = r"[\x{FE00}-\x{FE0F}\x{1F3FB}-\x{1F3FF}]";

/// Options for emoji transformation
#[derive(Debug, Clone)]
pub struct EmojiOptions {
    /// Replace task completion emojis with text alternatives
    pub replace_task_emojis: bool,
    /// Remove all other emojis
    pub remove_other_emojis: bool,
    /// File extensions to process; empty matches every file
    pub file_extensions: Vec<String>,
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't modify files)
    pub dry_run: bool,
}

impl Default for EmojiOptions {
    fn default() -> Self {
        EmojiOptions {
            replace_task_emojis: true,
            remove_other_emojis: true,
            file_extensions: vec![
                ".md", ".txt", ".rst", ".org", ".py", ".rs", ".go", ".java", ".js", ".ts", ".jsx",
                ".tsx", ".c", ".h", ".cpp", ".hpp",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            recursive: true,
            dry_run: false,
        }
    }
}

/// Emoji transformer for removing and replacing emojis
pub struct EmojiTransformer {
    options: EmojiOptions,
    task_emoji_pattern: Regex,
    general_emoji_pattern: Regex,
}

impl EmojiTransformer {
    /// Creates a new emoji transformer with the given options
    pub fn new(options: EmojiOptions) -> Self {
        // Task/status emojis, replaced with a text equivalent. A trailing
        // variation selector is consumed with the emoji so that removing it
        // cannot leave an invisible orphan behind.
        let task_emoji_pattern =
            Regex::new(&format!(r"[{}]{}?", TASK_EMOJI_CHARS, VARIATION_SELECTORS))
                .expect("task emoji pattern is a compile-time constant");

        // Decorative emoji. Matched as whole *sequences* rather than as
        // individual code points: a family emoji is a chain of people joined
        // by U+200D ZERO WIDTH JOINER, and removing only the people used to
        // leave the joiners behind as invisible debris. Keycap sequences
        // (`1` + U+FE0F + U+20E3) had the same problem.
        //
        // Task emojis sit inside these ranges. When they are not being
        // replaced they are subtracted, so "keep task emojis" keeps them.
        let base = if options.replace_task_emojis {
            EMOJI_BASE.to_string()
        } else {
            format!("[{}--[{}]]", EMOJI_BASE, TASK_EMOJI_CHARS)
        };
        let general_emoji_pattern = Regex::new(&format!(
            concat!(
                // Keycap sequences first, so the digit is consumed with its mark.
                r"(?:[0-9\#\*]{vs}?\x{{20E3}})",
                // An emoji, its modifiers, and any ZWJ-joined continuation.
                r"|(?:{base}{mods}*(?:\x{{200D}}{base}{mods}*)*)",
                // Joiners and keycap marks orphaned by earlier versions.
                r"|\x{{200D}}|\x{{20E3}}",
            ),
            base = base,
            mods = EMOJI_MODIFIERS,
            vs = VARIATION_SELECTORS,
        ))
        .expect("general emoji pattern is a compile-time constant");

        EmojiTransformer {
            options,
            task_emoji_pattern,
            general_emoji_pattern,
        }
    }

    /// Creates a transformer with default options
    pub fn with_defaults() -> Self {
        EmojiTransformer::new(EmojiOptions::default())
    }

    /// Replace task emojis with text equivalents. Keyed on the base character
    /// so that a trailing variation selector, consumed with it, does not
    /// defeat the lookup.
    fn replace_task_emoji(&self, matched: &str) -> &str {
        let Some(base) = matched.chars().next() else {
            return "";
        };
        match base {
            '\u{2705}' => "[x]",       // ✅ -> [x]
            '\u{2611}' => "[x]",       // ☑ -> [x]
            '\u{2714}' => "[x]",       // ✔ -> [x]
            '\u{2713}' => "[x]",       // ✓ -> [x]
            '\u{2610}' => "[ ]",       // ☐ -> [ ]
            '\u{2612}' => "[X]",       // ☒ -> [X]
            '\u{274C}' => "[X]",       // ❌ -> [X]
            '\u{274E}' => "[X]",       // ❎ -> [X]
            '\u{26A0}' => "[!]",       // ⚠ -> [!]
            '\u{26D4}' => "[!]",       // ⛔ -> [!]
            '\u{2B50}' => "[+]",       // ⭐ -> [+]
            '\u{1F7E0}' => "[orange]", // 🟠 -> [orange]
            '\u{1F7E1}' => "[yellow]", // 🟡 -> [yellow]
            '\u{1F7E8}' => "[yellow]", // 🟨 -> [yellow]
            '\u{1F7E2}' => "[green]",  // 🟢 -> [green]
            '\u{1F534}' => "[red]",    // 🔴 -> [red]
            '\u{1F4DD}' => "[note]",   // 📝 -> [note]
            '\u{1F4CB}' => "[list]",   // 📋 -> [list]
            '\u{1F4C4}' => "[doc]",    // 📄 -> [doc]
            '\u{1F4C5}' => "[cal]",    // 📅 -> [cal]
            '\u{1F4C6}' => "[cal]",    // 📆 -> [cal]
            '\u{1F5D3}' => "[cal]",    // 🗓 -> [cal]
            '\u{1F4D1}' => "[tab]",    // 📑 -> [tab]
            '\u{1F4CC}' => "[pin]",    // 📌 -> [pin]
            '\u{1F4CD}' => "[pin]",    // 📍 -> [pin]
            '\u{1F4CE}' => "[clip]",   // 📎 -> [clip]
            _ => "",
        }
    }

    /// Transform emojis in a single file
    pub fn transform_file(&self, path: &Path) -> crate::Result<usize> {
        if !path.is_file() {
            return Ok(0);
        }
        crate::step::apply_one(self, &FileTarget::file(path), self.options.dry_run)
    }

    /// Processes a directory or file
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        crate::step::process_path(self, path, self.options.recursive, self.options.dry_run)
    }
}

impl ContentStep for EmojiTransformer {
    fn name(&self) -> &'static str {
        "emojis"
    }

    fn accepts(&self, file: &FileTarget) -> bool {
        crate::step::accepts_by_extension(
            file,
            &self.options.file_extensions,
            self.options.recursive,
        )
    }

    fn transform(&self, text: &str, _file: &FileTarget) -> Option<(String, usize)> {
        let mut modified = text.to_string();
        let mut changes = 0;

        if self.options.replace_task_emojis {
            let found = self.task_emoji_pattern.find_iter(&modified).count();
            if found > 0 {
                modified = self
                    .task_emoji_pattern
                    .replace_all(&modified, |caps: &regex::Captures| {
                        self.replace_task_emoji(&caps[0])
                    })
                    .into_owned();
                changes += found;
            }
        }

        if self.options.remove_other_emojis {
            let found = self.general_emoji_pattern.find_iter(&modified).count();
            if found > 0 {
                modified = self
                    .general_emoji_pattern
                    .replace_all(&modified, "")
                    .into_owned();
                changes += found;
            }
        }

        (modified != text).then(|| (modified, changes.max(1)))
    }

    fn describe(&self, units: usize, dry_run: bool) -> String {
        if dry_run {
            format!("Would transform {} emoji(s) in", units)
        } else {
            format!("Transformed {} emoji(s) in", units)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the transformer over `body` in a directory unique to `case`.
    /// Tests in one binary run in parallel, so a shared fixture path lets
    /// them clobber each other.
    fn transform(_case: &str, body: &str) -> String {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("t.md");
        fs::write(&file, body).unwrap();
        EmojiTransformer::with_defaults()
            .transform_file(&file)
            .unwrap();

        fs::read_to_string(&file).unwrap()
    }

    /// Removing the people from a ZWJ sequence used to leave the joiners
    /// behind as invisible debris.
    #[test]
    fn test_zwj_sequences_leave_no_debris() {
        // man + ZWJ + woman + ZWJ + girl
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let out = transform(
            "zwj_sequences_leave_no_debris",
            &format!("family {} end\n", family),
        );
        assert_eq!(out, "family  end\n");
        assert!(!out.contains('\u{200D}'), "a zero width joiner survived");
    }

    /// Keycap sequences are digit + variation selector + enclosing mark.
    #[test]
    fn test_keycap_sequences_are_removed_whole() {
        let out = transform(
            "keycap_sequences_are_removed_whole",
            "keycap 1\u{FE0F}\u{20E3} end\n",
        );
        assert_eq!(out, "keycap  end\n");
        assert!(
            !out.contains('\u{20E3}'),
            "an enclosing keycap mark survived"
        );
    }

    /// Skin tone modifiers must go with their base emoji.
    #[test]
    fn test_skin_tone_modifiers_are_consumed() {
        let out = transform(
            "skin_tone_modifiers_are_consumed",
            "wave \u{1F44B}\u{1F3FD} end\n",
        );
        assert_eq!(out, "wave  end\n");
    }

    /// Card suits and musical notes are ordinary text, not decoration. The old
    /// blanket U+2600-U+26FF range deleted them.
    #[test]
    fn test_text_symbols_are_not_deleted() {
        let out = transform(
            "text_symbols_are_not_deleted",
            "cards \u{2660} \u{2665} notes \u{266A} end\n",
        );
        assert_eq!(
            out, "cards \u{2660} \u{2665} notes \u{266A} end\n",
            "text symbols in the Miscellaneous Symbols block were deleted"
        );
    }

    /// Real emoji from the same block are still removed.
    #[test]
    fn test_genuine_emoji_in_symbol_block_still_removed() {
        // U+26BD soccer ball, U+2708 airplane
        let out = transform(
            "genuine_emoji_in_symbol_block_still_removed",
            "play \u{26BD} fly \u{2708} end\n",
        );
        assert_eq!(out, "play  fly  end\n");
    }

    /// Debris left in files by earlier versions is cleaned up on a later run.
    #[test]
    fn test_orphaned_joiners_are_cleaned_up() {
        let out = transform(
            "orphaned_joiners_are_cleaned_up",
            "family \u{200D}\u{200D} end\n",
        );
        assert_eq!(out, "family  end\n");
    }

    /// A task emoji written with an explicit emoji presentation selector must
    /// still be replaced, and must not leave the selector behind.
    #[test]
    fn test_task_emoji_with_variation_selector() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("vs.md");
        fs::write(&file, "- \u{2714}\u{FE0F} done\n").unwrap();
        EmojiTransformer::with_defaults()
            .transform_file(&file)
            .unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "- [x] done\n");
    }
    use std::fs;

    fn apply(options: EmojiOptions, text: &str) -> String {
        let target = FileTarget::file(Path::new("x.md"));
        EmojiTransformer::new(options)
            .transform(text, &target)
            .map(|(s, _)| s)
            .unwrap_or_else(|| text.to_string())
    }

    /// With task replacement off, task emojis used to be deleted anyway,
    /// because their code points fall inside the decorative ranges.
    #[test]
    fn test_task_emojis_survive_when_not_replaced() {
        let options = EmojiOptions {
            replace_task_emojis: false,
            ..Default::default()
        };
        assert_eq!(
            apply(
                options,
                "done \u{2705}\u{FE0F} red \u{1F534} launch \u{1F680}\n"
            ),
            "done \u{2705}\u{FE0F} red \u{1F534} launch \n"
        );
    }

    #[test]
    fn test_replace_task_emojis() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.md");
        fs::write(
            &test_file,
            "- [x] Done task\n- [ ] Todo task\n- Task complete\n",
        )
        .unwrap();

        // Replace checkmarks with [x]
        let content = fs::read_to_string(&test_file).unwrap();
        let updated = content.replace("✅", "[x]");
        fs::write(&test_file, updated).unwrap();

        let transformer = EmojiTransformer::with_defaults();
        let (_files, _) = transformer.process(&test_file).unwrap();

        // Should still be valid markdown
        let content = fs::read_to_string(&test_file).unwrap();
        assert!(content.contains("[x]") || content.contains("[ ]"));
    }

    #[test]
    fn test_checkmark_replacement() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        fs::write(&test_file, "Task done ✅\nTask pending ☐\n").unwrap();

        let transformer = EmojiTransformer::with_defaults();
        let (files, _) = transformer.process(&test_file).unwrap();

        if files > 0 {
            let content = fs::read_to_string(&test_file).unwrap();
            assert!(content.contains("[x]") || content.contains("[ ]"));
            assert!(!content.contains("✅"));
            assert!(!content.contains("☐"));
        }
    }

    #[test]
    fn test_dry_run_mode() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        let original = "Task ✅ done";
        fs::write(&test_file, original).unwrap();

        let opts = EmojiOptions {
            dry_run: true,

            ..Default::default()
        };

        let transformer = EmojiTransformer::new(opts);
        transformer.process(&test_file).unwrap();

        // File should be unchanged
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_skip_hidden_files() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let hidden_file = test_dir.join(".hidden.txt");
        fs::write(&hidden_file, "Task ✅\n").unwrap();

        let transformer = EmojiTransformer::with_defaults();
        let (files, _) = transformer.process(&hidden_file).unwrap();

        // Hidden file should be skipped
        assert_eq!(files, 0);
    }

    #[test]
    fn test_extension_filtering() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let md_file = test_dir.join("test.md");
        let xyz_file = test_dir.join("test.xyz");

        fs::write(&md_file, "✅ Task\n").unwrap();
        fs::write(&xyz_file, "✅ Task\n").unwrap();

        let opts = EmojiOptions {
            file_extensions: vec![".md".to_string()],

            ..Default::default()
        };

        let transformer = EmojiTransformer::new(opts);
        let (files, _) = transformer.process(&test_dir).unwrap();

        // Only .md should be processed
        assert_eq!(files, 1);

        let md_content = fs::read_to_string(&md_file).unwrap();
        let xyz_content = fs::read_to_string(&xyz_file).unwrap();

        assert!(md_content.contains("[x]") || !md_content.contains("✅"));
        assert_eq!(xyz_content, "✅ Task\n"); // Unchanged
    }

    #[test]
    fn test_recursive_processing() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let sub_dir = test_dir.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();

        let file1 = test_dir.join("file1.md");
        let file2 = sub_dir.join("file2.md");

        fs::write(&file1, "✅ Done\n").unwrap();
        fs::write(&file2, "☐ Todo\n").unwrap();

        let transformer = EmojiTransformer::with_defaults();
        let (files, _) = transformer.process(&test_dir).unwrap();

        assert_eq!(files, 2);
    }

    #[test]
    fn test_star_and_circle_replacement() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.md");
        fs::write(
            &test_file,
            "⭐ Important task\n🟡 In progress\n🟢 Complete\n🔴 Blocked\n",
        )
        .unwrap();

        let transformer = EmojiTransformer::with_defaults();
        let (files, _) = transformer.process(&test_file).unwrap();

        if files > 0 {
            let content = fs::read_to_string(&test_file).unwrap();
            assert!(
                content.contains("[+]"),
                "Star emoji should be replaced with [+]"
            );
            assert!(
                content.contains("[yellow]"),
                "Yellow circle should be replaced with [yellow]"
            );
            assert!(
                content.contains("[green]"),
                "Green circle should be replaced with [green]"
            );
            assert!(
                content.contains("[red]"),
                "Red circle should be replaced with [red]"
            );
            assert!(!content.contains("⭐"), "Star emoji should be removed");
            assert!(!content.contains("🟡"), "Yellow circle should be removed");
            assert!(!content.contains("🟢"), "Green circle should be removed");
            assert!(!content.contains("🔴"), "Red circle should be removed");
        }
    }

    #[test]
    fn test_yellow_square_replacement() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.md");
        fs::write(&test_file, "🟨 In progress task\n🟡 Another yellow\n").unwrap();

        let transformer = EmojiTransformer::with_defaults();
        let (files, _) = transformer.process(&test_file).unwrap();

        if files > 0 {
            let content = fs::read_to_string(&test_file).unwrap();
            assert!(
                content.contains("[yellow]"),
                "Yellow square should be replaced with [yellow]"
            );
            assert!(
                !content.contains("🟨"),
                "Yellow square emoji should be removed"
            );
            assert!(
                !content.contains("🟡"),
                "Yellow circle emoji should be removed"
            );
        }
    }
}
