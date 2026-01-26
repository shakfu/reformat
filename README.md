# refmt

A modular code transformation framework for applying code transformations to code in a set of source code files.

Organized as a Cargo workspace:
- **refmt-core**: Core transformation library
- **refmt-cli**: Command-line interface
- **refmt-plugins**: Plugin system (foundation)

## Features

### Default Command (Quick Processing)
- **NEW**: Run all common transformations in a single pass with `refmt <path>`
- Combines three operations efficiently:
  1. Rename files to lowercase
  2. Transform task emojis to text alternatives
  3. Remove trailing whitespace
- **3x faster** than running individual commands separately
- Perfect for quick project cleanup: `refmt -r src/`

### Case Format Conversion
- Convert between 6 case formats: camelCase, PascalCase, snake_case, SCREAMING_SNAKE_CASE, kebab-case, and SCREAMING-KEBAB-CASE
- Process single files or entire directories (with recursive option)
- Dry-run mode to preview changes
- Filter files by glob patterns
- Filter which words to convert using regex patterns
- Add prefix/suffix to converted identifiers
- Support for multiple file extensions (.c, .h, .py, .md, .js, .ts, .java, .cpp, .hpp)

### Whitespace Cleaning
- Remove trailing whitespace from files
- Preserve line endings and file structure
- Recursive directory processing
- Extension filtering with sensible defaults
- Dry-run mode to preview changes
- Automatically skips hidden files and build directories

### Emoji Transformation
- Replace task completion emojis with text alternatives (✅ → [x], ☐ → [ ], etc.)
- Replace status indicator emojis (🟡 → [yellow], 🟢 → [green], 🔴 → [red])
- Remove non-task emojis from code and documentation
- Smart replacements for common task tracking symbols
- Configurable behavior (replace task emojis, remove others, or both)
- Support for markdown, documentation, and source files

### File Grouping
- Organize files by common prefix into subdirectories
- Automatically detect file prefixes based on separator character
- Optional prefix stripping from filenames after moving
- **Suffix-based splitting** (`--from-suffix`): Split at the LAST separator for multi-part prefixes
- Preview mode to see what groups would be created
- Configurable minimum file count for group creation
- Recursive processing for nested directories
- **Broken reference detection**: Automatically scan codebase for references to moved files
- **Interactive fix workflow**: Review and apply fixes for broken references
- Change tracking with `changes.json` and `fixes.json` output files

### Logging & UI
- Multi-level verbosity control (`-v`, `-vv`, `-vvv`)
- Quiet mode for silent operation (`-q`)
- File logging for debugging (`--log-file`)
- Progress spinners with indicatif
- Automatic operation timing
- Color-coded console output

## Installation

Install from the workspace:

```bash
cargo install --path refmt-cli
```

Or build from source:

```bash
cargo build --release -p refmt
```

The binary will be at `./target/release/refmt`

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
refmt-core = { path = "../refmt-core" }
```

### Case Conversion

```rust
use refmt_core::{CaseConverter, CaseFormat};

let converter = CaseConverter::new(
    CaseFormat::CamelCase,
    CaseFormat::SnakeCase,
    None, false, false,
    String::new(), String::new(),
    None, None
)?;

converter.process_directory(std::path::Path::new("src"))?;
```

### Whitespace Cleaning

```rust
use refmt_core::{WhitespaceCleaner, WhitespaceOptions};

let mut options = WhitespaceOptions::default();
options.dry_run = false;
options.recursive = true;

let cleaner = WhitespaceCleaner::new(options);
let (files_cleaned, lines_cleaned) = cleaner.process(std::path::Path::new("src"))?;
println!("Cleaned {} lines in {} files", lines_cleaned, files_cleaned);
```

### Combined Processing (Default Command)

```rust
use refmt_core::{CombinedProcessor, CombinedOptions};

let mut options = CombinedOptions::default();
options.recursive = true;
options.dry_run = false;

let processor = CombinedProcessor::new(options);
let stats = processor.process(std::path::Path::new("src"))?;

println!("Files renamed: {}", stats.files_renamed);
println!("Emojis transformed: {} files ({} changes)",
         stats.files_emoji_transformed, stats.emoji_changes);
println!("Whitespace cleaned: {} files ({} lines)",
         stats.files_whitespace_cleaned, stats.whitespace_lines_cleaned);
```

### File Grouping

```rust
use refmt_core::{FileGrouper, GroupOptions};

let mut options = GroupOptions::default();
options.strip_prefix = true;  // Remove prefix from filenames
options.from_suffix = false;  // Set true to split at LAST separator
options.min_count = 2;        // Require at least 2 files to create a group
options.dry_run = false;

let grouper = FileGrouper::new(options);
let stats = grouper.process(std::path::Path::new("templates"))?;

println!("Directories created: {}", stats.dirs_created);
println!("Files moved: {}", stats.files_moved);
println!("Files renamed: {}", stats.files_renamed);
```

## Quick Start

### Default Command (Recommended)

The fastest way to clean up your code:

```bash
# Process directory (non-recursive)
refmt <path>

# Process recursively
refmt -r <path>

# Preview changes without modifying files
refmt -d <path>
```

**What it does:**
1. Renames files to lowercase
2. Transforms task emojis: ✅ → [x], ☐ → [ ]
3. Removes trailing whitespace

**Example:**
```bash
# Clean up an entire project directory
refmt -r src/

# Preview changes first
refmt -d -r docs/

# Process a single file
refmt README.md
```

**Output:**
```
Renamed '/tmp/TestFile.txt' -> '/tmp/testfile.txt'
Transformed emojis in '/tmp/testfile.txt'
Cleaned 2 lines in '/tmp/testfile.txt'
Processed files:
  - Renamed: 1 file(s)
  - Emoji transformations: 1 file(s) (1 changes)
  - Whitespace cleaned: 1 file(s) (2 lines)
```

## Usage

### Case Conversion

Basic conversion (using subcommand):
```bash
refmt convert --from-camel --to-snake myfile.py
```

Or legacy mode (backwards compatible):
```bash
refmt --from-camel --to-snake myfile.py
```

Recursive directory conversion:
```bash
refmt convert --from-snake --to-camel -r src/
```

Dry run (preview changes):
```bash
refmt convert --from-camel --to-kebab --dry-run mydir/
```

Add prefix to all converted identifiers:
```bash
refmt convert --from-camel --to-snake --prefix "old_" myfile.py
```

Filter files by pattern:
```bash
refmt convert --from-camel --to-snake -r --glob "*test*.py" src/
```

Only convert specific identifiers:
```bash
refmt convert --from-camel --to-snake --word-filter "^get.*" src/
```

### Whitespace Cleaning

Clean all default file types in current directory:
```bash
refmt clean .
```

Clean with dry-run to preview changes:
```bash
refmt clean --dry-run src/
```

Clean only specific file types:
```bash
refmt clean -e .py -e .rs src/
```

Clean a single file:
```bash
refmt clean myfile.py
```

### Emoji Transformation

Replace task emojis with text in markdown files:
```bash
refmt emojis docs/
```

Process with dry-run to preview changes:
```bash
refmt emojis --dry-run README.md
```

Only replace task emojis, keep other emojis:
```bash
refmt emojis --replace-task --no-remove-other docs/
```

Process specific file types:
```bash
refmt emojis -e .md -e .txt project/
```

### File Grouping

Organize files by common prefix into subdirectories:
```bash
# Preview what groups would be created
refmt group --preview templates/

# Dry run to see what would happen
refmt group --dry-run templates/

# Group files (keep original filenames)
refmt group templates/

# Group files and strip prefix from filenames
refmt group --strip-prefix templates/

# Group by suffix (split at LAST separator) - for multi-part prefixes
refmt group --from-suffix templates/

# Process subdirectories recursively
refmt group -r templates/

# Use custom separator (e.g., hyphen)
refmt group -s '-' templates/

# Require at least 3 files to create a group
refmt group -m 3 templates/
```

Example transformation with `--strip-prefix` (splits at FIRST separator):
```
Before:                          After:
templates/                       templates/
├── wbs_create.tmpl             ├── wbs/
├── wbs_delete.tmpl             │   ├── create.tmpl
├── wbs_list.tmpl               │   ├── delete.tmpl
├── work_package_create.tmpl    │   └── list.tmpl
├── work_package_delete.tmpl    ├── work/
└── other.txt                   │   ├── package_create.tmpl
                                │   └── package_delete.tmpl
                                └── other.txt
```

Example transformation with `--from-suffix` (splits at LAST separator):
```
Before:                                    After:
templates/                                 templates/
├── activity_relationships_list.tmpl      ├── activity_relationships/
├── activity_relationships_create.tmpl    │   ├── list.tmpl
├── activity_relationships_delete.tmpl    │   ├── create.tmpl
├── user_profile_edit.tmpl                │   └── delete.tmpl
├── user_profile_view.tmpl                ├── user_profile/
└── other.txt                             │   ├── edit.tmpl
                                          │   └── view.tmpl
                                          └── other.txt
```

#### Broken Reference Detection

After grouping files, refmt can scan your codebase for broken references:

```bash
# Interactive mode (default) - prompts for scanning
refmt group --strip-prefix templates/

# Output:
# Grouping complete:
#   - Directories created: 2
#   - Files moved: 5
# 
# Changes recorded to: changes.json
# 
# Would you like to scan for broken references? [y/N]: y
# Enter directories to scan: src
# 
# Found 3 broken reference(s).
# Proposed fixes written to: fixes.json
# 
# Review fixes.json and apply changes? [y/N]: y
# Fixed 3 reference(s) in 2 file(s).
```

```bash
# Non-interactive mode with automatic scanning
refmt group --strip-prefix --no-interactive --scope src templates/

# Skip reference scanning entirely
refmt group --strip-prefix --no-interactive templates/
```

**Generated files:**
- `changes.json` - Record of all file operations (for auditing)
- `fixes.json` - Proposed reference fixes (review before applying)

### Logging and Debugging

Control output verbosity:
```bash
# Info level output (-v)
refmt -v convert --from-camel --to-snake src/

# Debug level output (-vv)
refmt -vv clean src/

# Silent mode (errors only)
refmt -q convert --from-camel --to-snake src/

# Log to file
refmt --log-file debug.log -v convert --from-camel --to-snake src/
```

Output example with `-v`:
```
2025-10-10T00:15:08.927Z [INFO] Converting from CamelCase to SnakeCase
2025-10-10T00:15:08.927Z [INFO] Target path: /tmp/test.py
2025-10-10T00:15:08.927Z [INFO] Recursive: false, Dry run: false
Converted '/tmp/test.py'
2025-10-10T00:15:08.931Z [INFO] Conversion completed successfully
2025-10-10T00:15:08.931Z [INFO] run_convert(), Elapsed=4.089125ms
```

## Case Format Options

- `--from-camel` / `--to-camel` - camelCase (firstName, lastName)
- `--from-pascal` / `--to-pascal` - PascalCase (FirstName, LastName)
- `--from-snake` / `--to-snake` - snake_case (first_name, last_name)
- `--from-screaming-snake` / `--to-screaming-snake` - SCREAMING_SNAKE_CASE (FIRST_NAME, LAST_NAME)
- `--from-kebab` / `--to-kebab` - kebab-case (first-name, last-name)
- `--from-screaming-kebab` / `--to-screaming-kebab` - SCREAMING-KEBAB-CASE (FIRST-NAME, LAST-NAME)

## Examples

### Case Conversion Examples

Convert Python file from camelCase to snake_case:
```bash
refmt convert --from-camel --to-snake main.py
```

Convert C++ project from snake_case to PascalCase:
```bash
refmt convert --from-snake --to-pascal -r -e .cpp -e .hpp src/
```

Preview converting JavaScript getters to snake_case:
```bash
refmt convert --from-camel --to-snake --word-filter "^get.*" -d src/
```

### Whitespace Cleaning Examples

Clean trailing whitespace from entire project:
```bash
refmt clean -r .
```

Clean only Python files in src directory:
```bash
refmt clean -e .py src/
```

Preview what would be cleaned without making changes:
```bash
refmt clean --dry-run .
```

### Emoji Transformation Examples

Transform task emojis in documentation:
```bash
refmt emojis -r docs/
```

Example transformation:
```markdown
Before:
- Task done ✅
- Task pending ☐
- Warning ⚠ issue
- 🟡 In progress
- 🟢 Complete
- 🔴 Blocked

After:
- Task done [x]
- Task pending [ ]
- Warning [!] issue
- [yellow] In progress
- [green] Complete
- [red] Blocked
```

Process only markdown files:
```bash
refmt emojis -e .md README.md
```

### File Grouping Examples

Organize template files by prefix (split at first separator):
```bash
refmt group --strip-prefix web/templates/
```

Organize files with multi-part prefixes (split at last separator):
```bash
# activity_relationships_list.tmpl -> activity_relationships/list.tmpl
refmt group --from-suffix web/templates/
```

Preview groups without making changes:
```bash
refmt group --preview web/templates/
```

Example output:
```
Found 2 potential group(s):

  wbs (3 files):
    - wbs_create.tmpl
    - wbs_delete.tmpl
    - wbs_list.tmpl

  work (2 files):
    - work_package_create.tmpl
    - work_package_delete.tmpl
```

Group files with hyphen separator:
```bash
refmt group -s '-' --strip-prefix components/
```

Recursively organize nested directories:
```bash
refmt group -r --strip-prefix src/
```

Group files and automatically scan for broken references:
```bash
refmt group --strip-prefix --scope src templates/
```

Example `changes.json`:
```json
{
  "operation": "group",
  "timestamp": "2026-01-15T16:30:00+00:00",
  "base_dir": "/project/templates",
  "changes": [
    {"type": "directory_created", "path": "wbs"},
    {"type": "file_moved", "from": "wbs_create.tmpl", "to": "wbs/create.tmpl"}
  ]
}
```

Example `fixes.json`:
```json
{
  "generated_from": "changes.json",
  "fixes": [
    {
      "file": "src/handler.go",
      "line": 15,
      "context": "template.ParseFiles(\"wbs_create.tmpl\")",
      "old_reference": "wbs_create.tmpl",
      "new_reference": "wbs/create.tmpl"
    }
  ]
}
```

## License

See LICENSE file for details.
