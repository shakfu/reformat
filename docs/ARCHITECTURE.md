# reformat Architecture Documentation

## Overview

reformat is a modular code transformation framework implemented in Rust. It provides a library-first design with a command-line interface for applying transformations to source code files. The framework supports case format conversion, whitespace cleaning, emoji transformation, and file renaming operations.

## Design Principles

1. **Library-First**: Core functionality in `reformat-core`, CLI as thin wrapper in `reformat-cli`
2. **Modularity**: Each transformation is independent and self-contained
3. **Composability**: Combined processor enables efficient single-pass operations
4. **Type Safety**: Strong typing with enums and structs ensures compile-time guarantees
5. **Performance**: Single-pass processing and efficient regex-based matching
6. **Usability**: Simple struct-based API with sensible defaults

## Current Project Structure

### Workspace Organization

The project is organized as a Cargo workspace:

```
reformat/
├── Cargo.toml                 # Workspace definition
├── reformat-core/                # Core library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs             # Public API exports
│       ├── case.rs            # CaseFormat enum and word splitting
│       ├── converter.rs       # CaseConverter implementation
│       ├── whitespace.rs      # WhitespaceCleaner implementation
│       ├── emoji.rs           # EmojiTransformer implementation
│       ├── rename.rs          # FileRenamer implementation
│       └── combined.rs        # CombinedProcessor for single-pass operations
│
├── reformat-cli/                 # CLI binary
│   ├── Cargo.toml
│   └── src/
│       └── main.rs            # Clap-based CLI with subcommands
│
├── reformat-plugins/             # Plugin system (foundation only)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs             # Plugin API placeholder
│
└── tests/                     # Integration tests
    ├── cli_integration.rs     # CLI functionality tests
    └── library_integration.rs # Library API tests
```

### Workspace Cargo.toml

```toml
[workspace]
members = [
    "reformat-core",
    "reformat-cli",
    "reformat-plugins",
]

[workspace.package]
version = "0.2.2"
edition = "2021"

[workspace.dependencies]
regex = "1.11"
anyhow = "1.0"
walkdir = "2.5"
glob = "0.3"
log = "0.4"
simplelog = "0.12"
indicatif = "0.17"
```

### Core Library (reformat-core)

The core library provides the fundamental transformation capabilities:

**Dependencies:**
- `regex` - Pattern matching for case formats
- `walkdir` - Directory traversal
- `glob` - File pattern matching
- `anyhow` - Error handling
- `rayon` (optional) - Parallel processing support

**Features:**
- `default = ["parallel"]` - Default feature set
- `parallel` - Enable parallel processing (currently not utilized)

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

**Default Extensions:** `.c`, `.h`, `.py`, `.md`, `.js`, `.ts`, `.java`, `.cpp`, `.hpp`

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

**Features:**
- Preserves CRLF/LF line endings
- Skips hidden files and build directories (`.git`, `node_modules`, `target`, etc.)
- Configurable file extension filtering

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
    pub replace_task: bool,    // Default: true
    pub remove_other: bool,    // Default: true
}
```

**Emoji Mappings:**
- ✅ → `[x]` (checkmark)
- ☐ → `[ ]` (empty box)
- ☑ → `[x]` (checked box)
- ✓ → `[x]` (check mark)
- ☒ → `[X]` (crossed box)
- ❌ → `[X]` (cross mark)
- ⚠ → `[!]` (warning)
- 🟡 → `[yellow]` (status indicator)
- 🟢 → `[green]` (status indicator)
- 🔴 → `[red]` (status indicator)
- ⭐ → `[+]` (star)
- 📝 → `[note]` (memo)
- 📋 → `[list]` (clipboard)

**Key Methods:**
- `transform_file(&self, path: &Path) -> Result<usize>` - Returns emoji changes count
- `process(&self, path: &Path) -> Result<(usize, usize)>` - Returns (files, changes)
- `replace_task_emoji(&self, content: &str) -> String` - Task emoji mapping

### 5. FileRenamer (`rename.rs`)

Renames files with case transformations and separator replacements.

```rust
pub struct FileRenamer {
    options: RenameOptions,
}

pub struct RenameOptions {
    pub case_transform: CaseTransform,
    pub space_replace: SpaceReplace,
    pub add_prefix: Option<String>,
    pub rm_prefix: Option<String>,
    pub add_suffix: Option<String>,
    pub rm_suffix: Option<String>,
    pub recursive: bool,
    pub dry_run: bool,
}

pub enum CaseTransform {
    None,
    Lowercase,
    Uppercase,
    Capitalize,
}

pub enum SpaceReplace {
    None,
    Underscore,
    Hyphen,
}
```

**Transformation Pipeline:**
1. Remove prefix (--rm-prefix)
2. Remove suffix (--rm-suffix)
3. Replace separators (--underscored or --hyphenated)
4. Case transformation (--to-lowercase, --to-uppercase, --to-capitalize)
5. Add prefix (--add-prefix)
6. Add suffix (--add-suffix)

**Key Methods:**
- `rename_file(&self, path: &Path) -> Result<bool>` - Renames single file
- `process(&self, path: &Path) -> Result<usize>` - Returns files renamed

### 6. CombinedProcessor (`combined.rs`)

Efficient single-pass processing applying multiple transformations.

```rust
pub struct CombinedProcessor {
    options: CombinedOptions,
    rename_options: RenameOptions,
    emoji_options: EmojiOptions,
    whitespace_options: WhitespaceOptions,
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

**Benefits:**
- **3x faster** than running individual commands
- Single directory traversal
- Automatic path tracking after renames
- Comprehensive statistics

**Key Methods:**
- `new(options: CombinedOptions) -> Self` - Creates processor with pipeline
- `with_defaults() -> Self` - Creates with default options
- `process(&self, path: &Path) -> Result<CombinedStats>` - Processes path

## CLI Architecture

### CLI Binary (reformat-cli)

Built with `clap` derive macros using subcommand architecture.

**Global Flags:**
- `-v, --verbose` - Multi-level verbosity (can be repeated: `-v`, `-vv`, `-vvv`)
- `-q, --quiet` - Quiet mode (errors only)
- `--log-file <PATH>` - Write logs to file

**Logging Levels:**
- No flag: WARN (minimal output)
- `-v`: INFO (progress and completion)
- `-vv`: DEBUG (detailed operations)
- `-vvv`: TRACE (maximum verbosity)

**UI Features:**
- Progress spinners with `indicatif`
- Automatic operation timing with `logging_timer`
- Color-coded output with `simplelog`
- Structured timestamps

### Commands

#### Default Command

Runs all transformations in a single pass (no subcommand required).

```bash
reformat <path>         # Process path
reformat -r <path>      # Process recursively
reformat -d <path>      # Dry run
```

**Pipeline:**
1. Rename to lowercase
2. Transform emojis
3. Clean whitespace

#### Subcommands

**1. `convert` - Case format conversion**

```bash
reformat convert --from-camel --to-snake src/
```

Options:
- `--from-<format>`, `--to-<format>` - Case format selection
- `-r, --recursive` - Process directories recursively
- `-d, --dry-run` - Preview changes
- `-e, --extensions` - File extension filter
- `--glob <PATTERN>` - File pattern filter
- `--word-filter <REGEX>` - Word-level filter
- `--prefix`, `--suffix` - Add prefix/suffix to converted identifiers
- `--strip-prefix`, `--strip-suffix` - Remove prefix/suffix before conversion
- `--replace-prefix-from`, `--replace-prefix-to` - Replace prefix
- `--replace-suffix-from`, `--replace-suffix-to` - Replace suffix

**2. `clean` - Whitespace cleaning**

```bash
reformat clean src/
```

Options:
- `-r, --recursive` - Process recursively (default: true)
- `-d, --dry-run` - Preview changes
- `-e, --extensions` - File extension filter

**3. `emojis` - Emoji transformation**

```bash
reformat emojis docs/
```

Options:
- `-r, --recursive` - Process recursively (default: true)
- `-d, --dry-run` - Preview changes
- `-e, --extensions` - File extension filter
- `--replace-task` - Replace task emojis (default: true)
- `--remove-other` - Remove non-task emojis (default: true)

**4. `rename_files` - File renaming**

```bash
reformat rename_files --to-lowercase src/
```

Options:
- `-r, --recursive` - Process recursively (default: true)
- `-d, --dry-run` - Preview changes
- `--to-lowercase`, `--to-uppercase`, `--to-capitalize` - Case transformations
- `--underscored`, `--hyphenated` - Separator replacements
- `--add-prefix`, `--rm-prefix` - Prefix operations
- `--add-suffix`, `--rm-suffix` - Suffix operations

## Library Usage

### Public API

All transformers are exported from `reformat-core`:

```rust
pub use case::CaseFormat;
pub use converter::CaseConverter;
pub use whitespace::{WhitespaceCleaner, WhitespaceOptions};
pub use emoji::{EmojiTransformer, EmojiOptions};
pub use rename::{FileRenamer, RenameOptions, CaseTransform, SpaceReplace};
pub use combined::{CombinedProcessor, CombinedOptions, CombinedStats};
```

### Usage Examples

**Case Conversion:**
```rust
use reformat_core::{CaseConverter, CaseFormat};

let converter = CaseConverter::new(
    CaseFormat::CamelCase,
    CaseFormat::SnakeCase,
    None, false, false,
    String::new(), String::new(),
    None, None, None, None, None, None,
    None, None
)?;

converter.process_directory(Path::new("src"))?;
```

**Whitespace Cleaning:**
```rust
use reformat_core::{WhitespaceCleaner, WhitespaceOptions};

let mut options = WhitespaceOptions::default();
options.recursive = true;

let cleaner = WhitespaceCleaner::new(options);
let (files, lines) = cleaner.process(Path::new("src"))?;
```

**Combined Processing:**
```rust
use reformat_core::{CombinedProcessor, CombinedOptions};

let processor = CombinedProcessor::with_defaults();
let stats = processor.process(Path::new("src"))?;

println!("Renamed: {}", stats.files_renamed);
println!("Emojis: {} files, {} changes",
         stats.files_emoji_transformed,
         stats.emoji_changes);
println!("Whitespace: {} files, {} lines",
         stats.files_whitespace_cleaned,
         stats.whitespace_lines_cleaned);
```

## Testing Architecture

### Test Organization

```
reformat-core/
├── src/
│   ├── case.rs          # 5 unit tests
│   ├── converter.rs     # 7 unit tests
│   ├── whitespace.rs    # 6 unit tests
│   ├── emoji.rs         # 10 unit tests
│   ├── rename.rs        # 11 unit tests
│   └── combined.rs      # 5 unit tests
└── tests/
    └── library_integration.rs  # 11 integration tests

reformat-cli/
└── tests/
    └── cli_integration.rs      # 34 CLI tests
```

**Total: 89 tests**
- 44 unit tests (in module files)
- 11 library integration tests
- 34 CLI integration tests

### Test Strategy

**Unit Tests:**
- Format pattern matching accuracy
- Word splitting and joining logic
- Prefix/suffix transformations
- Identifier transformation pipeline
- Edge cases (empty strings, special characters)

**Integration Tests:**
- File processing with temp directories
- Directory traversal (recursive and non-recursive)
- Dry-run mode validation
- Extension filtering
- Error handling

**CLI Tests:**
- Command parsing
- Argument validation
- Output format verification
- Help and version commands
- Exit codes

## Implementation Details

### Regex Patterns

Each case format has a precise regex pattern:

```rust
CaseFormat::CamelCase => r"\b[a-z]+(?:[A-Z][a-z0-9]*)+\b"
CaseFormat::PascalCase => r"\b[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]*)+\b"
CaseFormat::SnakeCase => r"\b[a-z]+(?:_[a-z0-9]+)+\b"
CaseFormat::ScreamingSnakeCase => r"\b[A-Z]+(?:_[A-Z0-9]+)+\b"
CaseFormat::KebabCase => r"\b[a-z]+(?:-[a-z0-9]+)+\b"
CaseFormat::ScreamingKebabCase => r"\b[A-Z]+(?:-[A-Z0-9]+)+\b"
```

**Note:** Patterns require at least 2 segments to avoid false positives (e.g., `MyClass` matches PascalCase, but `My` doesn't).

### Word Splitting Strategy

**camelCase/PascalCase:**
- Manual character iteration (Rust regex lacks lookahead/lookbehind)
- Split on uppercase boundaries
- Normalize all words to lowercase

**snake_case/kebab-case variants:**
- Regex-based splitting on `_` or `-`
- Direct split using standard library methods

### File Processing

**Directory Traversal:**
- Uses `walkdir` crate for recursive traversal
- Single-level traversal uses `std::fs::read_dir`
- Files sorted by depth (deepest first) for rename operations

**File Filtering:**
- Extension matching (case-insensitive)
- Glob pattern matching (filename and relative path)
- Hidden file and build directory skipping

**Content Processing:**
- Read entire file into memory
- Apply regex replacements
- Write back only if modified
- Preserve file metadata

### Error Handling

Uses `anyhow::Result<T>` for flexible error propagation:

```rust
pub type Result<T> = anyhow::Result<T>;
```

**Error Categories:**
- Regex compilation errors
- File I/O errors
- Path manipulation errors
- Glob pattern errors

## Performance Characteristics

### Optimization Strategies

1. **Single-Pass Processing** - `CombinedProcessor` traverses directory once
2. **Lazy File Writing** - Only write files that were modified
3. **Efficient Pattern Matching** - Compiled regex patterns reused
4. **Minimal Memory Overhead** - Stream processing where possible

### Performance Metrics

Typical operation times (small-to-medium projects):
- Case conversion: 4-10ms for 50-100 files
- Whitespace cleaning: 2-5ms for 50-100 files
- Emoji transformation: 3-8ms for 50-100 files
- Combined processing: ~3x faster than separate operations

**Example:**
```
run_convert(), Elapsed=4.089125ms
```

## Extension Points

### Adding New Transformers

1. Create new module in `reformat-core/src/`
2. Define struct with options
3. Implement transformation logic
4. Add tests
5. Export from `lib.rs`
6. Add CLI command in `reformat-cli/src/main.rs`

**Example Structure:**
```rust
// reformat-core/src/my_transformer.rs
pub struct MyTransformer {
    options: MyOptions,
}

pub struct MyOptions {
    pub recursive: bool,
    pub dry_run: bool,
}

impl MyTransformer {
    pub fn new(options: MyOptions) -> Self {
        MyTransformer { options }
    }

    pub fn transform_file(&self, path: &Path) -> Result<usize> {
        // Implementation
        Ok(changes)
    }

    pub fn process(&self, path: &Path) -> Result<(usize, usize)> {
        // Directory traversal and file processing
        Ok((files_processed, total_changes))
    }
}
```

### Extending CombinedProcessor

To add new transformations to the default pipeline:

1. Add transformer to `CombinedProcessor` struct
2. Update `CombinedStats` with new metrics
3. Add transformation step in `process_single_file()`
4. Update tests

## Future Vision

The following features are planned but not yet implemented:

### Planned Architecture Enhancements

**1. Trait-Based Architecture**
- Abstract `Transformer` trait for polymorphism
- `Filter` trait for file set refinement
- `Analyzer` trait for code metrics
- Pipeline builder pattern for composition

**2. Advanced Features**
- AST-based transformations using Tree-sitter
- Language-specific semantic transformations
- Plugin system with dynamic loading
- YAML-based configuration files
- Interactive CLI mode

**3. Performance Improvements**
- Parallel file processing with rayon
- AST caching for repeated operations
- Incremental processing (skip unchanged files)

**4. Developer Tools**
- Transaction/rollback system
- Git integration (auto-commit, branch creation)
- Transformation preview with diffs
- Complexity and metrics analysis

### Migration Strategy

The current simple struct-based design can evolve to trait-based architecture without breaking changes:

1. Define core traits alongside existing structs
2. Implement traits for existing transformers
3. Add trait-based pipeline builder
4. Maintain existing API for backwards compatibility
5. Document migration path for users

## Build and Release

### Build Commands

```bash
# Build entire workspace
cargo build --workspace

# Build specific crates
cargo build -p reformat-core     # Library only
cargo build -p reformat          # CLI binary

# Release build
cargo build --release -p reformat

# Run tests
cargo test --workspace        # All tests
cargo test -p reformat-core      # Core tests only
cargo test -p reformat           # CLI tests only
```

### Release Profile

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

### Installation

```bash
# Install from workspace
cargo install --path reformat-cli

# Binary location
./target/release/reformat
```

## Documentation

### Documentation Structure

```
docs/
├── README.md              # User guide and quick start
├── ARCHITECTURE.md        # This file - architecture documentation
├── CLAUDE.md              # Project context for Claude Code
├── CHANGELOG.md           # Version history and changes
└── LICENSE                # License information
```

### Inline Documentation

All public APIs have rustdoc comments:
- Module-level documentation in each `.rs` file
- Struct and enum documentation
- Method documentation with examples
- Example usage in integration tests

Generate documentation:
```bash
cargo doc --workspace --open
```

## References

### Dependencies

- [regex](https://docs.rs/regex/) - Regular expressions
- [walkdir](https://docs.rs/walkdir/) - Directory traversal
- [glob](https://docs.rs/glob/) - Pattern matching
- [anyhow](https://docs.rs/anyhow/) - Error handling
- [clap](https://docs.rs/clap/) - CLI parsing
- [simplelog](https://docs.rs/simplelog/) - Logging
- [indicatif](https://docs.rs/indicatif/) - Progress indicators

### Related Projects

- [Comby](https://comby.dev/) - Structural code search and replace
- [Semgrep](https://semgrep.dev/) - Semantic code analysis
- [Codemod](https://github.com/facebook/codemod) - Facebook's transformation framework
- [jscodeshift](https://github.com/facebook/jscodeshift) - JavaScript codemods
