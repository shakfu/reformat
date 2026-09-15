# reformat architecture

This document describes how a run flows, the invariants the code maintains,
and where new code goes. The per-type API is documented in the rustdoc
(`cargo doc --workspace --open`). User-facing behaviour is in `README.md`.

## Crates

| Crate | Role | MSRV |
|---|---|---|
| `reformat-core` | Transformations, the content-step runner, presets, change records, reference fixing | 1.82 |
| `reformat` (`reformat-cli/`) | Argument parsing, file discovery, output, exit status, git guard | 1.88 |
| `reformat-plugins` | Empty placeholder | 1.82 |

The floors differ because the CLI's dependencies need 1.88 (`simplelog ->
time`, `ignore`). File discovery lives in the CLI for that reason. CI checks
both floors.

## Module map

```text
reformat-core/src/
  step.rs        ContentStep trait, run_content_steps, write_atomic, extension matching
  style.rs       StyleStep: applies a per-file FileStyle (used for EditorConfig)
  whitespace.rs  clean      trailing whitespace, final newline, trailing blank lines
  emoji.rs       emojis     task emojis to text, other emojis removed
  endings.rs     endings    LF / CRLF / CR; also works on non-UTF-8 bytes
  indent.rs      indent     tabs <-> spaces, leading whitespace only
  replace.rs     replace    sequential regex replacements
  header.rs      header     insert or update a header, year-aware
  converter.rs   convert    identifier case conversion (ConvertOptions)
  case.rs        CaseFormat: patterns, word splitting and joining
  rename.rs      rename     file name transforms
  group.rs       group      move files into prefix directories, record moves
  changes.rs     ChangeRecord (changes.json)
  refs.rs        ReferenceScanner / ReferenceFixer (fixes.json)
  combined.rs    CombinedProcessor: library form of the default command
  config.rs      Preset and per-step *Config types; *Config -> options
  lines.rs       terminator-preserving line splitting
  walk.rs        walkdir-based traversal for the library path methods; in_git_dir

reformat-cli/src/
  main.rs          clap definitions, execute(), presentation, exit status
  select.rs        file discovery with the ignore crate
  dirty.rs         refuse destructive runs over uncommitted work
  editorconfig.rs  .editorconfig -> FileStyle via ec4rs
  config.rs        reformat.json lookup (current directory, then ancestors)
```

## How a run flows

Every command that rewrites contents or names is expressed as a `Preset`: an
ordered list of step names plus per-step config. A subcommand builds a
one-step preset from its flags. `-p` loads a preset from `reformat.json`, and
`--job` parses one from a file or stdin. All three go through `execute` in
`main.rs`. `group` and `apply_fixes` are the exceptions: they have
interactive or record-driven flows of their own.

`execute` does, in order:

1. Validate step names, and check that every path exists. A missing path
   fails the run before any file changes.
2. If the run writes and contains `rename`, `group`, `convert` or `replace`,
   call `dirty::ensure_clean`, which refuses paths with uncommitted changes.
3. If `--include` is given, widen unset `file_extensions` to "any".
4. Split the steps into segments. Consecutive content steps form one segment.
   `rename` and `group` each run alone, because they change paths.
5. For a content segment: build a `ContentStep` per step, discover files once
   with `select::discover`, and call `run_content_steps`.
6. For `rename`: discover files, filter by extension, and call
   `FileRenamer::rename_paths`. For `group`: require one directory, run
   `FileGrouper::process_with_changes`, and write `changes.json` when writing.

`finish` turns the result into the exit status: 2 if any file failed, 1 if
`--check` found changes, otherwise 0. `--dry-run`, `--check` and `--diff` all
mean "write nothing". `--diff` also prints each change.

## Content steps

`ContentStep` separates deciding from doing:

- `accepts(file)` decides from the path alone: extension and depth. It does
  not read the file, and it does not reject hidden names. Selection is the
  caller's job.
- `transform(text, file)` returns new text and a change count, or `None`. It
  never touches the filesystem.
- `transform_bytes` is optional, for content that is not UTF-8. Only
  `endings` implements it.

`run_content_steps` reads each file once and applies the accepting steps in
order to the running result. It reports the original and final bytes to a
callback, which `--diff` uses, and writes once with `write_atomic`. A
multi-step dry run therefore shows each step applied to the previous step's
output. A file that cannot be read or written is recorded in
`RunReport::errors`, and the run continues.

The per-transformer methods (`process`, `clean_file`, `transform_file`, and
so on) are thin wrappers over the same runner. They walk with
`walk::walk_files` and skip hidden and build-directory names, as they did
before the runner existed.

## File selection (CLI)

`select::discover` returns each file at most once, in name order:

- Directories are walked with `ignore::WalkBuilder`. `.gitignore`, `.ignore`
  and git exclude files apply. Hidden entries are skipped unless `--hidden`.
  `DEFAULT_SKIP_DIRS` are pruned unless `--no-ignore`.
- `--include` and `--exclude` become `ignore` overrides. They match relative
  to the walked directory, or to the working directory for a file named
  directly.
- A file named directly is subject to `--hidden` and the globs, but not to
  `.gitignore`.
- The walker never filters its root, so a root that is `.git` or a build
  directory is refused before walking.

## Invariants

These hold for every command. The tests that cover them are named in
parentheses.

- **`.git` is never modified.** `select` prunes it. The runner and the
  renamer refuse any path with a `.git` component, whatever the flags
  (`repo_safety.rs`, `test_runner_never_modifies_git_metadata`,
  `test_rename_paths_selection_and_git_refusal`).
- **No truncated files.** `write_atomic` writes a temporary file in the same
  directory and renames it over the original. It resolves symlinks first,
  copies permissions, and writes in place when a file has several hard links
  (`test_write_atomic_*`).
- **Line terminators survive content edits.** Transformers split with
  `lines::split_lines` and write each terminator back. Only `endings` and
  `editorconfig` change terminators on purpose.
- **Writing nothing means nothing.** A dry run, `--check` or `--diff` leaves
  no file changed and no record written.
- **Binary files are skipped.** A NUL byte marks a file as binary. Files that
  are not UTF-8 are skipped by every step except `endings`.
- **Destructive runs need a clean tree.** `rename`, `group`, `convert` and
  `replace` refuse paths git reports as modified or untracked, unless
  `--allow-dirty`. Fixes chosen at `group`'s interactive prompt are checked
  again before they are applied.

## Reference fixing

`group` moves files and records each move in a `ChangeRecord`
(`changes.json`). `ReferenceScanner` searches the scope directories for the
old file names with Aho-Corasick. It keeps only matches that stand alone as a
path, not ones inside a longer name. It writes a `FixRecord` (`fixes.json`)
with a byte offset per occurrence. `ReferenceFixer` applies fixes back to
front by offset, and skips any fix whose recorded text no longer matches, so
applying a stale or already-applied record changes nothing.
`reformat apply_fixes` runs the fixer on a saved record.

## Adding a content transformer

1. Add a module with an options struct (`Default`) and a transformer type.
   Implement `ContentStep`, using `step::accepts_by_extension` in `accepts`.
2. Add `process` and a per-file method as wrappers over `step::process_path`
   and `step::apply_one`, matching the existing modules.
3. Add a `*Config` in `config.rs` with a `to_options`, a field on `Preset`,
   and the name in `VALID_STEPS`.
4. In `main.rs`, add a subcommand using `Common` and `Recursion`, a branch in
   `build_content_step`, and an entry in `step_recursive`. If the step's
   output needs review before committing, add it to `NEEDS_CLEAN_TREE`.
5. If it has `file_extensions`, add it to `widen_extensions_for_include`.
6. Add text-level unit tests in the module, and CLI tests for any
   non-obvious `--check` or `--diff` behaviour.

## Tests

| File | Covers |
|---|---|
| `reformat-core/src/*.rs` | Unit tests per module |
| `reformat-core/tests/library_integration.rs` | Library use of `CaseConverter` |
| `reformat-cli/tests/cli_integration.rs` | Subcommands, presets, jobs, defect regressions |
| `reformat-cli/tests/workflow.rs` | `--check`, `--diff`, paths, selection, EditorConfig, pre-commit hook entries |
| `reformat-cli/tests/repo_safety.rs` | `.git` safety and the git guard, against real repositories |

Tests that need git skip themselves when it is not installed. Tests that rely
on file permissions are Unix-only.

## Dependencies

| Crate | Used for |
|---|---|
| `regex`, `aho-corasick`, `glob` | Patterns, reference scanning, `convert --glob` |
| `walkdir` | Library path methods |
| `serde`, `serde_json`, `chrono` | Presets, change and fix records, timestamps |
| `anyhow`, `log` | Errors and logging facade |
| `clap` | CLI parsing (CLI only) |
| `ignore` | File discovery (CLI only) |
| `similar` | `--diff` (CLI only) |
| `ec4rs` | `.editorconfig` parsing (CLI only) |
| `simplelog`, `logging_timer` | Log output and timing (CLI only) |
