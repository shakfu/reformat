use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info};
use logging_timer::time;
use reformat_core::{
    CaseConverter, CaseFormat, CaseTransform, CombinedOptions, CombinedProcessor, EmojiOptions,
    EmojiTransformer, FileGrouper, FileRenamer, GroupOptions, ReferenceFixer, ReferenceScanner,
    RenameOptions, ScanOptions, SpaceReplace, TimestampFormat, WhitespaceCleaner,
    WhitespaceOptions,
};
use simplelog::*;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "reformat",
    version = "0.2.0",
    about = "Code transformation tool for case conversion and cleaning",
    long_about = "A modular code transformation framework.\n\n\
                  Usage:\n\
                  - reformat <path>: Run all transformations (rename to lowercase, emojis, clean)\n\
                  - reformat -r <path>: Run all transformations recursively\n\n\
                  Commands:\n\
                  - convert: Convert between case formats\n\
                  - clean: Remove trailing whitespace\n\
                  - emojis: Remove or replace emojis with text alternatives\n\
                  - rename_files: Rename files with various transformations"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// The directory or file to process (when no subcommand is specified)
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Process files recursively (when no subcommand is specified)
    #[arg(short = 'r', long, requires = "path")]
    recursive: bool,

    /// Dry run (don't modify files, when no subcommand is specified)
    #[arg(short = 'd', long = "dry-run", requires = "path")]
    dry_run: bool,

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

        /// The directory or file to convert
        path: PathBuf,

        /// Convert files recursively
        #[arg(short = 'r', long)]
        recursive: bool,

        /// Dry run the conversion
        #[arg(short = 'd', long = "dry-run")]
        dry_run: bool,

        /// File extensions to process
        #[arg(short = 'e', long = "extensions")]
        extensions: Option<Vec<String>>,

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
        #[arg(long = "replace-prefix-from")]
        replace_prefix_from: Option<String>,

        /// Replace prefix (to) before conversion (e.g., 'Abstract')
        #[arg(long = "replace-prefix-to", requires = "replace_prefix_from")]
        replace_prefix_to: Option<String>,

        /// Replace suffix (from) before conversion
        #[arg(long = "replace-suffix-from")]
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
    },

    /// Remove trailing whitespace from files
    Clean {
        /// The directory or file to clean
        path: PathBuf,

        /// Process files recursively
        #[arg(short = 'r', long, default_value_t = true)]
        recursive: bool,

        /// Dry run (don't modify files)
        #[arg(short = 'd', long = "dry-run")]
        dry_run: bool,

        /// File extensions to process
        #[arg(short = 'e', long = "extensions")]
        extensions: Option<Vec<String>>,
    },

    /// Remove or replace emojis with text alternatives
    Emojis {
        /// The directory or file to process
        path: PathBuf,

        /// Process files recursively [default: true]
        #[arg(short = 'r', long, default_value_t = true)]
        recursive: bool,

        /// Dry run (don't modify files)
        #[arg(short = 'd', long = "dry-run")]
        dry_run: bool,

        /// File extensions to process (default: .md, .txt, and common source files)
        #[arg(short = 'e', long = "extensions")]
        extensions: Option<Vec<String>>,

        /// Replace task completion emojis with text (e.g., ✅ -> [x]) [default: true]
        #[arg(long = "replace-task", default_value_t = true)]
        replace_task: bool,

        /// Remove all other emojis [default: true]
        #[arg(long = "remove-other", default_value_t = true)]
        remove_other: bool,
    },

    /// Rename files with various transformations
    #[command(name = "rename_files")]
    RenameFiles {
        /// The directory or file to rename
        path: PathBuf,

        /// Process directories recursively [default: true]
        #[arg(short = 'r', long, default_value_t = true)]
        recursive: bool,

        /// Dry run (don't rename files)
        #[arg(short = 'd', long = "dry-run")]
        dry_run: bool,

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
    },
}

/// Initialize logging based on verbosity level
fn init_logging(verbose: u8, quiet: bool, log_file: Option<PathBuf>) -> anyhow::Result<()> {
    let log_level = if quiet {
        LevelFilter::Error
    } else {
        match verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    };

    let config = ConfigBuilder::new()
        .set_time_format_rfc3339()
        .set_thread_level(LevelFilter::Off)
        .set_target_level(LevelFilter::Off)
        .build();

    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
        log_level,
        config.clone(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )];

    if let Some(log_path) = log_file {
        let file = std::fs::File::create(&log_path)?;
        loggers.push(WriteLogger::new(LevelFilter::Debug, config, file));
        eprintln!("Logging to file: {}", log_path.display());
    }

    CombinedLogger::init(loggers)?;

    debug!("Logging initialized with level: {:?}", log_level);
    Ok(())
}

/// Create a progress spinner
fn create_spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner
}

fn determine_case_format(
    from_camel: bool,
    from_pascal: bool,
    from_snake: bool,
    from_screaming_snake: bool,
    from_kebab: bool,
    _from_screaming_kebab: bool,
) -> CaseFormat {
    if from_camel {
        CaseFormat::CamelCase
    } else if from_pascal {
        CaseFormat::PascalCase
    } else if from_snake {
        CaseFormat::SnakeCase
    } else if from_screaming_snake {
        CaseFormat::ScreamingSnakeCase
    } else if from_kebab {
        CaseFormat::KebabCase
    } else {
        CaseFormat::ScreamingKebabCase
    }
}

#[allow(clippy::too_many_arguments)]
#[time("info")]
fn run_convert(
    from_camel: bool,
    from_pascal: bool,
    from_snake: bool,
    from_screaming_snake: bool,
    from_kebab: bool,
    from_screaming_kebab: bool,
    to_camel: bool,
    to_pascal: bool,
    to_snake: bool,
    to_screaming_snake: bool,
    to_kebab: bool,
    to_screaming_kebab: bool,
    path: PathBuf,
    recursive: bool,
    dry_run: bool,
    extensions: Option<Vec<String>>,
    prefix: String,
    suffix: String,
    strip_prefix: Option<String>,
    strip_suffix: Option<String>,
    replace_prefix_from: Option<String>,
    replace_prefix_to: Option<String>,
    replace_suffix_from: Option<String>,
    replace_suffix_to: Option<String>,
    glob: Option<String>,
    word_filter: Option<String>,
) -> anyhow::Result<()> {
    let from_format = determine_case_format(
        from_camel,
        from_pascal,
        from_snake,
        from_screaming_snake,
        from_kebab,
        from_screaming_kebab,
    );

    let to_format = determine_case_format(
        to_camel,
        to_pascal,
        to_snake,
        to_screaming_snake,
        to_kebab,
        to_screaming_kebab,
    );

    info!("Converting from {:?} to {:?}", from_format, to_format);
    info!("Target path: {}", path.display());
    info!("Recursive: {}, Dry run: {}", recursive, dry_run);

    if let Some(ref exts) = extensions {
        debug!("File extensions: {:?}", exts);
    }
    if !prefix.is_empty() {
        debug!("Prefix: '{}'", prefix);
    }
    if !suffix.is_empty() {
        debug!("Suffix: '{}'", suffix);
    }
    if let Some(ref pattern) = glob {
        debug!("Glob pattern: '{}'", pattern);
    }
    if let Some(ref filter) = word_filter {
        debug!("Word filter: '{}'", filter);
    }

    let spinner = create_spinner("Processing files...");

    let converter = CaseConverter::new(
        from_format,
        to_format,
        extensions,
        recursive,
        dry_run,
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
    )?;

    let result = converter.process_directory(&path);

    spinner.finish_and_clear();

    match result {
        Ok(_) => {
            info!("Conversion completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Conversion failed: {}", e);
            Err(e)
        }
    }
}

#[time("info")]
fn run_clean(
    path: PathBuf,
    recursive: bool,
    dry_run: bool,
    extensions: Option<Vec<String>>,
) -> anyhow::Result<()> {
    info!("Cleaning whitespace from: {}", path.display());
    info!("Recursive: {}, Dry run: {}", recursive, dry_run);

    if let Some(ref exts) = extensions {
        debug!("File extensions: {:?}", exts);
    }

    let mut options = WhitespaceOptions {
        recursive,
        dry_run,
        ..Default::default()
    };
    if let Some(exts) = extensions {
        options.file_extensions = exts;
    }

    let spinner = create_spinner("Cleaning files...");

    let cleaner = WhitespaceCleaner::new(options);
    let (files, lines) = cleaner.process(&path)?;

    spinner.finish_and_clear();

    if files > 0 {
        let prefix = if dry_run { "[DRY-RUN] " } else { "" };
        info!("{}Cleaned {} lines in {} file(s)", prefix, lines, files);
        println!("{}Cleaned {} lines in {} file(s)", prefix, lines, files);
    } else {
        info!("No files needed cleaning");
        println!("No files needed cleaning");
    }

    Ok(())
}

#[time("info")]
fn run_emojis(
    path: PathBuf,
    recursive: bool,
    dry_run: bool,
    extensions: Option<Vec<String>>,
    replace_task: bool,
    remove_other: bool,
) -> anyhow::Result<()> {
    info!("Processing emojis from: {}", path.display());
    info!("Recursive: {}, Dry run: {}", recursive, dry_run);
    info!(
        "Replace task emojis: {}, Remove other emojis: {}",
        replace_task, remove_other
    );

    if let Some(ref exts) = extensions {
        debug!("File extensions: {:?}", exts);
    }

    let mut options = EmojiOptions {
        recursive,
        dry_run,
        replace_task_emojis: replace_task,
        remove_other_emojis: remove_other,
        ..Default::default()
    };
    if let Some(exts) = extensions {
        options.file_extensions = exts;
    }

    let spinner = create_spinner("Transforming emojis...");

    let transformer = EmojiTransformer::new(options);
    let (files, changes) = transformer.process(&path)?;

    spinner.finish_and_clear();

    if files > 0 {
        let prefix = if dry_run { "[DRY-RUN] " } else { "" };
        info!(
            "{}Transformed emojis in {} file(s) ({} changes)",
            prefix, files, changes
        );
        println!(
            "{}Transformed emojis in {} file(s) ({} changes)",
            prefix, files, changes
        );
    } else {
        info!("No files contained emojis to transform");
        println!("No files contained emojis to transform");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[time("info")]
fn run_rename(
    path: PathBuf,
    recursive: bool,
    dry_run: bool,
    include_symlinks: bool,
    to_lowercase: bool,
    to_uppercase: bool,
    to_capitalize: bool,
    underscored: bool,
    hyphenated: bool,
    add_prefix: Option<String>,
    rm_prefix: Option<String>,
    add_suffix: Option<String>,
    rm_suffix: Option<String>,
    replace_prefix: Option<Vec<String>>,
    replace_suffix: Option<Vec<String>>,
    timestamp_long: bool,
    timestamp_short: bool,
) -> anyhow::Result<()> {
    info!("Renaming files in: {}", path.display());
    info!(
        "Recursive: {}, Dry run: {}, Include symlinks: {}",
        recursive, dry_run, include_symlinks
    );

    let mut options = RenameOptions {
        recursive,
        dry_run,
        include_symlinks,
        ..Default::default()
    };

    // Set case transform (only one should be selected)
    if to_lowercase {
        options.case_transform = CaseTransform::Lowercase;
        debug!("Case transform: Lowercase");
    } else if to_uppercase {
        options.case_transform = CaseTransform::Uppercase;
        debug!("Case transform: Uppercase");
    } else if to_capitalize {
        options.case_transform = CaseTransform::Capitalize;
        debug!("Case transform: Capitalize");
    }

    // Set separator replacement (only one should be selected)
    if underscored {
        options.space_replace = SpaceReplace::Underscore;
        debug!("Separator replacement: Underscore");
    } else if hyphenated {
        options.space_replace = SpaceReplace::Hyphen;
        debug!("Separator replacement: Hyphen");
    }

    // Set prefix/suffix options
    options.add_prefix = add_prefix.clone();
    options.remove_prefix = rm_prefix.clone();
    options.add_suffix = add_suffix.clone();
    options.remove_suffix = rm_suffix.clone();

    // Set replace prefix/suffix options
    if let Some(ref args) = replace_prefix {
        if args.len() == 2 {
            options.replace_prefix = Some((args[0].clone(), args[1].clone()));
        }
    }
    if let Some(ref args) = replace_suffix {
        if args.len() == 2 {
            options.replace_suffix = Some((args[0].clone(), args[1].clone()));
        }
    }

    // Set timestamp format (only one should be selected)
    if timestamp_long {
        options.timestamp_format = TimestampFormat::Long;
        debug!("Timestamp format: Long (YYYYMMDD)");
    } else if timestamp_short {
        options.timestamp_format = TimestampFormat::Short;
        debug!("Timestamp format: Short (YYMMDD)");
    }

    if let Some(ref prefix) = add_prefix {
        debug!("Add prefix: '{}'", prefix);
    }
    if let Some(ref prefix) = rm_prefix {
        debug!("Remove prefix: '{}'", prefix);
    }
    if let Some(ref suffix) = add_suffix {
        debug!("Add suffix: '{}'", suffix);
    }
    if let Some(ref suffix) = rm_suffix {
        debug!("Remove suffix: '{}'", suffix);
    }
    if let Some(ref args) = replace_prefix {
        debug!("Replace prefix: '{}' -> '{}'", args[0], args[1]);
    }
    if let Some(ref args) = replace_suffix {
        debug!("Replace suffix: '{}' -> '{}'", args[0], args[1]);
    }

    let spinner = create_spinner("Renaming files...");

    let renamer = FileRenamer::new(options);
    let count = renamer.process(&path)?;

    spinner.finish_and_clear();

    if count > 0 {
        let prefix = if dry_run { "[DRY-RUN] " } else { "" };
        info!("{}Renamed {} file(s)", prefix, count);
        println!("{}Renamed {} file(s)", prefix, count);
    } else {
        info!("No files needed renaming");
        println!("No files needed renaming");
    }

    Ok(())
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

#[allow(clippy::too_many_arguments)]
#[time("info")]
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
) -> anyhow::Result<()> {
    info!("Grouping files by prefix in: {}", path.display());
    info!(
        "Recursive: {}, Dry run: {}, Separator: '{}', Min count: {}",
        recursive, dry_run, separator, min_count
    );
    if from_suffix {
        info!("From suffix: enabled (splitting at last separator)");
    }
    if strip_prefix || from_suffix {
        info!("Strip prefix: enabled");
    }

    let options = GroupOptions {
        recursive,
        dry_run,
        separator,
        min_count,
        // from_suffix implies strip_prefix
        strip_prefix: strip_prefix || from_suffix,
        from_suffix,
    };

    let grouper = FileGrouper::new(options);

    if preview {
        let spinner = create_spinner("Analyzing files...");
        let groups = grouper.preview(&path)?;
        spinner.finish_and_clear();

        if groups.is_empty() {
            println!(
                "No file groups found matching criteria (min_count: {})",
                min_count
            );
        } else {
            println!("Found {} potential group(s):", groups.len());
            for (prefix, files) in &groups {
                println!("\n  {} ({} files):", prefix, files.len());
                for file in files {
                    println!("    - {}", file);
                }
            }
        }
        return Ok(());
    }

    let spinner = create_spinner("Grouping files...");
    let result = grouper.process_with_changes(&path)?;
    spinner.finish_and_clear();

    let stats = &result.stats;

    if stats.files_moved > 0 {
        let prefix_str = if dry_run { "[DRY-RUN] " } else { "" };
        info!(
            "{}Grouping complete: {} directories created, {} files moved",
            prefix_str, stats.dirs_created, stats.files_moved
        );
        println!("{}Grouping complete:", prefix_str);
        if stats.dirs_created > 0 {
            println!("  - Directories created: {}", stats.dirs_created);
        }
        println!("  - Files moved: {}", stats.files_moved);
        if stats.files_renamed > 0 {
            println!(
                "  - Files renamed (prefix stripped): {}",
                stats.files_renamed
            );
        }

        // Write changes.json (even in dry-run mode, for reference)
        if !result.changes.is_empty() {
            let changes_path = std::env::current_dir()?.join("changes.json");
            result.changes.write_to_file(&changes_path)?;
            println!("\nChanges recorded to: {}", changes_path.display());

            // Interactive workflow for reference scanning
            if !dry_run && !no_interactive {
                println!();
                if prompt_yes_no("Would you like to scan for broken references?") {
                    let dirs_to_scan = if let Some(dir) = scope {
                        vec![dir]
                    } else {
                        prompt_scan_dirs(&path)
                    };

                    // Check for scope/target overlap and warn
                    for scan_dir in &dirs_to_scan {
                        if let Some(warning) = check_scope_overlap(scan_dir, &path) {
                            eprintln!("\n{}\n", warning);
                        }
                    }

                    // Scan for broken references
                    let scan_options = ScanOptions {
                        verbose: verbose_scan,
                        ..Default::default()
                    };

                    let spinner = if verbose_scan {
                        eprintln!("Scanning for broken references...");
                        None
                    } else {
                        Some(create_spinner("Scanning for broken references..."))
                    };
                    let scanner =
                        ReferenceScanner::from_change_record(&result.changes, scan_options);
                    let fix_record = scanner.scan(&dirs_to_scan)?;
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }

                    if fix_record.is_empty() {
                        println!("No broken references found.");
                    } else {
                        // Write fixes.json
                        let fixes_path = std::env::current_dir()?.join("fixes.json");
                        fix_record.write_to_file(&fixes_path)?;
                        println!("\nFound {} broken reference(s).", fix_record.len());
                        println!("Proposed fixes written to: {}", fixes_path.display());

                        // Show summary of fixes
                        println!("\nProposed fixes:");
                        for fix in fix_record.fixes.iter().take(10) {
                            println!(
                                "  {}:{}: '{}' -> '{}'",
                                fix.file, fix.line, fix.old_reference, fix.new_reference
                            );
                        }
                        if fix_record.len() > 10 {
                            println!("  ... and {} more (see fixes.json)", fix_record.len() - 10);
                        }

                        println!();
                        if prompt_yes_no("Review fixes.json and apply changes?") {
                            let spinner = create_spinner("Applying fixes...");
                            let apply_result = ReferenceFixer::apply_fixes(&fix_record)?;
                            spinner.finish_and_clear();

                            println!(
                                "\nFixed {} reference(s) in {} file(s).",
                                apply_result.references_fixed, apply_result.files_modified
                            );

                            if !apply_result.errors.is_empty() {
                                println!("\nErrors encountered:");
                                for err in &apply_result.errors {
                                    println!("  - {}", err);
                                }
                            }
                        } else {
                            println!("Fixes not applied. You can review fixes.json and apply them later.");
                        }
                    }
                }
            } else if !dry_run && scope.is_some() {
                // Non-interactive mode with --scope specified
                let dirs_to_scan = vec![scope.unwrap()];

                // Check for scope/target overlap and warn
                for scan_dir in &dirs_to_scan {
                    if let Some(warning) = check_scope_overlap(scan_dir, &path) {
                        eprintln!("\n{}\n", warning);
                    }
                }

                let scan_options = ScanOptions {
                    verbose: verbose_scan,
                    ..Default::default()
                };

                let spinner = if verbose_scan {
                    eprintln!("Scanning for broken references...");
                    None
                } else {
                    Some(create_spinner("Scanning for broken references..."))
                };
                let scanner = ReferenceScanner::from_change_record(&result.changes, scan_options);
                let fix_record = scanner.scan(&dirs_to_scan)?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }

                if fix_record.is_empty() {
                    println!("\nNo broken references found.");
                } else {
                    let fixes_path = std::env::current_dir()?.join("fixes.json");
                    fix_record.write_to_file(&fixes_path)?;
                    println!("\nFound {} broken reference(s).", fix_record.len());
                    println!("Proposed fixes written to: {}", fixes_path.display());
                }
            }
        }
    } else {
        info!("No files needed grouping");
        println!("No files needed grouping");
    }

    Ok(())
}

#[time("info")]
fn run_combined(path: PathBuf, recursive: bool, dry_run: bool) -> anyhow::Result<()> {
    info!("Running combined transformations on: {}", path.display());
    info!("Recursive: {}, Dry run: {}", recursive, dry_run);

    let options = CombinedOptions { recursive, dry_run };

    let spinner = create_spinner("Processing files (rename, emojis, clean)...");

    let processor = CombinedProcessor::new(options);
    let stats = processor.process(&path)?;

    spinner.finish_and_clear();

    let prefix = if dry_run { "[DRY-RUN] " } else { "" };

    // Print summary
    if stats.files_renamed > 0
        || stats.files_emoji_transformed > 0
        || stats.files_whitespace_cleaned > 0
    {
        info!(
            "{}Combined processing complete: {} renamed, {} emoji-transformed ({} changes), {} whitespace-cleaned ({} lines)",
            prefix, stats.files_renamed, stats.files_emoji_transformed, stats.emoji_changes,
            stats.files_whitespace_cleaned, stats.whitespace_lines_cleaned
        );
        println!("{}Processed files:", prefix);
        if stats.files_renamed > 0 {
            println!("  - Renamed: {} file(s)", stats.files_renamed);
        }
        if stats.files_emoji_transformed > 0 {
            println!(
                "  - Emoji transformations: {} file(s) ({} changes)",
                stats.files_emoji_transformed, stats.emoji_changes
            );
        }
        if stats.files_whitespace_cleaned > 0 {
            println!(
                "  - Whitespace cleaned: {} file(s) ({} lines)",
                stats.files_whitespace_cleaned, stats.whitespace_lines_cleaned
            );
        }
    } else {
        info!("No files needed processing");
        println!("No files needed processing");
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if let Err(e) = init_logging(cli.verbose, cli.quiet, cli.log_file.clone()) {
        eprintln!("Warning: Failed to initialize logging: {}", e);
    }

    debug!("CLI arguments parsed successfully");

    let result = match cli.command {
        None => {
            // Default command: run combined processing
            if let Some(path) = cli.path {
                debug!("Running combined processing (default command)");
                run_combined(path, cli.recursive, cli.dry_run)
            } else {
                // Neither command nor path specified - print help
                error!("No command or path specified. Use --help for usage information.");
                std::process::exit(1);
            }
        }

        Some(cmd) => match cmd {
            Commands::Convert {
                from_camel,
                from_pascal,
                from_snake,
                from_screaming_snake,
                from_kebab,
                from_screaming_kebab,
                to_camel,
                to_pascal,
                to_snake,
                to_screaming_snake,
                to_kebab,
                to_screaming_kebab,
                path,
                recursive,
                dry_run,
                extensions,
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
            } => {
                debug!("Running convert subcommand");
                run_convert(
                    from_camel,
                    from_pascal,
                    from_snake,
                    from_screaming_snake,
                    from_kebab,
                    from_screaming_kebab,
                    to_camel,
                    to_pascal,
                    to_snake,
                    to_screaming_snake,
                    to_kebab,
                    to_screaming_kebab,
                    path,
                    recursive,
                    dry_run,
                    extensions,
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
                )
            }

            Commands::Clean {
                path,
                recursive,
                dry_run,
                extensions,
            } => {
                debug!("Running clean subcommand");
                run_clean(path, recursive, dry_run, extensions)
            }

            Commands::Emojis {
                path,
                recursive,
                dry_run,
                extensions,
                replace_task,
                remove_other,
            } => {
                debug!("Running emojis subcommand");
                run_emojis(
                    path,
                    recursive,
                    dry_run,
                    extensions,
                    replace_task,
                    remove_other,
                )
            }

            Commands::RenameFiles {
                path,
                recursive,
                dry_run,
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
            } => {
                debug!("Running rename subcommand");
                run_rename(
                    path,
                    recursive,
                    dry_run,
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
            } => {
                debug!("Running group subcommand");
                run_group(
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
                )
            }
        },
    };

    if let Err(ref e) = result {
        error!("Operation failed: {}", e);
    } else {
        debug!("Operation completed successfully");
    }

    result
}
