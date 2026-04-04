# reformat

A modular code transformation framework. Each transformer handles one
concern -- renaming files, normalising whitespace, converting identifier case,
etc. -- and the pipeline system lets you compose them into multi-step workflows
that run in a single invocation.

## Features

### Modular transformers

Every transformation is an independent module with its own options struct,
sensible defaults, and a consistent interface (`process(path) -> Result`).
Transformers can be used standalone via CLI subcommands, composed into
pipelines, or called directly as a Rust library from `reformat-core`.

| Transformer | CLI subcommand | What it does |
|---|---|---|
| `FileRenamer` | `rename_files` | Case transforms, prefix/suffix operations, timestamps on filenames |
| `CaseConverter` | `convert` | Convert identifiers between 6 case formats (camel, pascal, snake, screaming snake, kebab, screaming kebab) |
| `WhitespaceCleaner` | `clean` | Strip trailing whitespace while preserving line endings |
| `EmojiTransformer` | `emojis` | Replace task/status emojis with text alternatives, remove decorative emojis |
| `FileGrouper` | `group` | Organise files by common prefix into subdirectories, detect and fix broken references |
| `EndingsNormalizer` | `endings` | Normalise line endings to LF, CRLF, or CR (skips binary files automatically) |
| `IndentNormalizer` | `indent` | Convert between tabs and spaces with configurable width, tab-stop-aware |
| `ContentReplacer` | `replace` | Regex find-and-replace with capture group support, multiple sequential patterns |
| `HeaderManager` | `header` | Insert or update file headers (license, copyright) with year templating |

All transformers share common behaviours: recursive directory traversal,
file extension filtering, dry-run mode, and automatic skipping of hidden files
and build directories (`.git`, `node_modules`, `target`, `__pycache__`, etc.).

### Pipelines: presets and jobs

Transformers become more useful when composed. The pipeline system chains any
combination of the above steps and runs them in order on the same path.

There are two ways to define a pipeline, reflecting two different needs:

- **Presets** (`-p`) -- Reusable, named pipelines stored in `reformat.json` at the project root. Version-controlled, shared across a team, run repeatedly.
- **Jobs** (`--job`) -- Ad-hoc, throwaway pipelines loaded from any file or stdin. No project config needed. Ideal for one-off migrations, scripted CI transforms, or quick multi-pattern replacements.

Both use the same JSON format (a `steps` array plus per-step config) and the
same execution engine. The only difference is where they are stored.

```json
{
  "steps": ["endings", "indent", "clean", "header"],
  "endings": { "style": "lf" },
  "indent": { "style": "spaces", "width": 4 },
  "header": {
    "text": "// Copyright {year} MyOrg. All rights reserved.",
    "update_year": true,
    "file_extensions": [".rs", ".go"]
  }
}
```

```bash
# As a reusable preset (stored in reformat.json under a name):
reformat -p normalize src/

# As a throwaway job (from a file):
reformat --job normalize.json src/

# As a throwaway job (piped from stdin):
cat normalize.json | reformat --job - src/
```

### Quick processing (default command)

For the common case of cleaning up a directory, `reformat <path>` runs three
transformations in a single optimised pass -- rename to lowercase, replace task
emojis, strip trailing whitespace -- without needing a config file.

### Library-first design

The project is organised as a Cargo workspace:

- **reformat-core** -- All transformation logic. Every struct and option type
  is a public API. Use this crate directly if you want programmatic access.
- **reformat-cli** -- Thin CLI wrapper using clap. Parses arguments, loads
  config, calls into core.
- **reformat-plugins** -- Plugin system foundation (not yet active).

### Observability

- Multi-level verbosity (`-v`, `-vv`, `-vvv`), quiet mode (`-q`), file logging (`--log-file`)
- Progress spinners, automatic operation timing, colour-coded output
- Dry-run mode on every transformer and every pipeline step

## Installation

Install from crates.io:

```bash
cargo install reformat
```

Or install from the workspace:

```bash
cargo install --path reformat-cli
```

Or build from source:

```bash
cargo build --release -p reformat
```

The binary will be at `./target/release/reformat`

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
reformat-core = "0.1.6"
```

### Case Conversion

```rust
use reformat_core::{CaseConverter, CaseFormat};

let converter = CaseConverter::new(
    CaseFormat::CamelCase,    // from
    CaseFormat::SnakeCase,    // to
    None,                     // file_extensions
    false,                    // recursive
    false,                    // dry_run
    String::new(),            // prefix
    String::new(),            // suffix
    None,                     // strip_prefix
    None,                     // strip_suffix
    None,                     // replace_prefix_from
    None,                     // replace_prefix_to
    None,                     // replace_suffix_from
    None,                     // replace_suffix_to
    None,                     // glob_pattern
    None,                     // word_filter
)?;

converter.process_directory(std::path::Path::new("src"))?;
```

### Whitespace Cleaning

```rust
use reformat_core::{WhitespaceCleaner, WhitespaceOptions};

let mut options = WhitespaceOptions::default();
options.dry_run = false;
options.recursive = true;

let cleaner = WhitespaceCleaner::new(options);
let (files_cleaned, lines_cleaned) = cleaner.process(std::path::Path::new("src"))?;
println!("Cleaned {} lines in {} files", lines_cleaned, files_cleaned);
```

### Combined Processing (Default Command)

```rust
use reformat_core::{CombinedProcessor, CombinedOptions};

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

### Line Ending Normalization

```rust
use reformat_core::{EndingsNormalizer, EndingsOptions, LineEnding};

let options = EndingsOptions {
    style: LineEnding::Lf,
    recursive: true,
    dry_run: false,
    ..Default::default()
};

let normalizer = EndingsNormalizer::new(options);
let (files, endings) = normalizer.process(std::path::Path::new("src"))?;
println!("Normalized {} endings in {} files", endings, files);
```

### Indentation Normalization

```rust
use reformat_core::{IndentNormalizer, IndentOptions, IndentStyle};

let options = IndentOptions {
    style: IndentStyle::Spaces,
    width: 4,
    recursive: true,
    dry_run: false,
    ..Default::default()
};

let normalizer = IndentNormalizer::new(options);
let (files, lines) = normalizer.process(std::path::Path::new("src"))?;
println!("Normalized {} lines in {} files", lines, files);
```

### Regex Find-and-Replace

```rust
use reformat_core::{ContentReplacer, ReplaceOptions, ReplacePattern};

let options = ReplaceOptions {
    patterns: vec![
        ReplacePattern {
            find: r"old_api\(".to_string(),
            replace: "new_api(".to_string(),
        },
    ],
    recursive: true,
    dry_run: false,
    ..Default::default()
};

let replacer = ContentReplacer::new(options)?;
let (files, replacements) = replacer.process(std::path::Path::new("src"))?;
println!("Made {} replacements in {} files", replacements, files);
```

### File Header Management

```rust
use reformat_core::{HeaderManager, HeaderOptions};

let options = HeaderOptions {
    text: "// Copyright {year} MyOrg. All rights reserved.\n// SPDX-License-Identifier: MIT".to_string(),
    update_year: true,
    recursive: true,
    dry_run: false,
    ..Default::default()
};

let manager = HeaderManager::new(options)?;
let (files, _) = manager.process(std::path::Path::new("src"))?;
println!("Updated headers in {} files", files);
```

### File Grouping

```rust
use reformat_core::{FileGrouper, GroupOptions};

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
reformat <path>

# Process recursively
reformat -r <path>

# Preview changes without modifying files
reformat -d <path>
```

**What it does:**

1. Renames files to lowercase
2. Transforms task emojis: ✅ → [x], ☐ → [ ]
3. Removes trailing whitespace

**Example:**

```bash
# Clean up an entire project directory
reformat -r src/

# Preview changes first
reformat -d -r docs/

# Process a single file
reformat README.md
```

**Output:**

```text
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
reformat convert --from-camel --to-snake myfile.py
```

Recursive directory conversion:

```bash
reformat convert --from-snake --to-camel -r src/
```

Dry run (preview changes):

```bash
reformat convert --from-camel --to-kebab --dry-run mydir/
```

Add prefix to all converted identifiers:

```bash
reformat convert --from-camel --to-snake --prefix "old_" myfile.py
```

Filter files by pattern:

```bash
reformat convert --from-camel --to-snake -r --glob "*test*.py" src/
```

Only convert specific identifiers:

```bash
reformat convert --from-camel --to-snake --word-filter "^get.*" src/
```

### Whitespace Cleaning

Clean all default file types in current directory:

```bash
reformat clean .
```

Clean with dry-run to preview changes:

```bash
reformat clean --dry-run src/
```

Clean only specific file types:

```bash
reformat clean -e .py -e .rs src/
```

Clean a single file:

```bash
reformat clean myfile.py
```

### Emoji Transformation

Replace task emojis with text in markdown files:

```bash
reformat emojis docs/
```

Process with dry-run to preview changes:

```bash
reformat emojis --dry-run README.md
```

Only replace task emojis, keep other emojis:

```bash
reformat emojis --replace-task --no-remove-other docs/
```

Process specific file types:

```bash
reformat emojis -e .md -e .txt project/
```

### File Grouping

Organize files by common prefix into subdirectories:

```bash
# Preview what groups would be created
reformat group --preview templates/

# Dry run to see what would happen
reformat group --dry-run templates/

# Group files (keep original filenames)
reformat group templates/

# Group files and strip prefix from filenames
reformat group --strip-prefix templates/

# Group by suffix (split at LAST separator) - for multi-part prefixes
reformat group --from-suffix templates/

# Process subdirectories recursively
reformat group -r templates/

# Use custom separator (e.g., hyphen)
reformat group -s '-' templates/

# Require at least 3 files to create a group
reformat group -m 3 templates/
```

Example transformation with `--strip-prefix` (splits at FIRST separator):

```text
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

```text
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

After grouping files, reformat can scan your codebase for broken references:

```bash
# Interactive mode (default) - prompts for scanning
reformat group --strip-prefix templates/

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
reformat group --strip-prefix --no-interactive --scope src templates/

# Skip reference scanning entirely
reformat group --strip-prefix --no-interactive templates/
```

**Generated files:**

- `changes.json` - Record of all file operations (for auditing)
- `fixes.json` - Proposed reference fixes (review before applying)

### Line Ending Normalization

Normalize line endings across files:

```bash
# Convert to Unix line endings (LF) - default
reformat endings src/

# Convert to Windows line endings (CRLF)
reformat endings --style crlf src/

# Preview changes
reformat endings --dry-run src/

# Process specific file types
reformat endings -e .py -e .rs src/
```

### Indentation Normalization

Convert between tabs and spaces:

```bash
# Convert tabs to spaces (4-wide, default)
reformat indent src/

# Convert tabs to 2-space indentation
reformat indent --style spaces --width 2 src/

# Convert spaces to tabs
reformat indent --style tabs --width 4 src/

# Preview changes
reformat indent --dry-run src/
```

### Regex Find-and-Replace

Apply regex patterns across files:

```bash
# Simple text replacement
reformat replace --find "old_name" --replace-with "new_name" src/

# Regex with capture groups
reformat replace --find "func\((\w+), (\w+)\)" --replace-with "func(\$2, \$1)" src/

# Dry run
reformat replace --find "TODO" --replace-with "FIXME" --dry-run src/

# Filter by extension
reformat replace --find "2024" --replace-with "2025" -e .py src/
```

For multiple patterns, use a preset (see Presets section below).

### File Header Management

Insert or update file headers:

```bash
# Insert a license header
reformat header --text "// Copyright 2025 MyOrg\n// SPDX-License-Identifier: MIT" src/

# Insert header with automatic year
reformat header --text "// Copyright {year} MyOrg" --update-year src/

# Preview changes
reformat header --text "// Header" --dry-run src/

# Process specific file types
reformat header --text "# License" -e .py src/
```

### Presets

Define reusable transformation pipelines in a `reformat.json` file in your project root:

```json
{
  "code": {
    "steps": ["rename", "emojis", "clean"],
    "rename": {
      "case_transform": "lowercase",
      "space_replace": "hyphen"
    },
    "emojis": {
      "replace_task_emojis": true,
      "remove_other_emojis": false,
      "file_extensions": [".md", ".txt"]
    },
    "clean": {
      "remove_trailing": true,
      "file_extensions": [".rs", ".py"]
    }
  },
  "templates": {
    "steps": ["group", "clean"],
    "group": {
      "separator": "_",
      "min_count": 3,
      "strip_prefix": true
    }
  }
}
```

Run a preset:

```bash
reformat -p code src/

# Dry-run to preview changes
reformat -p code -d src/

# Run a different preset
reformat -p templates web/templates/
```

**Available step configuration options:**

| Step | Options |
|------|---------|
| `rename` | `case_transform` (lowercase/uppercase/capitalize), `space_replace` (underscore/hyphen), `recursive`, `include_symlinks` |
| `emojis` | `replace_task_emojis`, `remove_other_emojis`, `file_extensions`, `recursive` |
| `clean` | `remove_trailing`, `file_extensions`, `recursive` |
| `convert` | `from_format`, `to_format`, `file_extensions`, `recursive`, `prefix`, `suffix`, `glob`, `word_filter` |
| `group` | `separator`, `min_count`, `strip_prefix`, `from_suffix`, `recursive` |
| `endings` | `style` (lf/crlf/cr), `file_extensions`, `recursive` |
| `indent` | `style` (spaces/tabs), `width`, `file_extensions`, `recursive` |
| `replace` | `patterns` (array of `{find, replace}`), `file_extensions`, `recursive` |
| `header` | `text`, `update_year`, `file_extensions`, `recursive` |

Steps without explicit configuration use sensible defaults.

**Example preset using new transformers:**

```json
{
  "normalize": {
    "steps": ["endings", "indent", "clean", "header"],
    "endings": { "style": "lf" },
    "indent": { "style": "spaces", "width": 4 },
    "header": {
      "text": "// Copyright {year} MyOrg. All rights reserved.\n// SPDX-License-Identifier: MIT",
      "update_year": true,
      "file_extensions": [".rs", ".go", ".js"]
    }
  },
  "migrate-api": {
    "steps": ["replace"],
    "replace": {
      "patterns": [
        { "find": "old_api\\(", "replace": "new_api(" },
        { "find": "Copyright 2024", "replace": "Copyright 2025" }
      ],
      "file_extensions": [".rs", ".py"]
    }
  }
}
```

### Jobs

Jobs are ad-hoc transformation pipelines for one-off tasks. A job file has the same
format as a single preset -- just a JSON object with `steps` and per-step config --
but is loaded from an arbitrary file (or stdin) instead of your project's `reformat.json`.

Run a job from a file:

```bash
reformat --job migrate.json src/
```

Run a job from stdin:

```bash
echo '{"steps":["clean"]}' | reformat --job - src/
```

Example job file for a multi-pattern replacement:

```json
{
  "steps": ["replace", "clean"],
  "replace": {
    "patterns": [
      {"find": "old_api\\(", "replace": "new_api("},
      {"find": "Copyright 2024", "replace": "Copyright 2025"}
    ],
    "file_extensions": [".rs", ".py"]
  }
}
```

Jobs support dry-run mode:

```bash
reformat --job migrate.json --dry-run src/
```

**When to use presets vs. jobs:**

| | Presets (`-p`) | Jobs (`--job`) |
|---|---|---|
| Source | `reformat.json` in project root | Any file or stdin |
| Lifecycle | Reusable, version-controlled | Throwaway, ad-hoc |
| Use case | Standard project workflows | One-off migrations, scripted transforms |

### Logging and Debugging

Control output verbosity:

```bash
# Info level output (-v)
reformat -v convert --from-camel --to-snake src/

# Debug level output (-vv)
reformat -vv clean src/

# Silent mode (errors only)
reformat -q convert --from-camel --to-snake src/

# Log to file
reformat --log-file debug.log -v convert --from-camel --to-snake src/
```

Output example with `-v`:

```text
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
reformat convert --from-camel --to-snake main.py
```

Convert C++ project from snake_case to PascalCase:

```bash
reformat convert --from-snake --to-pascal -r -e .cpp -e .hpp src/
```

Preview converting JavaScript getters to snake_case:

```bash
reformat convert --from-camel --to-snake --word-filter "^get.*" -d src/
```

### Whitespace Cleaning Examples

Clean trailing whitespace from entire project:

```bash
reformat clean -r .
```

Clean only Python files in src directory:

```bash
reformat clean -e .py src/
```

Preview what would be cleaned without making changes:

```bash
reformat clean --dry-run .
```

### Emoji Transformation Examples

Transform task emojis in documentation:

```bash
reformat emojis -r docs/
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
reformat emojis -e .md README.md
```

### File Grouping Examples

Organize template files by prefix (split at first separator):

```bash
reformat group --strip-prefix web/templates/
```

Organize files with multi-part prefixes (split at last separator):

```bash
# activity_relationships_list.tmpl -> activity_relationships/list.tmpl
reformat group --from-suffix web/templates/
```

Preview groups without making changes:

```bash
reformat group --preview web/templates/
```

Example output:

```text
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
reformat group -s '-' --strip-prefix components/
```

Recursively organize nested directories:

```bash
reformat group -r --strip-prefix src/
```

Group files and automatically scan for broken references:

```bash
reformat group --strip-prefix --scope src templates/
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

### Line Ending Normalization Examples

Normalize a cross-platform project to Unix endings:

```bash
reformat endings -r src/
```

Convert to Windows line endings for distribution:

```bash
reformat endings --style crlf -r dist/
```

### Indentation Normalization Examples

Standardize a project to 4-space indentation:

```bash
reformat indent -r src/
```

Convert to 2-space indentation for JavaScript:

```bash
reformat indent --width 2 -e .js -e .ts src/
```

Convert to tabs:

```bash
reformat indent --style tabs --width 4 -e .go src/
```

### Regex Find-and-Replace Examples

Update copyright year across all files:

```bash
reformat replace --find "Copyright 2024" --replace-with "Copyright 2025" -r .
```

Swap function argument order using capture groups:

```bash
reformat replace --find "swap\((\w+), (\w+)\)" --replace-with "swap(\$2, \$1)" src/
```

### File Header Examples

Add MIT license header to all Rust files:

```bash
reformat header -t "// Copyright {year} MyOrg\n// SPDX-License-Identifier: MIT" --update-year -e .rs src/
```

Ensure all Python files have a header (preserves shebang):

```bash
reformat header -t "# Copyright {year} MyOrg" --update-year -e .py src/
```

### Preset Examples

Run a multi-step cleanup preset:

```bash
# Define in reformat.json, then run:
reformat -p code src/

# Output:
#   rename: 3 file(s) renamed
#   emojis: 2 file(s), 5 change(s)
#   clean: 4 file(s), 12 line(s) cleaned
# Preset 'code' complete.
```

Preview preset changes without modifying files:

```bash
reformat -p code -d src/
```

Case conversion preset:

```json
{
  "snake-to-camel": {
    "steps": ["convert"],
    "convert": {
      "from_format": "snake",
      "to_format": "camel",
      "file_extensions": [".py"],
      "recursive": true
    }
  }
}
```

```bash
reformat -p snake-to-camel src/
```

## License

MIT License. See [LICENSE](LICENSE) for details.
