mod config;
mod dirty;
mod editorconfig;
mod select;

use clap::{Args, CommandFactory, Parser, Subcommand};
use log::{debug, info, warn};
use logging_timer::time;
use reformat_core::config::{
    CleanConfig, ConvertConfig, EditorConfigConfig, EmojiConfig, EndingsConfig, GroupConfig,
    HeaderConfig, IndentConfig, RenameConfig, ReplaceConfig, ReplacePatternEntry,
};
use reformat_core::{
    ContentStep, EmojiTransformer, EndingsNormalizer, FileGrouper, FileRenamer, FileTarget,
    FixRecord, HeaderManager, IndentNormalizer, Preset, ReferenceFixer, ReferenceScanner,
    ScanOptions, StyleStep, WhitespaceCleaner,
};
use simplelog::*;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "reformat",
    version = env!("CARGO_PKG_VERSION"),
    about = "Code transformation tool for case conversion and cleaning",
    long_about = "A modular code transformation framework.\n\n\
                  Usage:\n\
                  - reformat <path>...: Replace task emojis and strip trailing whitespace\n\
                  - reformat -p <preset> <path>...: Run a named preset from reformat.json\n\
                  - reformat --job <file|-> <path>...: Run an ad-hoc job from a file or stdin\n\n\
                  Commands:\n\
                  - convert: Convert between case formats\n\
                  - clean: Remove trailing whitespace\n\
                  - emojis: Remove or replace emojis with text alternatives\n\
                  - rename_files: Rename files with various transformations\n\
                  - group: Group files by common prefix into subdirectories\n\
                  - endings: Normalize line endings (LF/CRLF/CR)\n\
                  - indent: Normalize indentation (tabs/spaces)\n\
                  - replace: Regex find-and-replace across files\n\
                  - header: Insert or update file headers\n\
                  - editorconfig: Apply .editorconfig settings\n\
                  - apply_fixes: Apply reference fixes recorded by group\n\
                  - presets: List the presets in reformat.json\n\
                  - completions, man: Print shell completions or the man page\n\n\
                  Exit status: 0 on success, 1 when --check finds files that would change, \
                  2 on error."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Files or directories to process (when no subcommand is specified)
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,

    /// Run a named preset from reformat.json
    #[arg(short = 'p', long = "preset", conflicts_with = "job")]
    preset: Option<String>,

    /// Run an ad-hoc job from a JSON file (or "-" for stdin)
    #[arg(short = 'j', long = "job", conflicts_with = "preset")]
    job: Option<String>,

    /// Process files recursively (when no subcommand is specified)
    #[arg(short = 'r', long)]
    recursive: bool,

    /// Config file for -p and `presets` [default: reformat.json in the current
    /// directory, a target path, or a parent of either]
    #[arg(long = "config", value_name = "FILE", global = true)]
    config: Option<PathBuf>,

    /// Let a preset or job with rename, group, convert or replace steps
    /// modify files that have uncommitted changes
    #[arg(long = "allow-dirty")]
    allow_dirty: bool,

    #[command(flatten)]
    run: RunFlags,

    #[command(flatten)]
    select: SelectArgs,

    /// Enable verbose output (can be used multiple times: -v, -vv, -vvv)
    #[arg(short = 'v', long = "verbose", global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Suppress all output except errors
    #[arg(short = 'q', long = "quiet", global = true)]
    quiet: bool,

    /// Write logs to file
    #[arg(long = "log-file", global = true)]
    log_file: Option<PathBuf>,
}

/// Where input comes from, and whether and how changes are written.
#[derive(Args, Debug, Clone, Default)]
struct RunFlags {
    /// Read content from stdin and write the result to stdout. NAME decides
    /// which steps apply, by extension, and which .editorconfig section is used
    #[arg(long = "stdin-filename", value_name = "NAME", conflicts_with = "paths")]
    stdin_filename: Option<PathBuf>,

    /// Dry run: report what would change, modify nothing
    #[arg(short = 'd', long = "dry-run")]
    dry_run: bool,

    /// Modify nothing; exit with status 1 if any file would change
    #[arg(long)]
    check: bool,

    /// Print a unified diff of content changes; modify nothing
    #[arg(long)]
    diff: bool,
}

impl RunFlags {
    fn writes(&self) -> bool {
        !(self.dry_run || self.check || self.diff)
    }

    /// True when stdout carries data (content or a diff), so log lines must
    /// go to stderr.
    fn stdout_is_data(&self) -> bool {
        self.stdin_filename.is_some() || self.diff
    }

    fn prefix(&self) -> &'static str {
        if self.writes() {
            ""
        } else {
            "[DRY-RUN] "
        }
    }
}

/// Which files are offered to a command.
#[derive(Args, Debug, Clone, Default)]
struct SelectArgs {
    /// Only process files matching GLOB (repeatable). Without -e, this
    /// replaces the default extension list
    #[arg(long, value_name = "GLOB")]
    include: Vec<String>,

    /// Skip files and directories matching GLOB (repeatable)
    #[arg(long, value_name = "GLOB")]
    exclude: Vec<String>,

    /// Include hidden files and directories (.git is always skipped)
    #[arg(long)]
    hidden: bool,

    /// Ignore .gitignore and .ignore files, and walk build directories
    /// such as node_modules and target
    #[arg(long = "no-ignore")]
    no_ignore: bool,
}

impl SelectArgs {
    fn selection(&self) -> select::Selection {
        select::Selection {
            include: self.include.clone(),
            exclude: self.exclude.clone(),
            hidden: self.hidden,
            no_ignore: self.no_ignore,
        }
    }
}

/// Arguments shared by the commands that rewrite file contents.
#[derive(Args, Debug)]
struct Common {
    /// Files or directories to process
    #[arg(value_name = "PATH", required_unless_present = "stdin_filename")]
    paths: Vec<PathBuf>,

    /// File extensions to process (repeatable; the leading dot is optional)
    #[arg(short = 'e', long = "extensions")]
    extensions: Option<Vec<String>>,

    #[command(flatten)]
    run: RunFlags,

    #[command(flatten)]
    select: SelectArgs,
}

/// Recursion for commands that recurse by default.
#[derive(Args, Debug)]
struct Recursion {
    /// Process directories recursively (the default)
    #[arg(short = 'r', long, overrides_with = "no_recursive")]
    recursive: bool,

    /// Process only the top level of each directory
    #[arg(long = "no-recursive", overrides_with = "recursive")]
    no_recursive: bool,
}

impl Recursion {
    fn enabled(&self) -> bool {
        !self.no_recursive
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Convert between case formats
    #[command(group(clap::ArgGroup::new("from").required(true).multiple(false)))]
    #[command(group(clap::ArgGroup::new("to").required(true).multiple(false)))]
    Convert {
        /// Convert FROM camelCase
        #[arg(long = "from-camel", group = "from")]
        from_camel: bool,

        /// Convert FROM PascalCase
        #[arg(long = "from-pascal", group = "from")]
        from_pascal: bool,

        /// Convert FROM snake_case
        #[arg(long = "from-snake", group = "from")]
        from_snake: bool,

        /// Convert FROM SCREAMING_SNAKE_CASE
        #[arg(long = "from-screaming-snake", group = "from")]
        from_screaming_snake: bool,

        /// Convert FROM kebab-case
        #[arg(long = "from-kebab", group = "from")]
        from_kebab: bool,

        /// Convert FROM SCREAMING-KEBAB-CASE
        #[arg(long = "from-screaming-kebab", group = "from")]
        from_screaming_kebab: bool,

        /// Convert TO camelCase
        #[arg(long = "to-camel", group = "to")]
        to_camel: bool,

        /// Convert TO PascalCase
        #[arg(long = "to-pascal", group = "to")]
        to_pascal: bool,

        /// Convert TO snake_case
        #[arg(long = "to-snake", group = "to")]
        to_snake: bool,

        /// Convert TO SCREAMING_SNAKE_CASE
        #[arg(long = "to-screaming-snake", group = "to")]
        to_screaming_snake: bool,

        /// Convert TO kebab-case
        #[arg(long = "to-kebab", group = "to")]
        to_kebab: bool,

        /// Convert TO SCREAMING-KEBAB-CASE
        #[arg(long = "to-screaming-kebab", group = "to")]
        to_screaming_kebab: bool,

        #[command(flatten)]
        common: Common,

        /// Convert files recursively
        #[arg(short = 'r', long)]
        recursive: bool,

        /// Prefix to add to all converted words
        #[arg(long, default_value = "")]
        prefix: String,

        /// Suffix to add to all converted words
        #[arg(long, default_value = "")]
        suffix: String,

        /// Strip prefix before conversion (e.g., 'm_' from 'm_userName')
        #[arg(long = "strip-prefix")]
        strip_prefix: Option<String>,

        /// Strip suffix before conversion
        #[arg(long = "strip-suffix")]
        strip_suffix: Option<String>,

        /// Replace prefix (from) before conversion (e.g., 'I' in 'IUserService')
        #[arg(long = "replace-prefix-from", requires = "replace_prefix_to")]
        replace_prefix_from: Option<String>,

        /// Replace prefix (to) before conversion (e.g., 'Abstract')
        #[arg(long = "replace-prefix-to", requires = "replace_prefix_from")]
        replace_prefix_to: Option<String>,

        /// Replace suffix (from) before conversion
        #[arg(long = "replace-suffix-from", requires = "replace_suffix_to")]
        replace_suffix_from: Option<String>,

        /// Replace suffix (to) before conversion
        #[arg(long = "replace-suffix-to", requires = "replace_suffix_from")]
        replace_suffix_to: Option<String>,

        /// Glob pattern to filter files
        #[arg(long)]
        glob: Option<String>,

        /// Regex pattern to filter which words get converted
        #[arg(long = "word-filter")]
        word_filter: Option<String>,

        /// Modify files even if they have uncommitted changes in git
        #[arg(long = "allow-dirty")]
        allow_dirty: bool,
    },

    /// Remove trailing whitespace from files
    Clean {
        #[command(flatten)]
        common: Common,

        #[command(flatten)]
        recursion: Recursion,

        /// Append a line terminator to files that lack a final newline
        #[arg(long = "final-newline")]
        final_newline: bool,

        /// Remove blank lines at the end of each file
        #[arg(long = "trim-blank-lines")]
        trim_blank_lines: bool,
    },

    /// Remove or replace emojis with text alternatives
    Emojis {
        #[command(flatten)]
        common: Common,

        #[command(flatten)]
        recursion: Recursion,

        /// Replace task emojis with text, e.g. a check mark with [x] (the default)
        #[arg(long = "replace-task", overrides_with = "no_replace_task")]
        replace_task: bool,

        /// Leave task emojis unchanged
        #[arg(long = "no-replace-task", overrides_with = "replace_task")]
        no_replace_task: bool,

        /// Remove all other emojis (the default)
        #[arg(long = "remove-other", overrides_with = "no_remove_other")]
        remove_other: bool,

        /// Leave other emojis unchanged
        #[arg(long = "no-remove-other", overrides_with = "remove_other")]
        no_remove_other: bool,
    },

    /// Rename files with various transformations
    #[command(name = "rename_files")]
    RenameFiles {
        #[command(flatten)]
        common: Common,

        #[command(flatten)]
        recursion: Recursion,

        /// Include symbolic links in processing
        #[arg(long = "include-symlinks")]
        include_symlinks: bool,

        /// Convert to lowercase
        #[arg(long = "to-lowercase")]
        to_lowercase: bool,

        /// Convert to UPPERCASE
        #[arg(long = "to-uppercase")]
        to_uppercase: bool,

        /// Capitalize (first letter uppercase, rest lowercase)
        #[arg(long = "to-capitalize")]
        to_capitalize: bool,

        /// Replace separators (spaces, hyphens, underscores) with underscores
        #[arg(long = "underscored")]
        underscored: bool,

        /// Replace separators (spaces, hyphens, underscores) with hyphens
        #[arg(long = "hyphenated")]
        hyphenated: bool,

        /// Add prefix to filename
        #[arg(long = "add-prefix")]
        add_prefix: Option<String>,

        /// Remove prefix from filename
        #[arg(long = "rm-prefix")]
        rm_prefix: Option<String>,

        /// Add suffix to filename (before extension)
        #[arg(long = "add-suffix")]
        add_suffix: Option<String>,

        /// Remove suffix from filename (before extension)
        #[arg(long = "rm-suffix")]
        rm_suffix: Option<String>,

        /// Replace prefix in filename (two arguments: <old> <new>)
        #[arg(long = "replace-prefix", num_args = 2, value_names = ["OLD", "NEW"])]
        replace_prefix: Option<Vec<String>>,

        /// Replace suffix in filename (two arguments: <old> <new>)
        #[arg(long = "replace-suffix", num_args = 2, value_names = ["OLD", "NEW"])]
        replace_suffix: Option<Vec<String>>,

        /// Add timestamp prefix in YYYYMMDD format (e.g., 20250915_)
        #[arg(long = "timestamp-long")]
        timestamp_long: bool,

        /// Add timestamp prefix in YYMMDD format (e.g., 250915_)
        #[arg(long = "timestamp-short")]
        timestamp_short: bool,

        /// Modify files even if they have uncommitted changes in git
        #[arg(long = "allow-dirty")]
        allow_dirty: bool,
    },

    /// Group files by common prefix into subdirectories
    #[command(name = "group")]
    Group {
        /// The directory to process
        path: PathBuf,

        /// Process subdirectories recursively
        #[arg(short = 'r', long)]
        recursive: bool,

        /// Dry run (don't move files or create directories)
        #[arg(short = 'd', long = "dry-run")]
        dry_run: bool,

        /// Separator character that divides prefix from rest of filename
        #[arg(short = 's', long = "separator", default_value_t = '_')]
        separator: char,

        /// Minimum number of files with same prefix to create a group
        #[arg(short = 'm', long = "min-count", default_value_t = 2)]
        min_count: usize,

        /// Remove the prefix from filenames after moving to subdirectory
        #[arg(long = "strip-prefix")]
        strip_prefix: bool,

        /// Group by suffix: split at LAST separator, use suffix as filename
        /// e.g., "activity_relationships_list.tmpl" -> "activity_relationships/list.tmpl"
        /// Implies --strip-prefix
        #[arg(long = "from-suffix")]
        from_suffix: bool,

        /// Preview groups without making changes (shows what would be grouped)
        #[arg(long = "preview")]
        preview: bool,

        /// Skip interactive prompts for reference scanning
        #[arg(long = "no-interactive")]
        no_interactive: bool,

        /// Directory to scan recursively for broken references caused by the grouping
        #[arg(long = "scope")]
        scope: Option<PathBuf>,

        /// Show verbose output during reference scanning (useful for debugging hangs)
        #[arg(long = "verbose-scan")]
        verbose_scan: bool,

        /// Where to write the record of moves [default: ./changes.json]
        #[arg(long = "changes-file")]
        changes_file: Option<PathBuf>,

        /// Where to write proposed reference fixes [default: ./fixes.json]
        #[arg(long = "fixes-file")]
        fixes_file: Option<PathBuf>,

        /// Modify files even if they have uncommitted changes in git
        #[arg(long = "allow-dirty")]
        allow_dirty: bool,
    },

    /// Normalize line endings across files
    Endings {
        #[command(flatten)]
        common: Common,

        #[command(flatten)]
        recursion: Recursion,

        /// Target line ending style: lf, crlf, or cr
        #[arg(short = 's', long = "style", default_value = "lf")]
        style: String,
    },

    /// Normalize indentation (convert between tabs and spaces)
    Indent {
        #[command(flatten)]
        common: Common,

        #[command(flatten)]
        recursion: Recursion,

        /// Target indent style: spaces or tabs
        #[arg(short = 's', long = "style", default_value = "spaces")]
        style: String,

        /// Number of spaces per indent level (or tab width for conversion)
        #[arg(short = 'w', long = "width", default_value_t = 4)]
        width: usize,
    },

    /// Regex find-and-replace across files
    Replace {
        #[command(flatten)]
        common: Common,

        #[command(flatten)]
        recursion: Recursion,

        /// Pattern to find, a regex unless --literal. Repeatable: the Nth
        /// --find pairs with the Nth --replace-with, applied in order
        #[arg(short = 'f', long = "find", required = true)]
        find: Vec<String>,

        /// Replacement for the matching --find (capture groups $1, $2 unless --literal)
        #[arg(long = "replace-with", required = true)]
        replace_with: Vec<String>,

        /// Match every --find as plain text, and insert replacements without $ expansion
        #[arg(long)]
        literal: bool,

        /// Match regardless of case
        #[arg(short = 'i', long = "ignore-case")]
        ignore_case: bool,

        /// Modify files even if they have uncommitted changes in git
        #[arg(long = "allow-dirty")]
        allow_dirty: bool,
    },

    /// Insert or update file headers (license, copyright, etc.)
    Header {
        #[command(flatten)]
        common: Common,

        #[command(flatten)]
        recursion: Recursion,

        /// Header text to insert (use \n for newlines)
        #[arg(short = 't', long = "text")]
        text: String,

        /// Replace {year} in header text with the current year
        #[arg(long = "update-year")]
        update_year: bool,
    },

    /// Apply .editorconfig: trim_trailing_whitespace, insert_final_newline, end_of_line
    Editorconfig {
        #[command(flatten)]
        common: Common,

        #[command(flatten)]
        recursion: Recursion,

        /// Also rewrite indentation from indent_style and indent_size
        #[arg(long)]
        indent: bool,
    },

    /// List the presets in reformat.json and their steps
    Presets,

    /// Print a shell completion script to stdout
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Print the man page, or write one page per command with --out-dir
    Man {
        /// Directory to write reformat.1 and a page per subcommand into
        #[arg(long = "out-dir", value_name = "DIR")]
        out_dir: Option<PathBuf>,
    },

    /// Apply the reference fixes recorded by `group` in a fixes file
    #[command(name = "apply_fixes")]
    ApplyFixes {
        /// The fixes file written by `group` (fixes.json)
        fixes_file: PathBuf,

        /// Show the fixes without applying them
        #[arg(short = 'd', long = "dry-run")]
        dry_run: bool,
    },
}

/// Initialize logging based on verbosity level
fn init_logging(
    verbose: u8,
    quiet: bool,
    log_file: Option<PathBuf>,
    stderr_only: bool,
) -> anyhow::Result<()> {
    // Transformers report each file they touch at `info`, so that is the
    // default level and `--quiet` silences it.
    let log_level = if quiet {
        LevelFilter::Error
    } else {
        match verbose {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    };

    // Terminal output is undecorated: these are user-facing progress lines,
    // not diagnostics, so no timestamp, level or target prefix.
    let term_config = ConfigBuilder::new()
        .set_time_level(LevelFilter::Off)
        .set_max_level(LevelFilter::Off)
        .set_thread_level(LevelFilter::Off)
        .set_target_level(LevelFilter::Off)
        .set_location_level(LevelFilter::Off)
        .build();

    // The log file keeps full detail, timestamps included.
    let file_config = ConfigBuilder::new()
        .set_time_format_rfc3339()
        .set_thread_level(LevelFilter::Off)
        .set_target_level(LevelFilter::Off)
        .build();

    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
        log_level,
        term_config,
        if stderr_only {
            TerminalMode::Stderr
        } else {
            TerminalMode::Mixed
        },
        ColorChoice::Auto,
    )];

    if let Some(log_path) = log_file {
        let file = std::fs::File::create(&log_path)?;
        loggers.push(WriteLogger::new(LevelFilter::Debug, file_config, file));
        warn!("Logging to file: {}", log_path.display());
    }

    CombinedLogger::init(loggers)?;

    debug!("Logging initialized with level: {:?}", log_level);
    Ok(())
}

/// Picks the single selected case format from a group of mutually exclusive
/// clap flags. The group is `required`, so exactly one is set.
fn selected_format(
    camel: bool,
    pascal: bool,
    snake: bool,
    screaming_snake: bool,
    kebab: bool,
) -> &'static str {
    if camel {
        "camel"
    } else if pascal {
        "pascal"
    } else if snake {
        "snake"
    } else if screaming_snake {
        "screaming_snake"
    } else if kebab {
        "kebab"
    } else {
        "screaming_kebab"
    }
}

/// What a step did. Steps compute; callers present.
enum StepOutcome {
    /// Files touched, plus a step-specific unit count (lines, changes,
    /// replacements, endings).
    Counted {
        files: usize,
        units: usize,
    },
    Renamed(reformat_core::RenameStats),
    Grouped(reformat_core::GroupStats),
}

impl StepOutcome {
    fn files(&self) -> usize {
        match self {
            StepOutcome::Counted { files, .. } => *files,
            StepOutcome::Renamed(s) => s.renamed,
            StepOutcome::Grouped(s) => s.files_moved,
        }
    }
}

/// The result of running a pipeline.
struct Execution {
    outcomes: Vec<(String, StepOutcome)>,
    errors: Vec<String>,
}

/// The paths, flags and file selection a command runs with.
struct Invocation<'a> {
    paths: &'a [PathBuf],
    flags: &'a RunFlags,
    select: &'a SelectArgs,
    allow_dirty: bool,
}

/// Steps whose changes need review, so they refuse uncommitted work.
const NEEDS_CLEAN_TREE: &[&str] = &["rename", "group", "convert", "replace"];

/// Steps that change paths rather than contents run on their own.
fn is_content_step(step: &str) -> bool {
    !matches!(step, "rename" | "group")
}

/// The `recursive` setting a step's config resolves to. Every step defaults
/// to recursive when its config leaves it unset.
fn step_recursive(preset: &Preset, step: &str) -> bool {
    match step {
        "rename" => preset.rename.as_ref().and_then(|c| c.recursive),
        "emojis" => preset.emojis.as_ref().and_then(|c| c.recursive),
        "clean" => preset.clean.as_ref().and_then(|c| c.recursive),
        "convert" => preset.convert.as_ref().and_then(|c| c.recursive),
        "endings" => preset.endings.as_ref().and_then(|c| c.recursive),
        "indent" => preset.indent.as_ref().and_then(|c| c.recursive),
        "replace" => preset.replace.as_ref().and_then(|c| c.recursive),
        "header" => preset.header.as_ref().and_then(|c| c.recursive),
        "editorconfig" => preset.editorconfig.as_ref().and_then(|c| c.recursive),
        _ => None,
    }
    .unwrap_or(true)
}

/// With `--include` and no explicit extensions, a step matches any extension:
/// the globs do the selecting, so `--include Makefile` can reach `Makefile`.
fn widen_extensions_for_include(preset: &mut Preset) {
    fn widen(extensions: &mut Option<Vec<String>>) {
        extensions.get_or_insert_with(Vec::new);
    }
    let p = preset;
    widen(
        &mut p
            .emojis
            .get_or_insert_with(Default::default)
            .file_extensions,
    );
    widen(&mut p.clean.get_or_insert_with(Default::default).file_extensions);
    widen(
        &mut p
            .endings
            .get_or_insert_with(Default::default)
            .file_extensions,
    );
    widen(
        &mut p
            .indent
            .get_or_insert_with(Default::default)
            .file_extensions,
    );
    widen(
        &mut p
            .rename
            .get_or_insert_with(Default::default)
            .file_extensions,
    );
    if let Some(c) = p.convert.as_mut() {
        widen(&mut c.file_extensions);
    }
    if let Some(c) = p.replace.as_mut() {
        widen(&mut c.file_extensions);
    }
    if let Some(c) = p.header.as_mut() {
        widen(&mut c.file_extensions);
    }
}

/// Builds the content step `step` from its config.
fn build_content_step(
    preset: &Preset,
    step: &str,
    dry_run: bool,
    label: &str,
) -> anyhow::Result<Box<dyn ContentStep>> {
    let missing = |what: &str| {
        anyhow::anyhow!(
            "{}: '{}' step requires a [{}] config with {}",
            label,
            step,
            step,
            what
        )
    };

    Ok(match step {
        "emojis" => {
            let cfg = preset.emojis.clone().unwrap_or_default();
            Box::new(EmojiTransformer::new(cfg.to_options(dry_run)))
        }
        "clean" => {
            let cfg = preset.clean.clone().unwrap_or_default();
            Box::new(WhitespaceCleaner::new(cfg.to_options(dry_run)))
        }
        "convert" => {
            let cfg = preset
                .convert
                .clone()
                .ok_or_else(|| missing("from_format and to_format"))?;
            Box::new(cfg.to_converter(dry_run)?)
        }
        "endings" => {
            let cfg = preset.endings.clone().unwrap_or_default();
            Box::new(EndingsNormalizer::new(cfg.to_options(dry_run)?))
        }
        "indent" => {
            let cfg = preset.indent.clone().unwrap_or_default();
            Box::new(IndentNormalizer::new(cfg.to_options(dry_run)?))
        }
        "replace" => {
            let cfg = preset.replace.clone().ok_or_else(|| missing("patterns"))?;
            Box::new(reformat_core::ContentReplacer::new(
                cfg.to_options(dry_run)?,
            )?)
        }
        "header" => {
            let cfg = preset.header.clone().ok_or_else(|| missing("text"))?;
            Box::new(HeaderManager::new(cfg.to_options(dry_run)?)?)
        }
        "editorconfig" => {
            let cfg: EditorConfigConfig = preset.editorconfig.clone().unwrap_or_default();
            let indent = cfg.indent.unwrap_or(false);
            Box::new(StyleStep::new(
                move |path: &Path| editorconfig::style_of(path, indent),
                cfg.recursive.unwrap_or(true),
            ))
        }
        _ => unreachable!("step validation should have caught this"),
    })
}

/// Writes data (content, a diff, completions) to stdout.
///
/// Errors are returned rather than panicking as `print!` does, so a closed
/// pipe can end the run quietly.
fn write_stdout(bytes: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes)?;
    stdout.flush()
}

/// Prints a unified diff of one file's change to stdout.
///
/// Carriage returns are shown as `\r`, so a line-ending change is visible.
fn print_diff(path: &Path, before: &[u8], after: &[u8]) -> io::Result<()> {
    let before = String::from_utf8_lossy(before).replace('\r', "\\r");
    let after = String::from_utf8_lossy(after).replace('\r', "\\r");
    let name = path.display().to_string();
    let diff = similar::TextDiff::from_lines(before.as_str(), after.as_str());
    let text = diff
        .unified_diff()
        .header(&format!("a/{}", name), &format!("b/{}", name))
        .to_string();
    write_stdout(text.as_bytes())
}

/// Runs every step of `preset` over the invocation's paths.
///
/// Consecutive content steps share one pass: each file is read once, every
/// step is applied in order, and the file is written once. Rename and group
/// run on their own, since they change paths.
#[time("debug")]
fn execute(label: &str, preset: &Preset, inv: &Invocation) -> anyhow::Result<Execution> {
    reformat_core::config::validate_steps(label, &preset.steps)?;
    for path in inv.paths {
        if std::fs::symlink_metadata(path).is_err() {
            anyhow::bail!("path '{}' does not exist", path.display());
        }
    }
    if inv.flags.writes()
        && preset
            .steps
            .iter()
            .any(|step| NEEDS_CLEAN_TREE.contains(&step.as_str()))
    {
        dirty::ensure_clean(label, inv.paths, inv.allow_dirty)?;
    }

    let mut preset = preset.clone();
    if !inv.select.include.is_empty() {
        widen_extensions_for_include(&mut preset);
    }
    let selection = inv.select.selection();
    let dry_run = !inv.flags.writes();
    let mut exec = Execution {
        outcomes: Vec::new(),
        errors: Vec::new(),
    };

    let steps = &preset.steps;
    let mut i = 0;
    while i < steps.len() {
        let step = steps[i].as_str();
        debug!("Executing step [{}/{}]: {}", i + 1, steps.len(), step);

        if step == "rename" {
            let cfg = preset.rename.clone().unwrap_or_default();
            let options = cfg.to_options(dry_run)?;
            let extensions = cfg.file_extensions.clone().unwrap_or_default();
            let files = select::discover(
                inv.paths,
                &selection,
                options.recursive,
                options.include_symlinks,
            )?
            .into_iter()
            .filter(|f| reformat_core::step::matches_extension(&f.target.path, &extensions))
            .map(|f| (f.target.path, f.is_symlink))
            .collect();
            let stats = FileRenamer::new(options).rename_paths(files);
            exec.outcomes
                .push((step.to_string(), StepOutcome::Renamed(stats)));
            i += 1;
            continue;
        }

        if step == "group" {
            let [path] = inv.paths else {
                anyhow::bail!(
                    "{}: the group step takes exactly one directory, got {} paths",
                    label,
                    inv.paths.len()
                );
            };
            let cfg = preset.group.clone().unwrap_or_default();
            let result = FileGrouper::new(cfg.to_options(dry_run)?).process_with_changes(path)?;
            if !dry_run && !result.changes.is_empty() {
                let changes_path = resolve_record_path(None, "changes.json")?;
                result.changes.write_to_file(&changes_path)?;
                info!("Changes recorded to: {}", changes_path.display());
            }
            exec.outcomes
                .push((step.to_string(), StepOutcome::Grouped(result.stats)));
            i += 1;
            continue;
        }

        let end = (i..steps.len())
            .find(|&j| !is_content_step(&steps[j]))
            .unwrap_or(steps.len());
        let names = &steps[i..end];

        let built = names
            .iter()
            .map(|name| build_content_step(&preset, name, dry_run, label))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let refs: Vec<&dyn ContentStep> = built.iter().map(|b| b.as_ref()).collect();

        let recursive = names.iter().any(|name| step_recursive(&preset, name));
        let files = select::discover(inv.paths, &selection, recursive, false)?
            .into_iter()
            .map(|f| f.target);

        let show_diff = inv.flags.diff;
        let mut diff_error = None;
        let report =
            reformat_core::step::run_content_steps(&refs, files, dry_run, &mut |file, a, b| {
                if show_diff && diff_error.is_none() {
                    diff_error = print_diff(&file.path, a, b).err();
                }
            });
        if let Some(e) = diff_error {
            return Err(e.into());
        }

        for totals in report.steps {
            exec.outcomes.push((
                totals.name.to_string(),
                StepOutcome::Counted {
                    files: totals.files,
                    units: totals.units,
                },
            ));
        }
        exec.errors.extend(report.errors);
        i = end;
    }

    Ok(exec)
}

/// Converts an execution into the process outcome: an error if any file
/// failed, otherwise whether `--check` found changes.
fn finish(exec: &Execution, flags: &RunFlags) -> anyhow::Result<bool> {
    if !exec.errors.is_empty() {
        anyhow::bail!("{} file(s) could not be processed", exec.errors.len());
    }
    let changed = exec.outcomes.iter().any(|(_, o)| o.files() > 0);
    if flags.check && changed {
        info!("Check failed: some files would change.");
    }
    Ok(flags.check && changed)
}

/// Runs a single step as a standalone subcommand, presenting the result the
/// way that command always has.
fn run_single_step(
    step: &str,
    preset: Preset,
    common: &Common,
    allow_dirty: bool,
    summary: impl Fn(&StepOutcome) -> String,
    empty: &str,
) -> anyhow::Result<bool> {
    let inv = Invocation {
        paths: &common.paths,
        flags: &common.run,
        select: &common.select,
        allow_dirty,
    };
    if let Some(name) = &common.run.stdin_filename {
        return run_stdin(step, &preset, name, &inv);
    }
    let exec = execute(step, &preset, &inv)?;
    let (_, outcome) = &exec.outcomes[0];

    if let StepOutcome::Renamed(ref stats) = outcome {
        if stats.skipped > 0 {
            warn!("Skipped {} file(s):", stats.skipped);
            for message in &stats.errors {
                warn!("  {}", message);
            }
        }
    }

    if outcome.files() > 0 {
        info!("{}{}", common.run.prefix(), summary(outcome));
    } else {
        info!("{}", empty);
    }
    finish(&exec, &common.run)
}

/// The counts of a content step's outcome.
fn counted(outcome: &StepOutcome) -> (usize, usize) {
    match outcome {
        StepOutcome::Counted { files, units } => (*files, *units),
        _ => unreachable!("content steps are counted"),
    }
}

fn preset_of(step: &str) -> Preset {
    Preset {
        steps: vec![step.to_string()],
        ..Default::default()
    }
}

/// Runs a pipeline (a preset or a job) and reports each step.
fn run_pipeline(name: &str, preset: &Preset, inv: &Invocation) -> anyhow::Result<bool> {
    if let Some(stdin_name) = &inv.flags.stdin_filename {
        return run_stdin(name, preset, stdin_name, inv);
    }
    debug!(
        "Running '{}' with {} step(s) on {} path(s)",
        name,
        preset.steps.len(),
        inv.paths.len()
    );
    let exec = execute(name, preset, inv)?;
    let flags = inv.flags;
    let prefix = flags.prefix();

    for (step, outcome) in &exec.outcomes {
        match outcome {
            StepOutcome::Renamed(stats) => {
                if stats.renamed > 0 {
                    info!("{}  rename: {} file(s) renamed", prefix, stats.renamed);
                } else {
                    info!("  rename: no files needed renaming");
                }
                if stats.skipped > 0 {
                    warn!("  rename: skipped {} file(s)", stats.skipped);
                    for message in &stats.errors {
                        warn!("    {}", message);
                    }
                }
            }
            StepOutcome::Grouped(stats) => {
                if stats.files_moved > 0 {
                    info!(
                        "{}  group: {} dir(s) created, {} file(s) moved",
                        prefix, stats.dirs_created, stats.files_moved
                    );
                } else {
                    info!("  group: no files needed grouping");
                }
            }
            StepOutcome::Counted { files, units } => {
                if *files > 0 {
                    info!(
                        "{}  {}: {} file(s), {} change(s)",
                        prefix, step, files, units
                    );
                } else {
                    info!("  {}: nothing to do", step);
                }
            }
        }
    }

    info!("Pipeline '{}' complete.", name);
    finish(&exec, flags)
}

#[time("debug")]
fn run_preset(name: &str, config_file: Option<&Path>, inv: &Invocation) -> anyhow::Result<bool> {
    let (_, config) = load_presets(config_file, inv.paths)?;
    let preset = config::get_preset(&config, name)?;
    run_pipeline(name, preset, inv)
}

/// Finds and loads the preset config, or explains where it looked.
fn load_presets(
    config_file: Option<&Path>,
    paths: &[PathBuf],
) -> anyhow::Result<(PathBuf, reformat_core::ReformatConfig)> {
    config::find_config(config_file, paths)?.ok_or_else(|| {
        anyhow::anyhow!(
            "reformat.json not found in the current directory, the target paths, \
             or their parents. Pass --config to name one."
        )
    })
}

fn run_list_presets(config_file: Option<&Path>) -> anyhow::Result<bool> {
    let (path, config) = load_presets(config_file, &[])?;
    let mut names: Vec<&String> = config.keys().collect();
    names.sort();
    let mut text = format!("{}:\n", path.display());
    for name in names {
        text.push_str(&format!("  {}: {}\n", name, config[name].steps.join(", ")));
    }
    write_stdout(text.as_bytes())?;
    Ok(false)
}

#[time("debug")]
fn run_job(job_source: &str, inv: &Invocation) -> anyhow::Result<bool> {
    let content = if job_source == "-" {
        if inv.flags.stdin_filename.is_some() {
            anyhow::bail!("--job - and --stdin-filename both read stdin; put the job in a file");
        }
        debug!("Reading job from stdin");
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
        buf
    } else {
        debug!("Reading job from file: {}", job_source);
        std::fs::read_to_string(job_source)
            .map_err(|e| anyhow::anyhow!("failed to read job file '{}': {}", job_source, e))?
    };

    let preset: Preset = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse job: {}", e))?;

    let label = if job_source == "-" {
        "stdin"
    } else {
        job_source
    };
    run_pipeline(label, &preset, inv)
}

/// The default command: replace task emojis and strip trailing whitespace.
#[time("debug")]
fn run_default(recursive: bool, inv: &Invocation) -> anyhow::Result<bool> {
    let preset = Preset {
        steps: vec!["emojis".to_string(), "clean".to_string()],
        emojis: Some(EmojiConfig {
            recursive: Some(recursive),
            ..Default::default()
        }),
        clean: Some(CleanConfig {
            recursive: Some(recursive),
            ..Default::default()
        }),
        ..Default::default()
    };
    if let Some(name) = &inv.flags.stdin_filename {
        return run_stdin("default", &preset, name, inv);
    }
    let flags = inv.flags;
    let exec = execute("default", &preset, inv)?;
    let (emoji_files, emoji_changes) = counted(&exec.outcomes[0].1);
    let (clean_files, clean_lines) = counted(&exec.outcomes[1].1);

    if emoji_files > 0 || clean_files > 0 {
        info!("{}Processed files:", flags.prefix());
        if emoji_files > 0 {
            info!(
                "  - Emoji transformations: {} file(s) ({} changes)",
                emoji_files, emoji_changes
            );
        }
        if clean_files > 0 {
            info!(
                "  - Whitespace cleaned: {} file(s) ({} lines)",
                clean_files, clean_lines
            );
        }
    } else {
        info!("No files needed processing");
    }
    finish(&exec, flags)
}

/// Resolves where a JSON record should be written, warning before replacing
/// an existing file rather than silently clobbering it.
fn resolve_record_path(explicit: Option<PathBuf>, default_name: &str) -> anyhow::Result<PathBuf> {
    let path = match explicit {
        Some(p) => p,
        None => std::env::current_dir()?.join(default_name),
    };
    if path.exists() {
        warn!("Overwriting existing file: {}", path.display());
    }
    Ok(path)
}

/// Prompts the user for a yes/no answer
fn prompt_yes_no(question: &str) -> bool {
    print!("{} [y/N]: ", question);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Checks if scope_path contains or is a parent of target_path
/// Returns a warning message if there's an overlap, None otherwise
fn check_scope_overlap(scope_path: &Path, target_path: &Path) -> Option<String> {
    // Canonicalize both paths for accurate comparison
    let scope_canonical = scope_path
        .canonicalize()
        .unwrap_or_else(|_| scope_path.to_path_buf());
    let target_canonical = target_path
        .canonicalize()
        .unwrap_or_else(|_| target_path.to_path_buf());

    // Check if scope contains target (scope is parent of target)
    if target_canonical.starts_with(&scope_canonical) {
        return Some(format!(
            "Warning: --scope '{}' contains the target directory '{}'\n\
             This will scan the newly created group directories and may be slow.\n\
             Consider using a more specific --scope that only includes directories\n\
             with files that reference the moved files (e.g., --scope ./src).",
            scope_path.display(),
            target_path.display()
        ));
    }

    // Check if target contains scope (target is parent of scope) - less common but worth noting
    if scope_canonical.starts_with(&target_canonical) {
        return Some(format!(
            "Warning: --scope '{}' is inside the target directory '{}'.\n\
             This is unusual - typically --scope should point to directories\n\
             containing files that reference the moved files.",
            scope_path.display(),
            target_path.display()
        ));
    }

    None
}

/// Prompts the user for directories to scan
fn prompt_scan_dirs(default_dir: &Path) -> Vec<PathBuf> {
    print!(
        "Enter directories to scan (comma-separated, or press Enter for '{}'): ",
        default_dir.display()
    );
    io::stdout().flush().unwrap();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
        return vec![default_dir.to_path_buf()];
    }

    input
        .trim()
        .split(',')
        .map(|s| PathBuf::from(s.trim()))
        .collect()
}

/// Logs what applying a fix record did.
fn report_applied(result: &reformat_core::ApplyResult) {
    info!(
        "\nFixed {} reference(s) in {} file(s).",
        result.references_fixed, result.files_modified
    );

    if result.references_skipped > 0 {
        info!(
            "Skipped {} reference(s): the recorded text no longer \
             matches the file. The fixes may already have been \
             applied, or the file changed since the scan.",
            result.references_skipped
        );
    }

    for err in &result.errors {
        log::error!("  - {}", err);
    }
}

#[allow(clippy::too_many_arguments)]
#[time("debug")]
fn run_group(
    path: PathBuf,
    recursive: bool,
    dry_run: bool,
    separator: char,
    min_count: usize,
    strip_prefix: bool,
    from_suffix: bool,
    preview: bool,
    no_interactive: bool,
    scope: Option<PathBuf>,
    verbose_scan: bool,
    changes_file: Option<PathBuf>,
    fixes_file: Option<PathBuf>,
    allow_dirty: bool,
) -> anyhow::Result<bool> {
    debug!("Grouping files by prefix in: {}", path.display());
    debug!(
        "Recursive: {}, Dry run: {}, Separator: '{}', Min count: {}",
        recursive, dry_run, separator, min_count
    );

    let cfg = GroupConfig {
        separator: Some(separator.to_string()),
        min_count: Some(min_count),
        strip_prefix: Some(strip_prefix),
        from_suffix: Some(from_suffix),
        recursive: Some(recursive),
    };

    let grouper = FileGrouper::new(cfg.to_options(dry_run)?);

    if preview {
        if !path.is_dir() {
            anyhow::bail!("Path is not a directory: {}", path.display());
        }
        let mut dirs = vec![path.clone()];
        if recursive {
            dirs.extend(reformat_core::walk::walk_dirs(&path, true));
        }

        let mut total = 0;
        for dir in &dirs {
            let groups = grouper.preview(dir)?;
            if groups.is_empty() {
                continue;
            }
            total += groups.len();
            if recursive {
                info!("\n{}:", dir.display());
            }
            for (prefix, files) in &groups {
                info!("\n  {} ({} files):", prefix, files.len());
                for file in files {
                    info!("    - {}", file);
                }
            }
        }

        if total == 0 {
            info!(
                "No file groups found matching criteria (min_count: {})",
                min_count
            );
        } else {
            info!("\nFound {} potential group(s).", total);
        }
        return Ok(false);
    }
    if !dry_run {
        let mut touched = vec![path.clone()];
        touched.extend(scope.clone());
        dirty::ensure_clean("group", &touched, allow_dirty)?;
    }
    let result = grouper.process_with_changes(&path)?;

    let stats = &result.stats;

    if stats.files_moved == 0 {
        info!("No files needed grouping");
        return Ok(false);
    }

    let prefix_str = if dry_run { "[DRY-RUN] " } else { "" };
    info!("{}Grouping complete:", prefix_str);
    if stats.dirs_created > 0 {
        info!("  - Directories created: {}", stats.dirs_created);
    }
    info!("  - Files moved: {}", stats.files_moved);
    if stats.files_renamed > 0 {
        info!(
            "  - Files renamed (prefix stripped): {}",
            stats.files_renamed
        );
    }

    // A dry run must not leave anything behind: a record of moves that did
    // not happen would mislead the reference fixer.
    if dry_run {
        info!("[DRY-RUN] No changes file written.");
        return Ok(false);
    }
    if result.changes.is_empty() {
        return Ok(false);
    }

    let changes_path = resolve_record_path(changes_file, "changes.json")?;
    result.changes.write_to_file(&changes_path)?;
    info!("\nChanges recorded to: {}", changes_path.display());

    let dirs_to_scan = if !no_interactive {
        info!("");
        if !prompt_yes_no("Would you like to scan for broken references?") {
            return Ok(false);
        }
        match scope {
            Some(dir) => vec![dir],
            None => prompt_scan_dirs(&path),
        }
    } else if let Some(dir) = scope {
        vec![dir]
    } else {
        return Ok(false);
    };

    for scan_dir in &dirs_to_scan {
        if let Some(warning) = check_scope_overlap(scan_dir, &path) {
            warn!("\n{}\n", warning);
        }
    }

    let scan_options = ScanOptions {
        verbose: verbose_scan,
        ..Default::default()
    };
    debug!("Scanning for broken references...");
    let scanner = ReferenceScanner::from_change_record(&result.changes, scan_options)?;
    let fix_record = scanner.scan(&dirs_to_scan)?;

    if fix_record.is_empty() {
        info!("\nNo broken references found.");
        return Ok(false);
    }

    let fixes_path = resolve_record_path(fixes_file, "fixes.json")?;
    fix_record.write_to_file(&fixes_path)?;
    info!("\nFound {} broken reference(s).", fix_record.len());
    info!("Proposed fixes written to: {}", fixes_path.display());

    if no_interactive {
        info!(
            "Review it, then apply with: reformat apply_fixes {}",
            fixes_path.display()
        );
        return Ok(false);
    }

    info!("\nProposed fixes:");
    for fix in fix_record.fixes.iter().take(10) {
        info!(
            "  {}:{}: '{}' -> '{}'",
            fix.file, fix.line, fix.old_reference, fix.new_reference
        );
    }
    if fix_record.len() > 10 {
        info!("  ... and {} more (see fixes.json)", fix_record.len() - 10);
    }

    info!("");
    if prompt_yes_no("Review fixes.json and apply changes?") {
        // The scan directories were chosen at the prompt, after the guard
        // above ran. Check the files the fixes edit, except those the
        // grouping itself just moved.
        let moved = path.canonicalize().unwrap_or_else(|_| path.clone());
        let mut to_edit: Vec<PathBuf> = fix_record
            .fixes
            .iter()
            .map(|fix| PathBuf::from(&fix.file))
            .filter(|file| {
                !file
                    .canonicalize()
                    .is_ok_and(|real| real.starts_with(&moved))
            })
            .collect();
        to_edit.sort();
        to_edit.dedup();
        dirty::ensure_clean("group", &to_edit, allow_dirty).map_err(|e| {
            anyhow::anyhow!(
                "{}\nThe grouping is done; apply the fixes later with: reformat apply_fixes {}",
                e,
                fixes_path.display()
            )
        })?;

        let apply_result = ReferenceFixer::apply_fixes(&fix_record)?;
        report_applied(&apply_result);
        if !apply_result.errors.is_empty() {
            anyhow::bail!("{} file(s) could not be fixed", apply_result.errors.len());
        }
    } else {
        info!(
            "Fixes not applied. Review them, then run: reformat apply_fixes {}",
            fixes_path.display()
        );
    }

    Ok(false)
}

#[time("debug")]
fn run_apply_fixes(fixes_file: &Path, dry_run: bool) -> anyhow::Result<bool> {
    let record = FixRecord::read_from_file(fixes_file)
        .map_err(|e| anyhow::anyhow!("failed to read '{}': {}", fixes_file.display(), e))?;

    if record.is_empty() {
        info!("No fixes recorded in {}", fixes_file.display());
        return Ok(false);
    }

    if dry_run {
        for line in ReferenceFixer::dry_run(&record) {
            info!("[DRY-RUN] {}", line);
        }
        info!("[DRY-RUN] {} fix(es) would be applied", record.len());
        return Ok(false);
    }

    let result = ReferenceFixer::apply_fixes(&record)?;
    report_applied(&result);
    if !result.errors.is_empty() {
        anyhow::bail!("{} file(s) could not be fixed", result.errors.len());
    }
    Ok(false)
}

impl Commands {
    /// The shared arguments of the commands that have them.
    fn common(&self) -> Option<&Common> {
        match self {
            Commands::Convert { common, .. }
            | Commands::Clean { common, .. }
            | Commands::Emojis { common, .. }
            | Commands::RenameFiles { common, .. }
            | Commands::Endings { common, .. }
            | Commands::Indent { common, .. }
            | Commands::Replace { common, .. }
            | Commands::Header { common, .. }
            | Commands::Editorconfig { common, .. } => Some(common),
            _ => None,
        }
    }
}

/// Applies the content steps of `preset` to stdin, as if it were the file
/// `name`, and writes the result to stdout.
///
/// `--diff` prints a diff instead of the content. `--check` and `--dry-run`
/// print nothing; `--check` still sets the exit status. Content that `name`
/// does not select, or that no step accepts, is written back unchanged.
fn run_stdin(label: &str, preset: &Preset, name: &Path, inv: &Invocation) -> anyhow::Result<bool> {
    reformat_core::config::validate_steps(label, &preset.steps)?;
    if let Some(step) = preset.steps.iter().find(|s| !is_content_step(s)) {
        anyhow::bail!(
            "{}: the {} step renames or moves files, so it cannot read stdin",
            label,
            step
        );
    }

    let mut preset = preset.clone();
    if !inv.select.include.is_empty() {
        widen_extensions_for_include(&mut preset);
    }
    let dry_run = !inv.flags.writes();
    let built = preset
        .steps
        .iter()
        .map(|step| build_content_step(&preset, step, dry_run, label))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let refs: Vec<&dyn ContentStep> = built.iter().map(|b| b.as_ref()).collect();

    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;

    let output = if select::named_file_selected(name, &inv.select.selection())? {
        reformat_core::apply_to_bytes(&refs, &FileTarget::file(name), &input, dry_run).0
    } else {
        input.clone()
    };
    let changed = output != input;

    if inv.flags.diff {
        if changed {
            print_diff(name, &input, &output)?;
        }
    } else if !(inv.flags.check || inv.flags.dry_run) {
        write_stdout(&output)?;
    }
    Ok(inv.flags.check && changed)
}

fn run(cli: Cli) -> anyhow::Result<bool> {
    let Some(command) = cli.command else {
        if cli.paths.is_empty() && cli.run.stdin_filename.is_none() {
            anyhow::bail!("no command or path specified. Use --help for usage information.");
        }
        let inv = Invocation {
            paths: &cli.paths,
            flags: &cli.run,
            select: &cli.select,
            allow_dirty: cli.allow_dirty,
        };
        return if let Some(preset_name) = &cli.preset {
            run_preset(preset_name, cli.config.as_deref(), &inv)
        } else if let Some(job_source) = &cli.job {
            run_job(job_source, &inv)
        } else {
            run_default(cli.recursive, &inv)
        };
    };

    match command {
        Commands::Convert {
            from_camel,
            from_pascal,
            from_snake,
            from_screaming_snake,
            from_kebab,
            from_screaming_kebab: _,
            to_camel,
            to_pascal,
            to_snake,
            to_screaming_snake,
            to_kebab,
            to_screaming_kebab: _,
            common,
            recursive,
            prefix,
            suffix,
            strip_prefix,
            strip_suffix,
            replace_prefix_from,
            replace_prefix_to,
            replace_suffix_from,
            replace_suffix_to,
            glob,
            word_filter,
            allow_dirty,
        } => {
            let cfg = ConvertConfig {
                from_format: Some(
                    selected_format(
                        from_camel,
                        from_pascal,
                        from_snake,
                        from_screaming_snake,
                        from_kebab,
                    )
                    .to_string(),
                ),
                to_format: Some(
                    selected_format(to_camel, to_pascal, to_snake, to_screaming_snake, to_kebab)
                        .to_string(),
                ),
                file_extensions: common.extensions.clone(),
                recursive: Some(recursive),
                prefix: Some(prefix),
                suffix: Some(suffix),
                glob,
                word_filter,
                strip_prefix,
                strip_suffix,
                replace_prefix_from,
                replace_prefix_to,
                replace_suffix_from,
                replace_suffix_to,
            };
            run_single_step(
                "convert",
                Preset {
                    convert: Some(cfg),
                    ..preset_of("convert")
                },
                &common,
                allow_dirty,
                |o| {
                    let (files, units) = counted(o);
                    format!("Converted {} identifier(s) in {} file(s)", units, files)
                },
                "No changes needed",
            )
        }

        Commands::Clean {
            common,
            recursion,
            final_newline,
            trim_blank_lines,
        } => {
            let cfg = CleanConfig {
                remove_trailing: Some(true),
                insert_final_newline: Some(final_newline),
                trim_trailing_blank_lines: Some(trim_blank_lines),
                file_extensions: common.extensions.clone(),
                recursive: Some(recursion.enabled()),
            };
            run_single_step(
                "clean",
                Preset {
                    clean: Some(cfg),
                    ..preset_of("clean")
                },
                &common,
                false,
                |o| {
                    let (files, units) = counted(o);
                    format!("Cleaned {} lines in {} file(s)", units, files)
                },
                "No files needed cleaning",
            )
        }

        Commands::Emojis {
            common,
            recursion,
            replace_task: _,
            no_replace_task,
            remove_other: _,
            no_remove_other,
        } => {
            let cfg = EmojiConfig {
                replace_task_emojis: Some(!no_replace_task),
                remove_other_emojis: Some(!no_remove_other),
                file_extensions: common.extensions.clone(),
                recursive: Some(recursion.enabled()),
            };
            run_single_step(
                "emojis",
                Preset {
                    emojis: Some(cfg),
                    ..preset_of("emojis")
                },
                &common,
                false,
                |o| {
                    let (files, units) = counted(o);
                    format!(
                        "Transformed emojis in {} file(s) ({} changes)",
                        files, units
                    )
                },
                "No files contained emojis to transform",
            )
        }

        Commands::RenameFiles {
            common,
            recursion,
            include_symlinks,
            to_lowercase,
            to_uppercase,
            to_capitalize,
            underscored,
            hyphenated,
            add_prefix,
            rm_prefix,
            add_suffix,
            rm_suffix,
            replace_prefix,
            replace_suffix,
            timestamp_long,
            timestamp_short,
            allow_dirty,
        } => {
            let case_transform = if to_lowercase {
                Some("lowercase")
            } else if to_uppercase {
                Some("uppercase")
            } else if to_capitalize {
                Some("capitalize")
            } else {
                None
            };
            let space_replace = if underscored {
                Some("underscore")
            } else if hyphenated {
                Some("hyphen")
            } else {
                None
            };
            let timestamp = if timestamp_long {
                Some("long")
            } else if timestamp_short {
                Some("short")
            } else {
                None
            };

            let cfg = RenameConfig {
                case_transform: case_transform.map(String::from),
                space_replace: space_replace.map(String::from),
                recursive: Some(recursion.enabled()),
                include_symlinks: Some(include_symlinks),
                add_prefix,
                remove_prefix: rm_prefix,
                add_suffix,
                remove_suffix: rm_suffix,
                replace_prefix,
                replace_suffix,
                timestamp: timestamp.map(String::from),
                file_extensions: common.extensions.clone(),
            };
            run_single_step(
                "rename",
                Preset {
                    rename: Some(cfg),
                    ..preset_of("rename")
                },
                &common,
                allow_dirty,
                |o| format!("Renamed {} file(s)", o.files()),
                "No files needed renaming",
            )
        }

        Commands::Group {
            path,
            recursive,
            dry_run,
            separator,
            min_count,
            strip_prefix,
            from_suffix,
            preview,
            no_interactive,
            scope,
            verbose_scan,
            changes_file,
            fixes_file,
            allow_dirty,
        } => run_group(
            path,
            recursive,
            dry_run,
            separator,
            min_count,
            strip_prefix,
            from_suffix,
            preview,
            no_interactive,
            scope,
            verbose_scan,
            changes_file,
            fixes_file,
            allow_dirty,
        ),

        Commands::Endings {
            common,
            recursion,
            style,
        } => {
            let cfg = EndingsConfig {
                style: Some(style),
                file_extensions: common.extensions.clone(),
                recursive: Some(recursion.enabled()),
            };
            run_single_step(
                "endings",
                Preset {
                    endings: Some(cfg),
                    ..preset_of("endings")
                },
                &common,
                false,
                |o| {
                    let (files, units) = counted(o);
                    format!("Normalized {} ending(s) in {} file(s)", units, files)
                },
                "No files needed line ending normalization",
            )
        }

        Commands::Indent {
            common,
            recursion,
            style,
            width,
        } => {
            let cfg = IndentConfig {
                style: Some(style),
                width: Some(width),
                file_extensions: common.extensions.clone(),
                recursive: Some(recursion.enabled()),
            };
            run_single_step(
                "indent",
                Preset {
                    indent: Some(cfg),
                    ..preset_of("indent")
                },
                &common,
                false,
                |o| {
                    let (files, units) = counted(o);
                    format!("Normalized {} line(s) in {} file(s)", units, files)
                },
                "No files needed indentation normalization",
            )
        }

        Commands::Replace {
            common,
            recursion,
            find,
            replace_with,
            literal,
            ignore_case,
            allow_dirty,
        } => {
            if find.len() != replace_with.len() {
                anyhow::bail!(
                    "--find was given {} time(s) and --replace-with {}; they pair in order",
                    find.len(),
                    replace_with.len()
                );
            }
            let cfg = ReplaceConfig {
                patterns: Some(
                    find.into_iter()
                        .zip(replace_with)
                        .map(|(find, replace)| ReplacePatternEntry {
                            find,
                            replace,
                            literal,
                            ignore_case,
                        })
                        .collect(),
                ),
                file_extensions: common.extensions.clone(),
                recursive: Some(recursion.enabled()),
            };
            run_single_step(
                "replace",
                Preset {
                    replace: Some(cfg),
                    ..preset_of("replace")
                },
                &common,
                allow_dirty,
                |o| {
                    let (files, units) = counted(o);
                    format!("Made {} replacement(s) in {} file(s)", units, files)
                },
                "No files matched the pattern",
            )
        }

        Commands::Header {
            common,
            recursion,
            text,
            update_year,
        } => {
            let cfg = HeaderConfig {
                // Allow \n in the argument to stand for a real newline.
                text: Some(text.replace("\\n", "\n")),
                update_year: Some(update_year),
                file_extensions: common.extensions.clone(),
                recursive: Some(recursion.enabled()),
            };
            run_single_step(
                "header",
                Preset {
                    header: Some(cfg),
                    ..preset_of("header")
                },
                &common,
                false,
                |o| format!("Updated headers in {} file(s)", o.files()),
                "All files already have correct headers",
            )
        }

        Commands::Editorconfig {
            common,
            recursion,
            indent,
        } => {
            if common.extensions.is_some() {
                anyhow::bail!("editorconfig selects files from .editorconfig; use --include or --exclude instead of -e");
            }
            let cfg = EditorConfigConfig {
                indent: Some(indent),
                recursive: Some(recursion.enabled()),
            };
            run_single_step(
                "editorconfig",
                Preset {
                    editorconfig: Some(cfg),
                    ..preset_of("editorconfig")
                },
                &common,
                false,
                |o| {
                    let (files, units) = counted(o);
                    format!(
                        "Applied EditorConfig to {} file(s) ({} changes)",
                        files, units
                    )
                },
                "All files already match .editorconfig",
            )
        }

        Commands::ApplyFixes {
            fixes_file,
            dry_run,
        } => run_apply_fixes(&fixes_file, dry_run),

        Commands::Presets => run_list_presets(cli.config.as_deref()),

        Commands::Completions { shell } => {
            let mut script = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "reformat", &mut script);
            write_stdout(&script)?;
            Ok(false)
        }

        Commands::Man { out_dir } => {
            match out_dir {
                Some(dir) => {
                    std::fs::create_dir_all(&dir)?;
                    clap_mangen::generate_to(Cli::command(), &dir)?;
                    info!("Wrote man pages to {}", dir.display());
                }
                None => {
                    let mut page = Vec::new();
                    clap_mangen::Man::new(Cli::command()).render(&mut page)?;
                    write_stdout(&page)?;
                }
            }
            Ok(false)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let stderr_only = match &cli.command {
        None => cli.run.stdout_is_data(),
        Some(Commands::Completions { .. } | Commands::Man { out_dir: None }) => true,
        Some(command) => command.common().is_some_and(|c| c.run.stdout_is_data()),
    };
    if let Err(e) = init_logging(cli.verbose, cli.quiet, cli.log_file.clone(), stderr_only) {
        eprintln!("Warning: Failed to initialize logging: {}", e);
    }
    debug!("CLI arguments parsed successfully");

    match run(cli) {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::from(1),
        // A reader such as `head` closed stdout early; stop quietly.
        Err(e)
            if e.downcast_ref::<io::Error>()
                .is_some_and(|io| io.kind() == io::ErrorKind::BrokenPipe) =>
        {
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
            ExitCode::from(2)
        }
    }
}
