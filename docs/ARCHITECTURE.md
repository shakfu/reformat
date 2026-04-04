# reformat Architecture Documentation

## Overview

reformat is a modular code transformation framework implemented in Rust. It provides a library-first design with a command-line interface for applying transformations to source code files. The framework supports case format conversion, whitespace cleaning, emoji transformation, file renaming, file grouping with broken reference detection, line ending normalization, indentation normalization, regex find-and-replace, file header management, and preset-based transformation pipelines.

## Design Principles

1. **Library-First**: Core functionality in `reformat-core`, CLI as thin wrapper in `reformat-cli`
2. **Modularity**: Each transformation is independent and self-contained
3. **Composability**: Combined processor and presets enable multi-step pipelines
4. **Type Safety**: Strong typing with enums and structs ensures compile-time guarantees
5. **Change Tracking**: File operations produce structured JSON records for auditing
6. **Usability**: Simple struct-based API with sensible defaults

## Project Structure

### Workspace Organization

```text
reformat/
├── Cargo.toml                    # Workspace definition (v0.1.6)
├── reformat.json                 # Example preset configuration
├── reformat-core/                # Core library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                # Public API exports
│       ├── case.rs               # CaseFormat enum and word splitting
│       ├── changes.rs            # Change tracking (ChangeRecord, Change)
│       ├── combined.rs           # CombinedProcessor for single-pass operations
│       ├── config.rs             # Preset configuration types
│       ├── converter.rs          # CaseConverter implementation
│       ├── emoji.rs              # EmojiTransformer implementation
│       ├── endings.rs            # EndingsNormalizer implementation
│       ├── group.rs              # FileGrouper implementation
│       ├── header.rs             # HeaderManager implementation
│       ├── indent.rs             # IndentNormalizer implementation
│       ├── refs.rs               # ReferenceScanner and ReferenceFixer
│       ├── rename.rs             # FileRenamer implementation
│       ├── replace.rs            # ContentReplacer implementation
│       └── whitespace.rs         # WhitespaceCleaner implementation
│
├── reformat-cli/                 # CLI binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # Clap-based CLI with subcommands
│       └── config.rs             # Config file loading
│
├── reformat-plugins/             # Plugin system (foundation only)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                # Plugin API placeholder
│
└── tests/                        # Integration tests
    ├── cli_integration.rs        # CLI functionality tests
    └── library_integration.rs    # Library API tests
```

### Workspace Cargo.toml

```toml
[workspace]
members = [
    "reformat-core",
    "reformat-cli",
    "reformat-plugins",
]
resolver = "2"

[workspace.package]
version = "0.1.6"
edition = "2021"

[workspace.dependencies]
regex = "1.11"
aho-corasick = "1.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"
walkdir = "2.5"
glob = "0.3"
chrono = { version = "0.4", features = ["serde"] }
log = "0.4"
simplelog = "0.12"
indicatif = "0.17"
logging_timer = "1.1"
```

## Core Components

### 1. CaseFormat Enum (`case.rs`)

Defines all supported case formats and provides word splitting/joining logic.

```rust
pub enum CaseFormat {
    CamelCase,              // camelCase
    PascalCase,             // PascalCase
    SnakeCase,              // snake_case
    ScreamingSnakeCase,     // SCREAMING_SNAKE_CASE
    KebabCase,              // kebab-case
    ScreamingKebabCase,     // SCREAMING-KEBAB-CASE
}
```

**Key Methods:**

- `pattern(&self) -> &str` - Returns regex pattern for identifying format
- `split_words(&self, text: &str) -> Vec<String>` - Splits identifier into words
- `join_words(&self, words: &[String], prefix: &str, suffix: &str) -> String` - Reassembles words

**Implementation Details:**

- Manual character-by-character iteration for camelCase/PascalCase splitting (Rust regex doesn't support lookahead/lookbehind)
- Regex-based splitting for snake_case, kebab-case variants
- All words normalized to lowercase during splitting

### 2. CaseConverter (`converter.rs`)

Main transformer for case format conversion in files.

```rust
pub struct CaseConverter {
    from_format: CaseFormat,
    to_format: CaseFormat,
    file_extensions: Vec<String>,
    recursive: bool,
    dry_run: bool,
    prefix: String,
    suffix: String,
    strip_prefix: Option<String>,
    strip_suffix: Option<String>,
    replace_prefix_from: Option<String>,
    replace_prefix_to: Option<String>,
    replace_suffix_from: Option<String>,
    replace_suffix_to: Option<String>,
    glob_pattern: Option<glob::Pattern>,
    word_filter: Option<Regex>,
    source_pattern: Regex,
}
```

**Transformation Pipeline:**

1. Strip prefix/suffix (if specified)
2. Replace prefix/suffix (if specified)
3. Apply word filter
4. Case conversion
5. Add prefix/suffix

**Key Methods:**

- `new(...)` - Creates converter with all options
- `convert(&self, text: &str) -> String` - Converts single string
- `process_file(&self, path: &Path) -> Result<bool>` - Processes single file
- `process_directory(&self, path: &Path) -> Result<()>` - Processes directory
- `matches_glob(&self, path: &Path, base: &Path) -> bool` - Checks glob patterns

### 3. WhitespaceCleaner (`whitespace.rs`)

Removes trailing whitespace while preserving line endings.

```rust
pub struct WhitespaceCleaner {
    options: WhitespaceOptions,
}

pub struct WhitespaceOptions {
    pub extensions: Vec<String>,
    pub recursive: bool,
    pub dry_run: bool,
}
```

**Key Methods:**

- `clean_file(&self, path: &Path) -> Result<usize>` - Returns lines cleaned
- `process(&self, path: &Path) -> Result<(usize, usize)>` - Returns (files, lines) cleaned
- `should_process(&self, path: &Path) -> bool` - Extension and path filtering

### 4. EmojiTransformer (`emoji.rs`)

Transforms emojis to text alternatives and removes non-task emojis.

```rust
pub struct EmojiTransformer {
    options: EmojiOptions,
}

pub struct EmojiOptions {
    pub extensions: Vec<String>,
    pub recursive: bool,
    pub dry_run: bool,
    pub replace_task: bool,
    pub remove_other: bool,
}
```

**Key Methods:**

- `transform_file(&self, path: &Path) -> Result<usize>` - Returns emoji changes count
- `process(&self, path: &Path) -> Result<(usize, usize)>` - Returns (files, changes)

### 5. FileRenamer (`rename.rs`)

Renames files with case transformations, separator replacements, and timestamp prefixes.

```rust
pub struct FileRenamer {
    options: RenameOptions,
}

pub struct RenameOptions {
    pub case_transform: CaseTransform,
    pub space_replace: SpaceReplace,
    pub add_prefix: Option<String>,
    pub remove_prefix: Option<String>,
    pub add_suffix: Option<String>,
    pub remove_suffix: Option<String>,
    pub replace_prefix: Option<(String, String)>,
    pub replace_suffix: Option<(String, String)>,
    pub timestamp_format: TimestampFormat,
    pub recursive: bool,
    pub dry_run: bool,
    pub include_symlinks: bool,
}

pub enum CaseTransform { None, Lowercase, Uppercase, Capitalize }
pub enum SpaceReplace { None, Underscore, Hyphen }
pub enum TimestampFormat { None, Long, Short }  // YYYYMMDD or YYMMDD
```

**Key Methods:**

- `rename_file(&self, path: &Path, is_symlink: bool) -> Result<bool>` - Renames single file
- `process(&self, path: &Path) -> Result<usize>` - Returns files renamed

### 6. FileGrouper (`group.rs`)

Organizes files by common prefix into subdirectories with change tracking.

```rust
pub struct FileGrouper {
    options: GroupOptions,
}

pub struct GroupOptions {
    pub separator: char,       // Default: '_'
    pub min_count: usize,      // Default: 2
    pub strip_prefix: bool,    // Remove prefix from filenames after moving
    pub from_suffix: bool,     // Split at LAST separator instead of first
    pub recursive: bool,
    pub dry_run: bool,
}

pub struct GroupStats {
    pub dirs_created: usize,
    pub files_moved: usize,
    pub files_renamed: usize,
}

pub struct GroupResult {
    pub stats: GroupStats,
    pub changes: ChangeRecord,
}
```

**Key Methods:**

- `process(&self, path: &Path) -> Result<GroupStats>` - Process directory
- `process_with_changes(&self, path: &Path) -> Result<GroupResult>` - Process with change tracking
- `preview(&self, path: &Path) -> Result<HashMap<String, Vec<String>>>` - Preview groups

### 7. Change Tracking (`changes.rs`)

Structured records of file operations for auditing and reference fixing.

```rust
pub enum Change {
    DirectoryCreated { path: String },
    FileMoved { from: String, to: String },
    FileRenamed { from: String, to: String, directory: String },
}

pub struct ChangeRecord {
    pub operation: String,
    pub timestamp: String,
    pub base_dir: String,
    pub options: Option<serde_json::Value>,
    pub changes: Vec<Change>,
}
```

**Key Methods:**

- `write_to_file(&self, path: &Path) -> Result<()>` - Serialize to JSON
- `read_from_file(path: &Path) -> Result<Self>` - Deserialize from JSON
- `file_moves(&self) -> Vec<(&str, &str)>` - Extract file move pairs

### 8. Reference Scanner & Fixer (`refs.rs`)

Scans codebases for broken references after file operations and proposes fixes.

```rust
pub struct ReferenceScanner { /* ... */ }
pub struct ReferenceFixer { /* ... */ }

pub struct ScanOptions {
    pub extensions: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub recursive: bool,
    pub verbose: bool,
}

pub struct ReferenceFix {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub context: String,
    pub old_reference: String,
    pub new_reference: String,
}

pub struct FixRecord {
    pub generated_from: String,
    pub timestamp: String,
    pub scan_directories: Vec<String>,
    pub fixes: Vec<ReferenceFix>,
}
```

**Key Methods on ReferenceScanner:**

- `from_change_record(record: &ChangeRecord, options: ScanOptions) -> Self`
- `new(file_moves: HashMap<String, String>, options: ScanOptions) -> Self`

### 9. EndingsNormalizer (`endings.rs`)

Normalizes line endings across files (LF, CRLF, CR).

```rust
pub struct EndingsNormalizer {
    options: EndingsOptions,
}

pub struct EndingsOptions {
    pub style: LineEnding,           // Lf, Crlf, or Cr
    pub file_extensions: Vec<String>,
    pub recursive: bool,
    pub dry_run: bool,
}

pub enum LineEnding { Lf, Crlf, Cr }
```

**Key Methods:**

- `normalize_file(&self, path: &Path) -> Result<usize>` - Returns endings changed
- `process(&self, path: &Path) -> Result<(usize, usize)>` - Returns (files, endings) changed

**Implementation Details:**

- Byte-level parsing to correctly distinguish CR, LF, and CRLF sequences
- Binary file detection via null-byte scanning (skips binary files)
- Processes files as raw bytes to avoid lossy string conversion of line endings

### 10. IndentNormalizer (`indent.rs`)

Converts between tabs and spaces with configurable width.

```rust
pub struct IndentNormalizer {
    options: IndentOptions,
}

pub struct IndentOptions {
    pub style: IndentStyle,          // Spaces or Tabs
    pub width: usize,                // Spaces per indent level (default: 4)
    pub file_extensions: Vec<String>,
    pub recursive: bool,
    pub dry_run: bool,
}

pub enum IndentStyle { Spaces, Tabs }
```

**Key Methods:**

- `normalize_file(&self, path: &Path) -> Result<usize>` - Returns lines changed
- `process(&self, path: &Path) -> Result<(usize, usize)>` - Returns (files, lines) changed

**Implementation Details:**

- Only modifies leading whitespace on each line; content after indentation is untouched
- Tab-stop-aware conversion: tabs align to the next multiple of `width`
- Handles mixed indentation (tabs + spaces on the same line)
- Partial tab stops preserved when converting spaces to tabs (e.g., 6 spaces with width 4 becomes 1 tab + 2 spaces)

### 11. ContentReplacer (`replace.rs`)

Applies regex find-and-replace patterns across files.

```rust
pub struct ContentReplacer {
    options: ReplaceOptions,
    compiled: Vec<CompiledPattern>,  // Pre-compiled regexes
}

pub struct ReplaceOptions {
    pub patterns: Vec<ReplacePattern>,
    pub file_extensions: Vec<String>,
    pub recursive: bool,
    pub dry_run: bool,
}

pub struct ReplacePattern {
    pub find: String,     // Regex pattern
    pub replace: String,  // Replacement (supports $1, $2 capture groups)
}
```

**Key Methods:**

- `new(options: ReplaceOptions) -> Result<Self>` - Compiles all regex patterns; errors on invalid regex
- `replace_file(&self, path: &Path) -> Result<usize>` - Returns replacement count
- `process(&self, path: &Path) -> Result<(usize, usize)>` - Returns (files, replacements)

**Implementation Details:**

- Patterns are applied sequentially (output of pattern N is input to pattern N+1)
- Regex compilation happens once at construction time
- Capture group references ($1, $2) supported in replacement strings

### 12. HeaderManager (`header.rs`)

Inserts or updates file headers (license, copyright, etc.).

```rust
pub struct HeaderManager {
    options: HeaderOptions,
    resolved_header: String,          // Header with {year} resolved
    header_detector: Option<Regex>,   // Detects existing headers (year-flexible)
}

pub struct HeaderOptions {
    pub text: String,                 // Header text (may contain {year})
    pub update_year: bool,            // Replace {year} with current year
    pub file_extensions: Vec<String>,
    pub recursive: bool,
    pub dry_run: bool,
}
```

**Key Methods:**

- `new(options: HeaderOptions) -> Result<Self>` - Resolves year template and builds detector regex
- `process_file(&self, path: &Path) -> Result<bool>` - Returns true if file was modified
- `process(&self, path: &Path) -> Result<(usize, usize)>` - Returns (files_changed, ops)

**Implementation Details:**

- Year-flexible detection: existing headers with any 4-digit year (19xx/20xx) are recognized and updated
- Shebang preservation: `#!` lines at the start of files are kept above the header
- Three modes: insert (header missing), update (header present but year differs), skip (exact match)

### 13. Configuration & Presets (`config.rs`)

Defines reusable transformation pipelines via `reformat.json`.

```rust
pub type ReformatConfig = HashMap<String, Preset>;

pub struct Preset {
    pub steps: Vec<String>,       // e.g., ["rename", "emojis", "clean"]
    pub rename: Option<RenameConfig>,
    pub emojis: Option<EmojiConfig>,
    pub clean: Option<CleanConfig>,
    pub convert: Option<ConvertConfig>,
    pub group: Option<GroupConfig>,
    pub endings: Option<EndingsConfig>,
    pub indent: Option<IndentConfig>,
    pub replace: Option<ReplaceConfig>,
    pub header: Option<HeaderConfig>,
}
```

**Valid steps:** `rename`, `emojis`, `clean`, `convert`, `group`, `endings`, `indent`, `replace`, `header`

Each step has a corresponding config struct with optional overrides. Steps without explicit configuration use sensible defaults.

### 14. CombinedProcessor (`combined.rs`)

Efficient single-pass processing applying multiple transformations.

```rust
pub struct CombinedProcessor {
    options: CombinedOptions,
    // ...
}

pub struct CombinedOptions {
    pub recursive: bool,
    pub dry_run: bool,
}

pub struct CombinedStats {
    pub files_renamed: usize,
    pub files_emoji_transformed: usize,
    pub emoji_changes: usize,
    pub files_whitespace_cleaned: usize,
    pub whitespace_lines_cleaned: usize,
}
```

**Default Pipeline:**

1. Rename files to lowercase
2. Transform emojis (task emoji replacement + removal)
3. Clean whitespace

**Key Methods:**

- `new(options: CombinedOptions) -> Self` - Creates processor with pipeline
- `with_defaults() -> Self` - Creates with default options
- `process(&self, path: &Path) -> Result<CombinedStats>` - Processes path

## CLI Architecture

### CLI Binary (reformat-cli)

Built with `clap` derive macros using subcommand architecture.

**Global Options:**

- `-v, --verbose` - Multi-level verbosity (`-v`, `-vv`, `-vvv`)
- `-q, --quiet` - Quiet mode (errors only)
- `--log-file <PATH>` - Write logs to file
- `-p, --preset <NAME>` - Run a named preset from `reformat.json`
- `-j, --job <FILE|->` - Run an ad-hoc job from a JSON file or stdin (mutually exclusive with `--preset`)

### Commands

#### Default Command (no subcommand)

Runs all transformations in a single pass:

```bash
reformat <path>         # Process path
reformat -r <path>      # Process recursively
reformat -d <path>      # Dry run
```

#### `convert` - Case format conversion

```bash
reformat convert --from-camel --to-snake src/
```

#### `clean` - Whitespace cleaning

```bash
reformat clean src/
```

#### `emojis` - Emoji transformation

```bash
reformat emojis docs/
```

#### `rename_files` - File renaming

```bash
reformat rename_files --to-lowercase src/
reformat rename_files --timestamp-long src/   # Add YYYYMMDD prefix
```

#### `group` - File grouping by prefix

```bash
reformat group --strip-prefix templates/
reformat group --from-suffix templates/       # Split at LAST separator
reformat group --preview templates/           # Preview only
reformat group --strip-prefix --scope src templates/  # Scan for broken refs
```

#### `endings` - Line ending normalization

```bash
reformat endings src/                    # Normalize to LF (default)
reformat endings --style crlf src/       # Normalize to CRLF
```

#### `indent` - Indentation normalization

```bash
reformat indent src/                     # Tabs to 4-space (default)
reformat indent --style tabs src/        # Spaces to tabs
reformat indent --width 2 src/           # Tabs to 2-space
```

#### `replace` - Regex find-and-replace

```bash
reformat replace --find "old" --replace-with "new" src/
reformat replace --find "func\((\w+)\)" --replace-with "call(\$1)" src/
```

#### `header` - File header management

```bash
reformat header -t "// Copyright 2025 MyOrg" src/
reformat header -t "// Copyright {year} MyOrg" --update-year src/
```

### Pipeline Execution Model

The CLI has two ways to run multi-step transformation pipelines:

- **Presets** (`-p <name>`) -- Named pipelines stored in `reformat.json`. Reusable and version-controlled.
- **Jobs** (`--job <file|->`) -- Ad-hoc pipelines loaded from an arbitrary JSON file or stdin. Throwaway.

Both share the same execution engine (`run_pipeline`), which validates steps and executes them in order. The `Preset` struct is the common pipeline format:

```text
run_preset(name)  -->  loads Preset from reformat.json  -->  run_pipeline(name, preset, path, dry_run)
run_job(source)   -->  loads Preset from file or stdin   -->  run_pipeline(label, preset, path, dry_run)
```

The `--preset` and `--job` flags are mutually exclusive (enforced by clap's `conflicts_with`).

## Public API

All types are exported from `reformat-core`:

```rust
pub use case::CaseFormat;
pub use changes::{Change, ChangeRecord};
pub use combined::{CombinedOptions, CombinedProcessor, CombinedStats};
pub use config::{Preset, ReformatConfig};
pub use converter::CaseConverter;
pub use emoji::{EmojiOptions, EmojiTransformer};
pub use endings::{EndingsNormalizer, EndingsOptions, LineEnding};
pub use group::{FileGrouper, GroupOptions, GroupResult, GroupStats};
pub use header::{HeaderManager, HeaderOptions};
pub use indent::{IndentNormalizer, IndentOptions, IndentStyle};
pub use refs::{ApplyResult, FixRecord, ReferenceFix, ReferenceFixer, ReferenceScanner, ScanOptions};
pub use rename::{CaseTransform, FileRenamer, RenameOptions, SpaceReplace, TimestampFormat};
pub use replace::{ContentReplacer, ReplaceOptions, ReplacePattern, ReplacePatternConfig};
pub use whitespace::{WhitespaceCleaner, WhitespaceOptions};

pub type Result<T> = anyhow::Result<T>;
```

## Testing

### Test Organization

| Location | Type | Count |
|----------|------|-------|
| reformat-cli unit tests | CLI parsing | 6 |
| reformat-cli integration tests | CLI end-to-end | 51 |
| reformat-core unit tests | Module-level | 132 |
| reformat-core integration tests | Library API | 11 |
| Doc tests | Compilation | 1 |
| **Total** | | **201** |

### Test Strategy

- **Unit tests** cover format patterns, word splitting, transformation pipelines, edge cases, change tracking, config parsing, reference scanning, line ending normalization, indentation conversion, regex replacement, header management
- **Integration tests** cover file processing with temp directories, directory traversal, dry-run mode, extension filtering, grouping operations, preset execution
- **CLI tests** cover command parsing, argument validation, output format, help/version, exit codes

## Dependencies

### reformat-core

- `regex` + `aho-corasick` - Pattern matching
- `walkdir` - Directory traversal
- `glob` - File pattern matching
- `anyhow` + `thiserror` - Error handling
- `serde` + `serde_json` - Serialization (change records, config)
- `chrono` - Timestamps
- `log` - Logging facade
- `rayon` (optional) - Parallel processing

### reformat-cli

- `reformat-core` - Core library
- `reformat-plugins` - Plugin system
- `clap` - CLI argument parsing
- `simplelog` - Logging implementation
- `indicatif` - Progress indicators
- `logging_timer` - Operation timing

## Build and Release

```bash
cargo build --workspace          # Build all
cargo build --release -p reformat  # Release binary
cargo test --workspace           # Run all 201 tests
cargo install --path reformat-cli  # Install binary
cargo doc --workspace --open     # Generate docs
```

### Release Profile

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```
