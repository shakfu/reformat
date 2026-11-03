//! Restructures a `TODO.md` into pma's format v2.
//!
//! The target format (see <https://github.com/shakfu/pma>, `docs/dev/design.md`):
//! a `# TODO` title, then `## Critical`, `## High`, `## Medium` and `## Low`,
//! each holding `- [ ] text` or `- [x] text` items, optionally under `###`
//! group headings. Indented lines under an item are its description.
//!
//! Items in other sections are moved into a priority section, and the
//! section's name becomes their `###` group. A `###` heading that names a
//! priority inside such a section sets the priority instead. Plain bullets
//! count as items only while the file has no priority section: once it has
//! one, pma treats bullets elsewhere as notes, and so does this step. Text
//! that is not an item is never dropped; text outside the priority sections
//! stays in its section, after them.

use std::path::Path;

use crate::step::{ContentStep, FileTarget};

/// Priority sections, in output order.
pub const PRIORITIES: [&str; 4] = ["Critical", "High", "Medium", "Low"];

/// A `TODO.md` with no items: what [`TodoFormatter::format`] makes of an
/// empty file.
pub const SKELETON: &str = "# TODO\n\n## Critical\n\n## High\n\n## Medium\n\n## Low\n";

/// Options for restructuring TODO files.
#[derive(Debug, Clone)]
pub struct TodoOptions {
    /// Section for items that come from a section without a priority.
    pub default_priority: String,
    /// File names to process, compared case-insensitively; empty matches
    /// every file.
    pub file_names: Vec<String>,
    /// Process directories recursively.
    pub recursive: bool,
    /// Dry run mode (don't modify files).
    pub dry_run: bool,
}

impl Default for TodoOptions {
    fn default() -> Self {
        TodoOptions {
            default_priority: "Medium".to_string(),
            file_names: vec!["TODO.md".to_string()],
            recursive: true,
            dry_run: false,
        }
    }
}

/// Restructures TODO files into pma's format.
pub struct TodoFormatter {
    options: TodoOptions,
    default: usize,
}

impl TodoFormatter {
    /// Creates a formatter. Fails if `default_priority` is not a priority.
    pub fn new(options: TodoOptions) -> crate::Result<Self> {
        let default = priority_index(&options.default_priority).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown priority '{}': expected one of {}",
                options.default_priority,
                PRIORITIES.join(", ")
            )
        })?;
        Ok(TodoFormatter { options, default })
    }

    /// Restructures a single file.
    pub fn format_file(&self, path: &Path) -> crate::Result<usize> {
        if !path.is_file() {
            return Ok(0);
        }
        crate::step::apply_one(self, &FileTarget::file(path), self.options.dry_run)
    }

    /// Processes a directory or file.
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        crate::step::process_path(self, path, self.options.recursive, self.options.dry_run)
    }

    /// Returns `text` in pma's format.
    pub fn format(&self, text: &str) -> String {
        let eol = crate::lines::split_lines(text)
            .map(|(_, t)| t)
            .find(|t| !t.is_empty())
            .unwrap_or("\n");
        let doc = Doc::parse(text, self.default);
        doc.render().join(eol) + eol
    }
}

impl ContentStep for TodoFormatter {
    fn name(&self) -> &'static str {
        "todo"
    }

    fn accepts(&self, file: &FileTarget) -> bool {
        let named = self.options.file_names.is_empty()
            || file
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| {
                    self.options
                        .file_names
                        .iter()
                        .any(|f| f.eq_ignore_ascii_case(n))
                });
        named && crate::step::within_depth(file, self.options.recursive)
    }

    fn transform(&self, text: &str, _file: &FileTarget) -> Option<(String, usize)> {
        let out = self.format(text);
        (out != text).then_some((out, 1))
    }

    fn describe(&self, _units: usize, dry_run: bool) -> String {
        if dry_run {
            "Would restructure".to_string()
        } else {
            "Restructured".to_string()
        }
    }
}

/// The priority a heading names: `High`, `high`, `High Priority`, `Priority: high`.
fn priority_index(name: &str) -> Option<usize> {
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .filter(|w| w != "priority")
        .collect();
    match words.as_slice() {
        [w] => PRIORITIES.iter().position(|p| p.eq_ignore_ascii_case(w)),
        _ => None,
    }
}

/// A section of finished work, whose plain bullets are finished items.
fn is_done_section(name: &str) -> bool {
    ["done", "completed", "finished"]
        .iter()
        .any(|d| name.trim().eq_ignore_ascii_case(d))
}

/// `(level, name)` of an ATX heading. `##Critical` counts, as pma reads it.
fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.bytes().take_while(|&b| b == b'#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &line[level..];
    if rest.is_empty()
        || rest.starts_with([' ', '\t'])
        || (level == 2 && priority_index(rest).is_some())
    {
        Some((level, rest.trim()))
    } else {
        None
    }
}

/// A list entry: `(checkbox mark, text)`. The mark is `None` for a plain
/// bullet, and `Some(done)` for `[ ]`, `[x]` or `[X]`.
fn list_entry(line: &str) -> Option<(Option<bool>, &str)> {
    let rest = if let Some(r) = line.strip_prefix(['-', '*', '+']) {
        r
    } else {
        let digits = line.trim_start_matches(|c: char| c.is_ascii_digit());
        if digits.len() == line.len() {
            return None;
        }
        digits.strip_prefix(['.', ')'])?
    };
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let rest = rest.trim_start();
    let mark = match rest.get(..3) {
        Some("[ ]") => Some(false),
        Some("[x]") | Some("[X]") => Some(true),
        _ => None,
    };
    match mark {
        Some(done) if rest.len() == 3 || rest[3..].starts_with([' ', '\t']) => {
            Some((Some(done), rest[3..].trim()))
        }
        _ => Some((None, rest.trim_end())),
    }
}

/// An open code fence: its character and length.
fn fence_open(line: &str) -> Option<(u8, usize)> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let mark = *t.as_bytes().first()?;
    if mark != b'`' && mark != b'~' {
        return None;
    }
    let n = t.bytes().take_while(|&b| b == mark).count();
    (n >= 3 && !(mark == b'`' && t[n..].contains('`'))).then_some((mark, n))
}

fn fence_closes(line: &str, (mark, len): (u8, usize)) -> bool {
    let t = line.trim();
    let n = t.bytes().take_while(|&b| b == mark).count();
    n >= len && n == t.len()
}

#[derive(Debug)]
enum Block {
    Item {
        done: bool,
        text: String,
        description: Vec<String>,
    },
    /// Lines kept verbatim: prose, a fenced block, a heading.
    Text(Vec<String>),
}

impl Block {
    fn render(&self, out: &mut Vec<String>) {
        match self {
            Block::Item {
                done,
                text,
                description,
            } => {
                let mark = if *done { 'x' } else { ' ' };
                out.push(format!("- [{mark}] {text}").trim_end().to_string());
                let end = description
                    .iter()
                    .rposition(|l| !l.trim().is_empty())
                    .map_or(0, |i| i + 1);
                out.extend(description[..end].iter().cloned());
            }
            Block::Text(lines) => out.extend(lines.iter().cloned()),
        }
    }
}

/// Blocks under one `###` group, `None` for items before any group.
type Groups = Vec<(Option<String>, Vec<Block>)>;

fn group_mut<'a>(groups: &'a mut Groups, name: &Option<String>) -> &'a mut Vec<Block> {
    let i = match groups.iter().position(|(g, _)| g == name) {
        Some(i) => i,
        None if name.is_none() => {
            groups.insert(0, (None, Vec::new()));
            0
        }
        None => {
            groups.push((name.clone(), Vec::new()));
            groups.len() - 1
        }
    };
    &mut groups[i].1
}

#[derive(Clone, Copy, PartialEq)]
enum Place {
    Preamble,
    Known(usize),
    /// Index into `Doc::others`.
    Other(usize),
}

#[derive(Default)]
struct Doc {
    preamble: Vec<Block>,
    priorities: [Groups; 4],
    /// Sections without a priority: heading, and the blocks left in them.
    others: Vec<(String, Vec<Block>)>,
}

/// Where the open item lives: a priority, a group, and its index.
type Slot = (usize, Option<String>, usize);

struct Parser {
    doc: Doc,
    default: usize,
    /// The file has a priority section, so plain bullets elsewhere are notes.
    adopted: bool,
    place: Place,
    /// `###` group in a priority section.
    group: Option<String>,
    /// `###` inside another section: its name, or the priority it names.
    sub: Option<String>,
    sub_priority: Option<usize>,
    open: Option<Slot>,
    /// Lines of text being collected, and blank lines since the last line.
    text: Vec<String>,
    blanks: usize,
    fence: Option<(u8, usize)>,
    /// The open fence belongs to the open item's description.
    fence_in_item: bool,
    titled: bool,
}

impl Doc {
    fn parse(text: &str, default: usize) -> Doc {
        let lines: Vec<&str> = crate::lines::split_lines(text)
            .map(|(b, _)| b.strip_prefix('\u{feff}').unwrap_or(b))
            .collect();
        let mut p = Parser {
            doc: Doc::default(),
            default,
            adopted: has_priority_section(&lines),
            place: Place::Preamble,
            group: None,
            sub: None,
            sub_priority: None,
            open: None,
            text: Vec::new(),
            blanks: 0,
            fence: None,
            fence_in_item: false,
            titled: false,
        };
        for line in lines {
            p.line(line);
        }
        p.flush_text();
        p.doc
    }

    fn render(&self) -> Vec<String> {
        let mut out = vec!["# TODO".to_string(), String::new()];
        let blocks = |out: &mut Vec<String>, blocks: &[Block]| {
            for b in blocks {
                b.render(out);
                out.push(String::new());
            }
        };
        blocks(&mut out, &self.preamble);
        for (name, groups) in PRIORITIES.iter().zip(&self.priorities) {
            out.push(format!("## {name}"));
            out.push(String::new());
            for (group, items) in groups {
                if let Some(g) = group {
                    out.push(format!("### {g}"));
                    out.push(String::new());
                }
                blocks(&mut out, items);
            }
        }
        for (name, kept) in &self.others {
            let kept = without_empty_headings(kept);
            if !kept.is_empty() {
                out.push(format!("## {name}"));
                out.push(String::new());
                blocks(&mut out, &kept);
            }
        }
        while out.last().is_some_and(|l| l.is_empty()) {
            out.pop();
        }
        out
    }
}

/// `kept` without headings that no text follows before the next heading:
/// the items under them have moved.
fn without_empty_headings(kept: &[Block]) -> Vec<Block> {
    let is_heading =
        |b: &Block| matches!(b, Block::Text(l) if l.len() == 1 && heading(&l[0]).is_some());
    kept.iter()
        .enumerate()
        .filter(|&(i, b)| !(is_heading(b) && kept.get(i + 1).is_none_or(is_heading)))
        .filter_map(|(_, b)| match b {
            Block::Text(l) => Some(Block::Text(l.clone())),
            Block::Item { .. } => None,
        })
        .collect()
}

fn has_priority_section(lines: &[&str]) -> bool {
    let mut fence = None;
    lines.iter().any(|line| {
        if let Some(f) = fence {
            if fence_closes(line, f) {
                fence = None;
            }
            return false;
        }
        if let Some(f) = fence_open(line) {
            fence = Some(f);
            return false;
        }
        matches!(heading(line), Some((1 | 2, name)) if priority_index(name).is_some())
    })
}

impl Parser {
    fn line(&mut self, raw: &str) {
        if let Some(f) = self.fence {
            if fence_closes(raw, f) {
                self.fence = None;
            }
            match self.fence_in_item {
                true => self.describe(raw),
                false => self.text.push(raw.to_string()),
            }
            return;
        }
        let line = raw.trim_end();
        if line.is_empty() {
            self.blanks += 1;
            return;
        }
        let indented = line.starts_with([' ', '\t']);
        if indented && self.open.is_some() {
            if let Some(f) = fence_open(raw) {
                self.fence = Some(f);
                self.fence_in_item = true;
            }
            self.describe(raw);
            return;
        }
        if let Some(f) = fence_open(line) {
            self.close();
            self.fence = Some(f);
            self.fence_in_item = false;
            self.text.push(raw.to_string());
            return;
        }
        if indented {
            self.add_text(raw);
            return;
        }
        if let Some((level, name)) = heading(line) {
            self.heading(level, name, line);
            return;
        }
        if let Some((mark, text)) = list_entry(line) {
            self.entry(mark, text, line);
            return;
        }
        // A line straight after an item continues its paragraph.
        if self.open.is_some() && self.blanks == 0 {
            self.describe(&format!("  {line}"));
            return;
        }
        self.add_text(raw);
    }

    /// Appends a description line to the open item, with any blank lines
    /// before it.
    fn describe(&mut self, raw: &str) {
        let blanks = std::mem::take(&mut self.blanks);
        if let Some(Block::Item { description, .. }) = self.open_block() {
            description.extend(std::iter::repeat_n(String::new(), blanks));
            description.push(raw.to_string());
        }
    }

    fn open_block(&mut self) -> Option<&mut Block> {
        let (p, g, i) = self.open.clone()?;
        group_mut(&mut self.doc.priorities[p], &g).get_mut(i)
    }

    fn add_text(&mut self, raw: &str) {
        self.open = None;
        if self.blanks > 0 {
            self.flush_text();
        }
        self.blanks = 0;
        self.text.push(raw.to_string());
    }

    /// Ends the open item and any text being collected.
    fn close(&mut self) {
        self.open = None;
        self.flush_text();
        self.blanks = 0;
    }

    fn flush_text(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let block = Block::Text(std::mem::take(&mut self.text));
        match self.place {
            Place::Preamble => self.doc.preamble.push(block),
            Place::Known(p) => group_mut(&mut self.doc.priorities[p], &self.group).push(block),
            Place::Other(i) => self.doc.others[i].1.push(block),
        }
    }

    fn heading(&mut self, level: usize, name: &str, line: &str) {
        self.close();
        if level == 1 && !self.titled {
            // The title is always `# TODO`; a second `#` heading is a section.
            self.titled = true;
            return;
        }
        if level <= 2 {
            self.group = None;
            self.sub = None;
            self.sub_priority = None;
            self.place = match priority_index(name) {
                Some(p) => Place::Known(p),
                None => match self.doc.others.iter().position(|(n, _)| n == name) {
                    Some(i) => Place::Other(i),
                    None => {
                        self.doc.others.push((name.to_string(), Vec::new()));
                        Place::Other(self.doc.others.len() - 1)
                    }
                },
            };
            return;
        }
        match self.place {
            // pma reads `####` and deeper as neither section nor group.
            Place::Known(p) if level == 3 => {
                self.group = Some(name.to_string()).filter(|g| !g.is_empty());
                // Registered now, so a group with no items keeps its heading.
                group_mut(&mut self.doc.priorities[p], &self.group);
            }
            Place::Known(_) => self.add_text(line),
            Place::Other(i) => {
                self.sub_priority = priority_index(name);
                self.sub = self.sub_priority.is_none().then(|| name.to_string());
                self.doc.others[i]
                    .1
                    .push(Block::Text(vec![line.to_string()]));
            }
            Place::Preamble => self.add_text(line),
        }
    }

    fn entry(&mut self, mark: Option<bool>, text: &str, line: &str) {
        self.close();
        let (priority, group, done) = match self.place {
            Place::Known(p) => (p, self.group.clone(), mark.unwrap_or(false)),
            _ if mark.is_none() && self.adopted => {
                self.add_text(line);
                return;
            }
            Place::Preamble => (self.default, None, mark.unwrap_or(false)),
            Place::Other(i) => {
                let section = &self.doc.others[i].0;
                let group = match &self.sub {
                    Some(sub) => format!("{section} / {sub}"),
                    None => section.clone(),
                };
                let done = mark.unwrap_or_else(|| is_done_section(section));
                (self.sub_priority.unwrap_or(self.default), Some(group), done)
            }
        };
        let blocks = group_mut(&mut self.doc.priorities[priority], &group);
        blocks.push(Block::Item {
            done,
            text: text.to_string(),
            description: Vec::new(),
        });
        self.open = Some((priority, group, blocks.len() - 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(text: &str) -> String {
        TodoFormatter::new(TodoOptions::default())
            .unwrap()
            .format(text)
    }

    const EMPTY: &str = SKELETON;

    #[test]
    fn empty_input_gives_the_skeleton() {
        assert_eq!(fmt(""), EMPTY);
    }

    #[test]
    fn canonical_file_is_unchanged() {
        let text = "# TODO\n\n## Critical\n\n### Category1\n\n- [ ] task1\n\n- [ ] task2\n\n\
                    ## High\n\n## Medium\n\n- [x] done #tag\n  description\n\n  more\n\n## Low\n";
        assert_eq!(fmt(text), text);
    }

    #[test]
    fn output_is_a_fixed_point() {
        let text = "# Project todo\n\nSome intro.\n\n## Parser\n- fix a\n* [X] fixed b\n\
                    ## Ideas\nWe might:\n\n1. try c\n   with notes\n## Done\n- shipped d\n";
        let once = fmt(text);
        assert_eq!(fmt(&once), once);
    }

    #[test]
    fn other_sections_become_groups_under_the_default_priority() {
        let text = "# Todo list\n\n## Parser\n\n- fix a\n* [X] fixed b\n\n## UI\n\n+ c\n";
        assert_eq!(
            fmt(text),
            "# TODO\n\n## Critical\n\n## High\n\n## Medium\n\n### Parser\n\n- [ ] fix a\n\n\
             - [x] fixed b\n\n### UI\n\n- [ ] c\n\n## Low\n"
        );
    }

    #[test]
    fn priority_subheadings_set_the_priority() {
        let text = "# TODO\n\n## Parser\n\n### High priority\n\n- [ ] a\n\n### Low\n\n- [ ] b\n";
        assert_eq!(
            fmt(text),
            "# TODO\n\n## Critical\n\n## High\n\n### Parser\n\n- [ ] a\n\n## Medium\n\n\
             ## Low\n\n### Parser\n\n- [ ] b\n"
        );
    }

    #[test]
    fn empty_groups_keep_their_headings() {
        let text = "# TODO\n\n## Critical\n\n## High\n\n### a\n\n### b\n\n- [ ] x\n\n### a\n\n\
                    ## Medium\n\n## Low\n";
        assert_eq!(
            fmt(text),
            "# TODO\n\n## Critical\n\n## High\n\n### a\n\n### b\n\n- [ ] x\n\n## Medium\n\n## Low\n"
        );
    }

    #[test]
    fn nested_subheadings_join_the_section_name() {
        let text = "## Parser\n\n### Lexer\n\n- a\n";
        assert!(fmt(text).contains("### Parser / Lexer\n\n- [ ] a\n"));
    }

    #[test]
    fn priority_headings_are_canonicalised_and_merged() {
        let text = "# TODO\n\n##high\n\n- a\n\n## HIGH PRIORITY\n\n- [ ] b\n";
        assert_eq!(
            fmt(text),
            "# TODO\n\n## Critical\n\n## High\n\n- [ ] a\n\n- [ ] b\n\n## Medium\n\n## Low\n"
        );
    }

    #[test]
    fn bullets_outside_priorities_are_notes_once_adopted() {
        let text = "# TODO\n\n## High\n\n- [ ] a\n\n## Notes\n\n- a note\n- [ ] stray task\n";
        assert_eq!(
            fmt(text),
            "# TODO\n\n## Critical\n\n## High\n\n- [ ] a\n\n## Medium\n\n### Notes\n\n\
             - [ ] stray task\n\n## Low\n\n## Notes\n\n- a note\n"
        );
    }

    #[test]
    fn done_sections_keep_their_items_finished() {
        let out = fmt("## Done\n\n- shipped\n- [ ] reopened\n");
        assert!(out.contains("### Done\n\n- [x] shipped\n\n- [ ] reopened\n"));
    }

    #[test]
    fn prose_is_kept_in_its_section() {
        let text = "Intro line.\n\n## Ideas\n\nWe might:\n\n- try c\n\n### Later\n\nnothing yet\n";
        assert_eq!(
            fmt(text),
            "# TODO\n\nIntro line.\n\n## Critical\n\n## High\n\n## Medium\n\n### Ideas\n\n\
             - [ ] try c\n\n## Low\n\n## Ideas\n\nWe might:\n\n### Later\n\nnothing yet\n"
        );
    }

    #[test]
    fn descriptions_and_lazy_continuations_follow_their_item() {
        let text = "## X\n\n- a\ncontinued\n  indented\n\n  after blank\n\nnot part\n";
        let out = fmt(text);
        assert!(out.contains("- [ ] a\n  continued\n  indented\n\n  after blank\n"));
        assert!(out.ends_with("## X\n\nnot part\n"));
    }

    #[test]
    fn fenced_code_is_not_structure() {
        let text = "## X\n\n```\n## Not a heading\n- not an item\n```\n\n- a\n  ```\n# x\n  ```\n";
        let out = fmt(text);
        assert!(out.contains("- [ ] a\n  ```\n# x\n  ```\n"));
        assert!(out.contains("## X\n\n```\n## Not a heading\n- not an item\n```\n"));
        assert!(!out.contains("- [ ] not an item"));
    }

    #[test]
    fn crlf_is_kept() {
        let out = fmt("# TODO\r\n\r\n## Low\r\n\r\n- a\r\n");
        assert_eq!(
            out,
            EMPTY
                .replace("## Low\n", "## Low\n\n- [ ] a\n")
                .replace('\n', "\r\n")
        );
    }

    #[test]
    fn a_byte_order_mark_is_removed() {
        assert_eq!(fmt("\u{feff}# TODO\n"), EMPTY);
    }

    #[test]
    fn default_priority_is_configurable_and_validated() {
        let low = TodoFormatter::new(TodoOptions {
            default_priority: "low".into(),
            ..Default::default()
        })
        .unwrap();
        assert!(low.format("- a\n").ends_with("## Low\n\n- [ ] a\n"));
        assert!(TodoFormatter::new(TodoOptions {
            default_priority: "urgent".into(),
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn accepts_todo_files_by_name() {
        let f = TodoFormatter::new(TodoOptions::default()).unwrap();
        assert!(f.accepts(&FileTarget::file(Path::new("a/todo.md"))));
        assert!(!f.accepts(&FileTarget::file(Path::new("a/README.md"))));
    }

    #[test]
    fn other_marks_stay_in_the_text() {
        assert!(fmt("- [~] half\n").contains("- [ ] [~] half\n"));
    }
}
