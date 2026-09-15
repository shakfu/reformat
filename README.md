# reformat

A modular code transformation framework. Each transformer handles one
concern -- renaming files, normalising whitespace, converting identifier case,
etc. -- and the pipeline system lets you compose them into multi-step workflows
that run in a single invocation.

> **Upgrade if you are on 0.1.6 or earlier.**
> Those versions did not exclude `.git` when walking a directory, so
> `reformat rename_files` and the default `reformat -r <path>` could rename
> files inside it -- `HEAD`, `config`, `index` -- destroying the repository.
> Renames are not journalled, so there was nothing to undo it with.
> Versions 0.1.4 through 0.1.6 have been yanked from crates.io; `cargo install
> reformat` now gets a fixed release. See the 0.1.7 entry in
> [CHANGELOG.md](CHANGELOG.md) for the full list of fixes.

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
| `WhitespaceCleaner` | `clean` | Strip trailing whitespace, preserving each line's original terminator |
| `EmojiTransformer` | `emojis` | Replace task/status emojis with text alternatives, remove decorative emojis |
| `FileGrouper` | `group` | Organise files by common prefix into subdirectories, detect and fix broken references |
| `EndingsNormalizer` | `endings` | Normalise line endings to LF, CRLF, or CR (skips binary files automatically) |
| `IndentNormalizer` | `indent` | Convert between tabs and spaces with configurable width, tab-stop-aware |
| `ContentReplacer` | `replace` | Regex find-and-replace with capture group support, multiple sequential patterns |
| `HeaderManager` | `header` | Insert or update file headers (license, copyright) with year templating |
| `StyleStep` | `editorconfig` | Apply `.editorconfig` settings: trailing whitespace, final newline, line endings, and optionally indentation |

All transformers share common behaviours: recursive directory traversal,
file extension filtering, dry-run mode, and the file selection rules below.

### Selecting files

Every command that rewrites contents or names accepts one or more paths, and
these flags:

| Flag | Effect |
|---|---|
| `-e, --extensions EXT` | Only these extensions. `py`, `.py` and `.PY` are equivalent |
| `--include GLOB` | Only files matching a glob (repeatable). Without `-e`, replaces the default extension list, so `--include Makefile` works |
| `--exclude GLOB` | Skip matching files and directories (repeatable) |
| `--hidden` | Walk hidden files and directories, such as `.github/` |
| `--no-ignore` | Ignore `.gitignore` and `.ignore`, and walk `node_modules`, `target`, `dist`, `vendor`, `__pycache__`, `venv`, `.venv` and `build` |

By default, hidden entries, those build directories, and anything matched by
`.gitignore` are skipped. `.git` is never entered, whatever the flags. Globs
use `.gitignore` syntax and are matched relative to the directory walked.

A directory you name explicitly is processed even if it is hidden --
`reformat clean ~/.config/nvim` works. A named build directory needs
`--no-ignore`. A named hidden file needs `--hidden`.

Binary files are skipped. Files that are not valid UTF-8 are skipped with a
warning, except by `endings`, which works on bytes.

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

For the common case of cleaning up a directory, `reformat <path>` replaces task
emojis and strips trailing whitespace, without needing a config file. Add `-r`
to recurse. It does not rename files; use `rename_files` for that.

### Library-first design

The project is organised as a Cargo workspace:

- **reformat-core** -- All transformation logic. Every struct and option type
  is a public API. Use this crate directly if you want programmatic access.
- **reformat-cli** -- Thin CLI wrapper using clap. Parses arguments, loads
  config, calls into core.
- **reformat-plugins** -- Plugin system foundation (not yet active).

### Observability

- Per-file reporting by default; `-v` and `-vv` add diagnostics, `-q` silences
  everything but errors. Transformers report through the `log` facade, so a
  library consumer controls this too.
- File logging (`--log-file`) keeps full detail with timestamps
- Dry-run mode on every transformer and every pipeline step, leaving nothing
  behind on disk
- `--diff` prints a unified diff of content changes; `--check` exits 1 when
  any file would change. Neither writes anything. See [CI and pre-commit](#ci-and-pre-commit)
- Exit status 0 on success, 1 when `--check` finds changes, 2 on error. A file
  that cannot be read or written is reported, the run continues, and the exit
  status is 2

## Before you run it

Transformations are applied **in place and are not reversible**. `reformat` has
no undo, and only `group` records what it did.

- Run against a clean working tree, or a backup. `rename_files`, `group`,
  `convert` and `replace` enforce this: they refuse to modify paths with
  uncommitted changes or untracked files in git, unless given `--allow-dirty`.
  So do presets and jobs containing those steps. Previews are never refused.
- Try `--diff` first. It shows exactly what would change and writes nothing.
- Start narrow. `--extensions`, `--include` and `--exclude` limit the blast
  radius.

Files are written atomically: the new contents go to a temporary file that is
renamed over the original, so an interrupted run cannot truncate a file.

## Caveats

`convert` and `replace` operate on text with regular expressions, not on parsed
syntax. They do not know what is code, what is a comment, and what is a string
literal, and they will rewrite matches in all three. This is the right trade-off
for a tool meant to work across languages, but it means the output of a
whole-tree `convert` deserves review before committing.

Related limits worth knowing:

- `convert` matches identifiers by shape. A single capitalised word is not a
  PascalCase candidate, so ordinary prose is left alone.
- `indent` rewrites leading whitespace only. It cannot tell an indentation tab
  from an alignment tab, so hand-aligned continuation lines may shift.
- `emojis` removes characters by Unicode range. Genuine emoji are removed and
  ordinary text symbols such as card suits and musical notes are preserved, but
  the boundary between the two is a judgement call, not a standard.
- `group`'s reference fixing matches filenames, and only rewrites the exact
  occurrences it recorded. Review `fixes.json` before applying it.

## Installation

Install from crates.io:

```bash
cargo install reformat
```

Or install from the workspace:

```bash
cargo install --path reformat-cli
```

Or with `make` (override the location with `PREFIX`):

```bash
make install PREFIX=~/.local
```

Or build from source:

```bash
cargo build --release -p reformat
```

The binary will be at `./target/release/reformat`

### Shell completions and man pages

```bash
# Completions: bash, zsh, fish, powershell or elvish
reformat completions zsh > ~/.zfunc/_reformat
reformat completions bash > ~/.local/share/bash-completion/completions/reformat

# Man pages: one for reformat and one per subcommand
reformat man --out-dir ~/.local/share/man/man1
reformat man > reformat.1 && man ./reformat.1
```

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
reformat-core = "0.2.0"
```

### Case Conversion

```rust
use reformat_core::{CaseConverter, CaseFormat, ConvertOptions};

let converter = CaseConverter::new(ConvertOptions {
    strip_prefix: Some("m_".to_string()),
    ..ConvertOptions::new(CaseFormat::CamelCase, CaseFormat::SnakeCase)
})?;

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

// Renaming to lowercase is opt-in: options.lowercase_filenames = true.
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

1. Transforms task emojis: ✅ → [x], ☐ → [ ]
2. Removes trailing whitespace

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
Transformed 1 emoji(s) in '/tmp/TestFile.txt'
Cleaned 2 lines in '/tmp/TestFile.txt'
Processed files:
  - Emoji transformations: 1 file(s) (1 changes)
  - Whitespace cleaned: 1 file(s) (2 lines)
```

## Usage

### CI and pre-commit

`--check` modifies nothing and exits 1 if any file would change. `--diff`
prints a unified diff of each change and also modifies nothing. They combine,
and work on every content command, the default command, presets and jobs.
`--check` also works on `rename_files`.

```bash
# Fail CI if any file has trailing whitespace or a missing final newline
reformat clean --check --final-newline .

# Show what a preset would change
reformat -p normalize --diff src/

# Run on the files a hook passes
reformat clean --check $(git diff --cached --name-only --diff-filter=d)
```

In a diff, a carriage return is shown as `\r`, so line-ending changes are
visible. With `--diff`, log lines go to stderr, so
`reformat clean --diff . > changes.patch` writes only the diff.

To run reformat from [pre-commit](https://pre-commit.com), install the binary
(`cargo install reformat`) and add:

```yaml
repos:
  - repo: https://github.com/shakfu/reformat
    rev: v0.2.0
    hooks:
      - id: reformat-clean         # trailing whitespace, final newline, trailing blank lines
      - id: reformat-endings       # LF line endings
      - id: reformat-editorconfig  # .editorconfig settings
      - id: reformat-emojis        # task emojis to text, other emojis removed
```

The hooks run the `reformat` found on PATH (`language: system`). pre-commit's
`language: rust` cannot build this repository, because its root is a virtual
workspace manifest. The clean and endings hooks process every text file
pre-commit passes, hidden ones included.

When several content steps run in one pipeline, each file is read once, every
step is applied in order, and the file is written once. A dry run, `--check`
and `--diff` therefore report the combined result of all steps.

### Editors and stdin

`--stdin-filename NAME` reads content from stdin and writes the result to
stdout, without touching any file. `NAME` decides which steps apply, by
extension, and which `.editorconfig` section is used. It need not exist.

```bash
# Format a buffer
reformat clean --final-newline --stdin-filename src/main.rs < buffer

# Apply a preset or the default command
reformat -p normalize --stdin-filename notes.md < notes.md

# Exit 1 if the buffer would change; print nothing
reformat editorconfig --check --stdin-filename a.txt < a.txt
```

It works with every content command, the default command, presets and jobs.

- `--diff` prints a diff instead of the content.
- `--check` and `--dry-run` print nothing; `--check` still sets the exit status.
- Content whose name is not selected is written back unchanged. This covers
  an extension no step accepts, `--exclude`, and hidden names without
  `--hidden`.
- Log lines go to stderr.
- Steps that rename or move files (`rename`, `group`) are rejected.
- `--job -` cannot be combined with it, since both read stdin.

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

Clean a single file, or several:

```bash
reformat clean myfile.py other.md
```

Fix the end of each file: add a missing final newline, and remove trailing
blank lines. Both are off by default:

```bash
reformat clean --final-newline --trim-blank-lines .
```

Clean only the top level of a directory:

```bash
reformat clean --no-recursive src/
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

Apply a reviewed `fixes.json` later:

```bash
reformat apply_fixes --dry-run fixes.json
reformat apply_fixes fixes.json
```

A fix whose recorded text no longer matches the file is skipped.

A `group` step in a preset or job also writes `changes.json`. It takes exactly
one directory.

### EditorConfig

`reformat editorconfig` applies the settings `.editorconfig` declares for each
file ([spec](https://spec.editorconfig.org/)):

| Property | Effect |
|---|---|
| `trim_trailing_whitespace = true` | Strip trailing whitespace |
| `insert_final_newline = true` | Add a missing final newline |
| `end_of_line` | Convert line endings |
| `indent_style`, `indent_size`, `tab_width` | Convert indentation, only with `--indent` |

```bash
reformat editorconfig --check .
reformat editorconfig --diff src/
reformat editorconfig --indent .
```

Files are selected by `.editorconfig` sections, so `-e` is not accepted; use
`--include` and `--exclude`. Indentation is opt-in because `indent_style =
space` under `[*]` would also rewrite the tabs a `Makefile` needs, unless the
file has its own section. `insert_final_newline = false` and `charset` are
ignored.

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

# Several patterns, applied in order: the Nth --find pairs with the Nth --replace-with
reformat replace -f "old_api\(" --replace-with "new_api(" -f "OldType" --replace-with "NewType" src/

# Plain text, not regex: no escaping, and $ in the replacement is literal
reformat replace --literal -f "cost(\$1)" --replace-with "price(\$1)" src/

# Case-insensitive
reformat replace -i -f "todo" --replace-with "TODO" src/
```

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

Define reusable transformation pipelines in a `reformat.json` file. The file
is searched for in the current directory and its parents, then in each target
path and its parents. `--config FILE` names one explicitly.

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

# List the presets and their steps
reformat presets
reformat presets --config ci/reformat.json
```

**Available step configuration options:**

| Step | Options |
|------|---------|
| `rename` | `case_transform` (lowercase/uppercase/capitalize), `space_replace` (underscore/hyphen), `file_extensions`, `recursive`, `include_symlinks` |
| `emojis` | `replace_task_emojis`, `remove_other_emojis`, `file_extensions`, `recursive` |
| `clean` | `remove_trailing`, `insert_final_newline`, `trim_trailing_blank_lines`, `file_extensions`, `recursive` |
| `convert` | `from_format`, `to_format`, `file_extensions`, `recursive`, `prefix`, `suffix`, `glob`, `word_filter` |
| `group` | `separator`, `min_count`, `strip_prefix`, `from_suffix`, `recursive` |
| `endings` | `style` (lf/crlf/cr), `file_extensions`, `recursive` |
| `indent` | `style` (spaces/tabs), `width`, `file_extensions`, `recursive` |
| `replace` | `patterns` (array of `{find, replace, literal, ignore_case}`; the last two default to false), `file_extensions`, `recursive` |
| `header` | `text`, `update_year`, `file_extensions`, `recursive` |
| `editorconfig` | `indent`, `recursive` |

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
| Source | `reformat.json` (or `--config`) | Any file or stdin |
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
  wbs (3 files):
    - wbs_create.tmpl
    - wbs_delete.tmpl
    - wbs_list.tmpl

  work (2 files):
    - work_package_create.tmpl
    - work_package_delete.tmpl

Found 2 potential group(s).
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
reformat endings --style crlf --no-ignore dist/
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
