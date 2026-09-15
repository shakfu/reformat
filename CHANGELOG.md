# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Library: `ChangeRecord::add_directory_created`, `add_file_moved` and
  `add_file_renamed` take `impl AsRef<Path>` and store paths with `/`
  separators. Converting inside the record means a new caller cannot
  reintroduce the Windows bug below. `&str` and `&Path` arguments still
  compile; `&Cow<str>` does not.

### Fixed

- On Windows, `group` recorded moves with `\` separators, so `apply_fixes`
  and the interactive prompt wrote references such as `wbs\a.tmpl` into
  source files. A `fixes.json` written by an earlier Windows build is
  corrected when read.

## [0.2.0] - 2026-09-15

This release breaks the CLI and the library API. See Changed.

### Changed

- **The default command no longer renames files.** `reformat <path>` now only
  replaces task emojis and strips trailing whitespace. Lowercasing every name
  renamed `Cargo.toml`, `Makefile` and `Button.tsx`, which broke builds and
  imports. Use `rename_files --to-lowercase`, or set
  `CombinedOptions::lowercase_filenames` in the library.
- **Exit status is now 0, 1 or 2.** 1 means `--check` found changes; 2 means
  an error. Errors previously exited 1, which `--check` would have made
  ambiguous.
- **A file that cannot be read or written no longer stops the run.** It is
  reported, the remaining files are processed, and the exit status is 2.
  Previously `convert` logged the error and exited 0, while the other
  transformers stopped at the first failure and left the tree half-processed.
- Directories are walked with the `ignore` crate, so files matched by
  `.gitignore` are skipped by default. `--no-ignore` restores the old
  selection.
- Files are written through a temporary file and a rename, so an interrupted
  run cannot truncate a file. Symbolic links and permissions are preserved. A
  file with several hard links is written in place, since a rename would
  detach it.
- With `--diff`, log lines go to stderr, so stdout carries only the diff.
- A closed stdout, as when piping into `head`, ends the run quietly with
  status 0 instead of an error.
- Binary files are skipped at debug level rather than with a warning.
- `rename_files`, `group`, `convert` and `replace` refuse to modify paths
  with uncommitted changes or untracked files in git, as do presets and jobs
  containing those steps. `--allow-dirty` overrides. Previews and paths
  outside a git work tree are unaffected. Reference fixes chosen at `group`'s
  interactive prompt are checked again before they are applied. `clean` and the other hygiene
  commands are not guarded, since pre-commit runs them on staged files.
- Library: an empty `file_extensions` list now matches every file; it
  previously matched none. Extension matching ignores case and a leading dot.
  `WhitespaceOptions`, `CleanConfig`, `RenameConfig`, `CombinedOptions` and
  `Preset` gained fields, so struct literals without `..Default::default()`
  need updating.
- Library: `ContentStep::accepts` no longer rejects hidden names, since the
  caller selects files. The per-transformer `process` and `*_file` methods
  still skip them. `FileRenamer::rename_file` and the content runner refuse
  any path inside `.git`; the renamer previously checked only the file name.
- Library: `CaseConverter::new` takes a `ConvertOptions` struct instead of 15
  positional arguments, 11 of them strings or optional strings, where a
  transposed pair compiled silently. Build it with
  `ConvertOptions { .., ..ConvertOptions::new(from, to) }`. A replace prefix or
  suffix is now a `(from, to)` pair; a half given alone in a preset is an
  error rather than ignored.
- Library: removed `text::read_text` and `ReplacePatternConfig`, both unused.
  Use `step::run_content_steps` and `config::ReplacePatternEntry`.

### Added

- `--check` and `--diff` on every content command, the default command,
  presets and jobs. `--check` also works on `rename_files`. Neither writes
  anything.
- Commands take several paths: `reformat clean a.py b.md`. Every path is
  checked before any file is changed.
- File selection flags on every content command and `rename_files`:
  `--include GLOB`, `--exclude GLOB`, `--hidden` and `--no-ignore`. Without
  `-e`, `--include` replaces the default extension list, so extensionless files
  such as `Makefile` are reachable.
- `clean --final-newline` and `--trim-blank-lines`, also available as
  `insert_final_newline` and `trim_trailing_blank_lines` in presets. Both are
  off by default.
- `reformat editorconfig` and the `editorconfig` step apply
  `trim_trailing_whitespace`, `insert_final_newline` and `end_of_line` from
  `.editorconfig`. Indentation is applied only with `--indent`, because
  `indent_style = space` under `[*]` would also rewrite the tabs a `Makefile`
  needs.
- `--stdin-filename NAME` reads content from stdin and writes the result to
  stdout, for editor integration. `NAME` selects the steps and the
  `.editorconfig` section; `--diff` and `--check` work as for files. Works
  with content commands, the default command, presets and jobs. Library:
  `apply_to_bytes` runs content steps on in-memory input.
- `replace` takes `--find` and `--replace-with` repeatedly, pairing them in
  order, plus `--literal` and `--ignore-case`. Presets accept `literal` and
  `ignore_case` per pattern. A second pattern previously needed a job file.
- `reformat.json` is also found from the target paths, not only the current
  directory, so `reformat -p tidy ../other-project/src` works. `--config
  FILE` names the file, and `reformat presets` lists presets and their steps.
- `reformat completions <shell>` prints a completion script for bash, zsh,
  fish, PowerShell or Elvish. `reformat man` prints the man page, and
  `--out-dir` writes one page per subcommand.
- `.pre-commit-hooks.yaml` with `reformat-clean`, `reformat-endings`,
  `reformat-editorconfig` and `reformat-emojis`. They use `language: system`:
  pre-commit's Rust support runs `cargo install --path .`, which fails on this
  repository's virtual workspace root.
- `reformat apply_fixes <fixes.json>` applies reference fixes recorded by
  `group`. `group` already told users to apply them later, but no command
  could.
- `--no-recursive` on `clean`, `emojis`, `rename_files`, `endings`, `indent`,
  `replace` and `header`, and `--no-replace-task` and `--no-remove-other` on
  `emojis`. These options were declared as flags with a `true` default, so they
  could not be turned off, and the README's `--no-remove-other` example failed.
- Library: the `ContentStep` trait and `step::run_content_steps`. Consecutive
  content steps in a pipeline now share one read and one write per file, so a
  multi-step dry run reports each step against the previous step's output
  rather than the original file.

### Fixed

- `header` inserted the header before a UTF-8 byte-order mark, moving the mark
  into the middle of the file. In a CRLF file it inserted LF line breaks,
  leaving mixed endings, and a multi-line header already written with CRLF
  endings was not recognised, so a second copy was inserted.
- `header` on a file whose only line is a shebang with no newline joined the
  header onto the shebang line.
- A `group` step in a preset or job discarded its record of moves, so
  references could not be fixed afterwards. It now writes `changes.json`.
- `group --preview -r` ignored `-r`.
- `-e py` without the leading dot matched nothing and exited 0.
- `emojis` with task replacement off deleted task emojis anyway, because
  their code points fall inside the decorative ranges that
  `remove_other_emojis` removes. They are now kept.
- `convert` now reports how many identifiers it changed.

### Internal

- `test_collision_does_not_abort_the_run` could not run on case-insensitive
  filesystems, so the macOS and Windows CI jobs failed on 0.1.8. The test built
  its collision from a case-only clash, writing `b.txt` and then `B.txt`; on
  APFS and NTFS the second write overwrites the first, so the collision it
  meant to exercise never existed and the run reported zero skipped files. The
  fixture now derives the collision from space replacement -- `a b.txt` cannot
  become `a_b.txt` because that name is taken -- which behaves the same on
  every platform, and it additionally asserts that the blocked file keeps its
  original name and contents.
- No library or binary code changed, and no released version is affected.
  `FileRenamer` already handled this correctly: it canonicalizes both paths
  before reporting a collision, so a case-only rename on a case-insensitive
  filesystem is allowed rather than refused.

## [0.1.8] - 2026-08-26

A documentation and metadata release. No behavioural changes.

### Fixed

- The declared minimum supported Rust version now reaches the published
  crates, and is correct. `rust-version = "1.82"` was set in
  `[workspace.package]` for 0.1.7 but not inherited by the member manifests, so
  `cargo metadata` reported no MSRV and the published manifests carried none.
  Users on an older toolchain got a compile error rather than cargo's clear
  "requires rustc" message.
- The value was also wrong. 1.82 was derived from this codebase's own use of
  `Option::is_none_or` without checking the dependency tree, where
  `simplelog -> time` requires 1.88. The floors differ per crate and are now
  declared that way: `reformat-core` and `reformat-plugins` at 1.82, the
  `reformat` binary at 1.88. Declaring one workspace-wide floor would have
  pushed library consumers to the CLI's requirement for no reason. CI verifies
  both.

### Documentation

- Added an upgrade warning to the top of the README. Versions 0.1.4 through
  0.1.6 did not exclude `.git` when walking a directory, so `rename_files` and
  the default `reformat -r <path>` could rename files inside it and destroy the
  repository; those versions have been yanked from crates.io. A yank is silent,
  so this is what reaches someone already running an affected version. Package
  pages render the README of the published version, which is why this needs a
  release to appear on crates.io.

## [0.1.7] - 2026-08-26

### Fixed

**Transformers no longer traverse into `.git`, or into build and vendor directories**

- `FileRenamer` checked only the final path component for a leading dot, so
  `WalkDir` descended into `.git/` and renamed `HEAD`, `config`, `index` and
  loose objects, destroying the repository. `reformat rename_files` and the
  default `reformat -r <path>` command were both affected. Renames are not
  journalled, so there was no way to undo it.
- `CaseConverter` had no exclusion logic at all and rewrote identifiers inside
  `node_modules/`, `target/` and other vendored trees.
- Exclusion is now shared: `reformat_core::walk` provides `is_excluded`,
  `is_excluded_component`, `include_entry`, `walk_files` and
  `walk_files_and_symlinks`, and all nine transformers use them. Excluded
  subtrees are pruned with `WalkDir::filter_entry` before being descended into,
  rather than walked and then filtered file by file.

**`reformat <command> .` is no longer a silent no-op**

- The exclusion check scanned every path component including `.`, which reads
  as a hidden directory name. `reformat clean .` -- the most natural way to
  invoke the tool -- therefore matched nothing and reported "No files needed
  cleaning" while leaving every file untouched. Only `Component::Normal`
  components are now considered, so `.` and `..` are treated as navigation.
  Affected `clean`, `emojis`, `endings`, `indent`, `replace` and `header`.
- The same defect pruned the walk root during reference scanning, so
  `group --scope .` scanned zero files. The walk root is now always accepted;
  explicitly named hidden files are still skipped by the per-file check.

**Trailing-whitespace and indentation cleanup no longer rewrite line endings**

- `clean` and `indent` split with `str::lines()` and rejoined with a literal
  `"\n"`, so any CRLF file they touched was silently converted to LF. The
  conversion was conditional on the file needing a change at all, so within one
  run some files were converted and others were not. `README.md` documented the
  opposite behaviour ("while preserving line endings").
- Both now split with `reformat_core::lines::split_lines`, which yields
  `(body, terminator)` pairs and recognises LF, CRLF and lone CR. Only the body
  is modified; the original terminator is written back. Lone-CR files, which the
  old code also mangled, are handled correctly now too.

**`header --update-year` updates the existing header instead of duplicating it**

- The year-flexibility regex was `(?:19|20)\d\{2\}`. The escaped braces made
  `\{2\}` a literal `{2}` rather than a repetition quantifier, so the
  substitution never fired, no year-variant header was ever recognised, and the
  update branch was unreachable. Running `--update-year` in a new year inserted
  a second header above the first; run annually, headers accumulated.
  `CHANGELOG.md` for 0.1.6 claimed this feature worked.
- Header detection is now anchored to the header zone -- the top of the file,
  after any shebang and leading blank lines -- so a year-variant string
  elsewhere in the body (a test fixture, a vendored blob) is no longer mistaken
  for the file's own header and rewritten in place. Previously detection
  searched the entire file.

**Reference fixing rewrites only the recorded occurrences**

- `ReferenceFixer::apply_fixes_to_file` performed a global
  `String::replace(old, new)` across the whole file, discarding the line and
  column the scanner had carefully recorded. Combined with substring matching,
  a move of `user_list.tmpl` also rewrote every mention of
  `super_user_list.tmpl` and `user_list.tmpl.bak` -- different files entirely.
  Fixes are now applied by byte offset, back to front, so only the recorded
  occurrences change.
- Matching now requires the reference to stand alone: a match flanked by a
  filename character (alphanumeric, `_`, `-`, `.`) is part of a longer name
  and is not reported. A leading `/` is still accepted, so a path-qualified
  reference such as `tmpl/user_list.tmpl` is correctly rewritten to
  `tmpl/user/list.tmpl`.
- Applying a record is now idempotent and stale-safe. Each edit verifies that
  the text at the recorded position is still the recorded `old_reference`
  before touching it; anything else is skipped and counted. Previously a
  second run compounded the replacement, turning `x/a.txt` into `x/x/a.txt`,
  because the move re-introduced `a.txt` as a substring.
- `references_fixed` now counts occurrences actually rewritten rather than
  fixes attempted, and `files_modified` counts only files that were written --
  it previously incremented for every file processed, including those where
  nothing matched.
- Deduplication no longer discards distinct occurrences. Two references on the
  same line at different columns are separate edits; deduplicating on
  `(file, line, old_reference)` kept only the first, so the rest were never
  fixed.
- `ReferenceFix` gained an optional `offset` field carrying the authoritative
  byte position. Records written by earlier versions omit it and fall back to
  line and column, so an existing `fixes.json` still applies.
- `ApplyResult` gained `references_skipped`, reported by the CLI, so a stale or
  already-applied record is visible rather than silently doing nothing.

**Emoji removal no longer leaves debris or deletes ordinary text**

- Emoji are matched as whole sequences rather than as individual code points.
  Removing the people from a ZWJ sequence (a family emoji) used to leave the
  U+200D joiners behind as invisible debris, and keycaps (`1` + U+FE0F +
  U+20E3) lost their digit but kept the enclosing mark. Skin tone modifiers
  and variation selectors are consumed with their base emoji. Joiners and
  keycap marks orphaned by earlier versions are cleaned up on the next run.
- Card suits (U+2660-U+2667) and musical notes (U+2669-U+266F) are excluded.
  They sit in the Miscellaneous Symbols block, which was matched wholesale, so
  `cards <U+2660> <U+2665> end` became `cards   end`. Genuine emoji from the
  same block are still removed.
- A task emoji written with an explicit presentation selector is replaced
  without leaving the selector behind.

**Acronyms survive case conversion**

- Word splitting broke on every capital, so `parseHTTPResponse` converted to
  `parse_h_t_t_p_response`. A run of capitals is now one word, ending one
  character early when the last capital begins the next word.
- The candidate patterns admit runs of capitals, so `XMLHttpRequest` is a
  conversion candidate at all. A single capitalised word still is not, so
  ordinary prose in `.md` files is left alone.

**`convert`'s prefix and suffix options actually work**

- `--strip-prefix m_`, documented with exactly that example, silently did
  nothing: the candidate pattern could not match `m_userName`, and `userName`
  within it has no word boundary before it since `_` is a word character. The
  pattern now admits any configured affix as an optional part of the match.
  This affected the subcommand and the preset path alike.

**`group --recursive` is recursive**

- It collected only the immediate subdirectories of the target, so nothing
  more than one level down was ever grouped, despite the flag's description.
- Group output is deterministic. Directories were iterated straight out of a
  `HashMap`, so the directories created, the lines logged and the contents of
  `changes.json` all varied between runs on identical input. `preview` returns
  a `BTreeMap` for the same reason.

**Dry runs leave nothing behind**

- `group --dry-run` wrote `changes.json` describing moves that had not
  happened. Feeding that record to the reference fixer would rewrite
  references to files still sitting where they were.
- `--changes-file` and `--fixes-file` set where those records are written;
  replacing an existing file is announced rather than silent.

**Errors are reported once, and missing paths fail**

- A failure was printed three times: by the command, by `main`, and by
  anyhow's own handler. Only anyhow's remains.
- Every command now exits non-zero for a path that does not exist. `reformat
  clean /nonexistent` used to print "No files needed cleaning" and exit 0,
  making a typo in a CI script indistinguishable from success.

**Robustness**

- A non-UTF-8 or binary file carrying a processed extension is skipped with a
  warning. Transformers read with `fs::read_to_string(path)?` and propagated
  the error, so one such file aborted the whole directory walk partway,
  leaving the tree half-processed.
- A rename collision no longer aborts the run. `FileRenamer::process_with_stats`
  reports what was renamed, what was skipped, and why; one collision in a large
  tree used to abandon it half-renamed with no record of what had moved.
- `ReferenceScanner::new` returns `Result` instead of panicking on a
  pathological pattern set.
- Reference scan patterns are sorted, so scan results are reproducible.

**Configuration is parsed strictly**

- An unrecognised `case_transform`, `space_replace`, `timestamp`, `style` or
  `separator` value is now an error naming the valid alternatives. Unknown
  values fell through to a no-op variant, so a typo such as `"lowercse"`
  silently disabled the step it configured.
- Unknown keys are rejected and named. A misspelled key was dropped in
  silence, leaving the user believing they had configured something.

**`--quiet` works, and output is no longer garbled**

- Transformers reported what they touched with `println!` straight from the
  library, where no CLI flag could reach them. They now log at `info`, which is
  the default level, so ordinary runs look the same and `--quiet` silences
  them. Library consumers control this through the `log` facade.
- Progress spinners are gone. They wrote to the same stream as the per-file
  reporting and overwrote it, and showed no actual progress. The per-file lines
  already convey that work is happening.
- The per-file "No changes needed" line, previously printed for every
  unmodified file on a real run but not on a dry run, is now a debug message.

**Hidden directories the user named explicitly are no longer skipped**

- Exclusion checked every component of a path, so anything under a hidden
  ancestor was silently skipped: `reformat clean ~/.config/nvim` did nothing,
  and so did any run inside a `.tmp`-prefixed scratch directory. Pruning now
  happens during the walk, and the walk root is honoured even when hidden.
  Metadata and build directories are still refused as a root, so pointing the
  renamer at `.git` cannot destroy a repository.

**Presets are found from a subdirectory**

- `reformat.json` was only ever read from the current directory, so running
  from anywhere below the project root found nothing -- despite the
  documentation describing the file as living at the project root. The current
  directory and then each ancestor are searched.

### Changed

- Dependency hygiene: `rayon` and the `parallel` feature it gated are gone. The
  feature was on by default and requested explicitly by the CLI, but nothing in
  the codebase ever used rayon, so it advertised a capability that did not
  exist. `thiserror` (declared in two crates, used in neither), `indicatif`
  (unused since the spinners were removed), and `reformat-plugins`' three
  unused dependencies are also removed, along with the CLI's dependency on
  `reformat-plugins`, which it never called into.
- Internal crate versions are declared once in `[workspace.dependencies]`. The
  CLI pinned `reformat-core` and `reformat-plugins` at `0.1.4` while the
  workspace was at `0.1.6`.
- `rust-version = "1.82"` is declared, and CI checks it.
- CI now runs tests on Linux, macOS and Windows, checks the MSRV, builds in
  release mode, and runs clippy with `--all-targets`. The lint gate previously
  covered library code only, leaving 51 warnings in test code unseen.
- `make install` uses `install(1)` and honours `PREFIX`; there is an
  `uninstall` target. It previously copied to `/usr/local/bin` without `sudo`
  and reported success regardless of the outcome.
- Tests use `tempfile::TempDir` rather than 160 hardcoded paths under the
  system temp directory. Fixtures are now unique per test and are removed even
  when a test panics; the old shared paths let parallel tests clobber each
  other, which they did.
- CLI coverage added for `group`, `endings`, `indent` and `header`, four of the
  nine subcommands -- including the most destructive one, which had none.

### Documentation

- Corrected the claims that all transformers skip build directories, that
  `clean` preserves line endings, that `--update-year` updates headers in
  place, that the default command is a single pass, that `-q` suppresses
  output, and that presets are read from the project root. Each is now true of
  the code rather than of the intention.
- Added a "Before you run it" section: transformations are in place and
  irreversible, `--dry-run` exists, start from a clean working tree.
- Added a "Caveats" section: `convert` and `replace` match text, not syntax,
  and will rewrite matches inside comments and string literals.
- `docs/ARCHITECTURE.md` describes the actual tree: no `reformat.json` at the
  repository root, tests under each crate, and the `walk`, `lines` and `text`
  modules.



**Option assembly is single-sourced**

- Each step's `*Config` is now the one place options are built, via
  `to_options()` (and `ConvertConfig::to_converter()`) in `reformat-core`. The
  subcommands and the preset/job runner both construct a config and hand it to
  the same code, where previously each assembled options separately and the
  two had drifted: the preset `convert` step hardcoded `None` for every
  strip/replace affix setting, leaving half of `ConvertConfig` unreachable from
  a preset. Steps now return a `StepOutcome` and callers format it, so a
  standalone command and a pipeline step keep their own wording. `main.rs`
  lost around 300 lines.
- `RenameConfig` and `ConvertConfig` gained the fields needed to express
  everything their subcommands can.
- File timestamps use `chrono` rather than hand-rolled calendar arithmetic, and
  are local rather than UTC.

**Lint gate restored**

- `reformat-core/src/rename.rs` failed `cargo clippy -- -D warnings`, so the
  CI `clippy` job was red on `main`. Fixed by using `sort_by_key`.

- The skip list is now uniform across every transformer and additionally covers
  `dist/` and `vendor/`, which previously only the reference scanner excluded.
- Non-recursive directory processing walks with `max_depth(1)` instead of
  `fs::read_dir`, so an unreadable entry is skipped rather than aborting the run.

### Added

- `tempfile` dev-dependency and `reformat-cli/tests/repo_safety.rs`, which
  asserts that every transformer leaves version-control metadata intact, that
  build directories are skipped, and that relative `.` paths are processed.
- `reformat_core::lines`, a line splitter that keeps each line's terminator
  separate from its body, for transformers that edit content but must not
  change how lines are separated.

## [0.1.6]

### Added

#### New Transformers

**Line Ending Normalization** (`endings` subcommand)

- Normalize line endings to LF, CRLF, or CR across files
- Byte-level parsing correctly distinguishes CR, LF, and CRLF sequences
- Binary file detection via null-byte scanning (skips binary files automatically)
- Supports file extension filtering, recursive processing, and dry-run mode
- Usage: `reformat endings --style lf src/`
- Library API: `EndingsNormalizer`, `EndingsOptions`, `LineEnding`

**Indentation Normalization** (`indent` subcommand)

- Convert between tabs and spaces with configurable width
- Tab-stop-aware conversion: tabs align to the next multiple of the configured width
- Handles mixed indentation (tabs + spaces on the same line)
- Partial tab stops preserved when converting spaces to tabs (e.g., 6 spaces with width 4 becomes 1 tab + 2 spaces)
- Only modifies leading whitespace; content after indentation is untouched
- Usage: `reformat indent --style spaces --width 4 src/`
- Library API: `IndentNormalizer`, `IndentOptions`, `IndentStyle`

**Regex Find-and-Replace** (`replace` subcommand)

- Apply regex patterns with capture group support (`$1`, `$2`, etc.)
- Chain multiple patterns in sequence via presets/jobs (output of pattern N is input to pattern N+1)
- Regex compilation happens once at construction time; invalid patterns produce clear errors
- Usage: `reformat replace --find "old_api\\(" --replace-with "new_api(" src/`
- Library API: `ContentReplacer`, `ReplaceOptions`, `ReplacePattern`

**File Header Management** (`header` subcommand)

- Insert or update license/copyright headers at the top of source files
- `{year}` template variable with automatic current-year substitution (`--update-year`)
- Year-flexible detection: existing headers with any 4-digit year (19xx/20xx) are recognized and updated in place
- Preserves shebang lines (`#!/usr/bin/env python`) above the header
- Three modes: insert (header missing), update (header present but year differs), skip (exact match)
- Usage: `reformat header -t "// Copyright {year} MyOrg" --update-year src/`
- Library API: `HeaderManager`, `HeaderOptions`

#### Jobs (`-j` / `--job`)

- **New ad-hoc pipeline execution** for one-off transformation tasks
  - Same format as a single preset, loaded from a file or stdin instead of `reformat.json`
  - Run with `reformat --job <file.json> <path>` or `reformat --job - <path>` (stdin)
  - Mutually exclusive with `--preset` (enforced by clap)
  - Supports all steps and dry-run mode
  - Ideal for: multi-pattern replacements, migration scripts, scripted CI transforms

- **Example job file**:

  ```json
  {
    "steps": ["replace", "clean"],
    "replace": {
      "patterns": [
        {"find": "old_api\\(", "replace": "new_api("},
        {"find": "Copyright 2024", "replace": "Copyright 2025"}
      ]
    }
  }
  ```

- **Stdin usage**:

  ```bash
  echo '{"steps":["clean"]}' | reformat --job - src/
  ```

#### Pipeline Architecture Refactor

- Extracted shared `run_pipeline()` execution engine from `run_preset()`
- Both presets (`-p`) and jobs (`--job`) now use the same pipeline executor
- Preset steps expanded from 5 to 9: `rename`, `emojis`, `clean`, `convert`, `group`, `endings`, `indent`, `replace`, `header`

#### New Per-step Configuration Options

- `endings`: `style` (lf/crlf/cr), `file_extensions`, `recursive`
- `indent`: `style` (spaces/tabs), `width`, `file_extensions`, `recursive`
- `replace`: `patterns` (array of `{find, replace}`), `file_extensions`, `recursive`
- `header`: `text`, `update_year`, `file_extensions`, `recursive`

#### New Core Modules

- **EndingsNormalizer** (`reformat-core/src/endings.rs`) -- line ending normalization
- **IndentNormalizer** (`reformat-core/src/indent.rs`) -- indentation conversion
- **ContentReplacer** (`reformat-core/src/replace.rs`) -- regex find-and-replace
- **HeaderManager** (`reformat-core/src/header.rs`) -- file header management
- **Config additions**: `EndingsConfig`, `IndentConfig`, `ReplaceConfig`, `HeaderConfig`, `ReplacePatternEntry`

### Changed

- Pipeline completion message changed from "Preset '...' complete." to "Pipeline '...' complete." (reflects that both presets and jobs use the same engine)

### Testing

- Added 10 new unit tests for `EndingsNormalizer` (CRLF/LF/CR conversion, mixed endings, binary skip, dry-run)
- Added 10 new unit tests for `IndentNormalizer` (tabs-to-spaces, spaces-to-tabs, mixed indent, partial tab stops)
- Added 9 new unit tests for `ContentReplacer` (simple replacement, regex, capture groups, multi-pattern, invalid regex)
- Added 9 new unit tests for `HeaderManager` (insert, update year, shebang preservation, multiline, dry-run)
- Added 8 new CLI integration tests for `--job` (file, stdin, multi-step, dry-run, missing file, invalid JSON, unknown step, conflicts with preset)
- All 201 tests passing

## [0.1.5] - 2026-03-29

### Added

#### Presets (`-p` / `--preset`)

- **New preset system** for defining reusable transformation pipelines in `reformat.json`
  - Define named presets with an ordered list of steps: `rename`, `emojis`, `clean`, `convert`, `group`
  - Per-step configuration overrides (e.g., custom case transforms, file extensions, separators)
  - Run with `reformat -p <preset-name> <path>`
  - Dry-run support via `-d` flag applies to all steps in the preset

- **Configuration file format** (`reformat.json`):

  ```json
  {
    "code": {
      "steps": ["rename", "emojis", "clean"],
      "rename": { "case_transform": "lowercase" },
      "emojis": { "replace_task_emojis": true, "remove_other_emojis": false }
    },
    "templates": {
      "steps": ["group", "clean"],
      "group": { "separator": "_", "min_count": 3, "strip_prefix": true }
    }
  }
  ```

- **Per-step configuration options**:
  - `rename`: `case_transform`, `space_replace`, `recursive`, `include_symlinks`
  - `emojis`: `replace_task_emojis`, `remove_other_emojis`, `file_extensions`, `recursive`
  - `clean`: `remove_trailing`, `file_extensions`, `recursive`
  - `convert`: `from_format`, `to_format`, `file_extensions`, `recursive`, `prefix`, `suffix`, `glob`, `word_filter`
  - `group`: `separator`, `min_count`, `strip_prefix`, `from_suffix`, `recursive`

- **Step validation**: Unknown step names are rejected with a clear error listing valid steps

#### New Core Module

- **Config module** (`reformat-core/src/config.rs`)
  - `ReformatConfig` type (preset name to `Preset` mapping)
  - `Preset` struct with ordered steps and optional per-step config
  - Per-step config structs: `RenameConfig`, `EmojiConfig`, `CleanConfig`, `ConvertConfig`, `GroupConfig`
  - `validate_steps()` for rejecting unknown step names
  - Case format parsing helpers for convert config

#### New CLI Module

- **Config loader** (`reformat-cli/src/config.rs`)
  - `load_config()` / `load_config_from()` for reading `reformat.json`
  - `get_preset()` for looking up and validating a named preset

### Fixed

- Simplified CLI integration test binary path resolution using `env!("CARGO_BIN_EXE_reformat")` instead of fragile manual path traversal with fallback build logic

### Testing

- Added 7 new unit tests for core config module (deserialization, validation, case format parsing)
- Added 6 new CLI unit tests for config loading and preset lookup
- Added 6 new CLI integration tests for preset execution (single step, multi-step, dry-run, missing config, unknown preset)
- All 155 tests passing

## [0.1.4]

### Added

#### File Grouping: Suffix-based Splitting (`--from-suffix`)

- **New `--from-suffix` option** for grouping files by splitting at the LAST separator instead of the first
  - Useful when files have multi-part prefixes like `activity_relationships_list.tmpl`
  - Creates directories from the full prefix (everything before the last separator)
  - Uses the suffix (part after last separator) as the filename

- **Example transformation** with `--from-suffix`:

  ```text
  Before:                                    After:
  activity_relationships_list.tmpl          activity_relationships/
  activity_relationships_create.tmpl            list.tmpl
  activity_relationships_delete.tmpl            create.tmpl
  activity_relationships_detail.tmpl            delete.tmpl
  activity_relationships_edit.tmpl              detail.tmpl
                                                edit.tmpl
  ```

- **Comparison of splitting modes**:

  | Input | `--strip-prefix` (first sep) | `--from-suffix` (last sep) |
  |-------|------------------------------|---------------------------|
  | `a_b_c.txt` | `a/b_c.txt` | `a_b/c.txt` |
  | `user_profile_edit.tmpl` | `user/profile_edit.tmpl` | `user_profile/edit.tmpl` |

- **Usage**:

  ```bash
  reformat group --from-suffix templates/
  ```

- `--from-suffix` implicitly enables prefix stripping (no need to also specify `--strip-prefix`)

### Testing

- Added 4 new unit tests for suffix-based splitting
- All 135 tests passing

## [0.1.3]

### Added

#### File Grouping Command (`group`)

- **New `group` subcommand** for organizing files by common prefix into subdirectories
  - Analyzes files in a directory and identifies common prefixes
  - Creates subdirectories matching file prefixes
  - Moves files into their respective subdirectories
  - Optional prefix stripping from filenames after moving

- **Use cases**:
  - Organize template files: `wbs_create.tmpl`, `wbs_delete.tmpl` → `wbs/create.tmpl`, `wbs/delete.tmpl`
  - Group related files by naming convention
  - Clean up flat directory structures into organized hierarchies

- **Command options**:
  - `-d, --dry-run`: Preview changes without modifying files
  - `-r, --recursive`: Process subdirectories recursively
  - `-s, --separator <CHAR>`: Separator character (default: `_`)
  - `-m, --min-count <N>`: Minimum files to create a group (default: 2)
  - `--strip-prefix`: Remove prefix from filenames after moving
  - `--preview`: Show groups that would be created without making changes
  - `--no-interactive`: Skip interactive prompts
  - `--scope <DIR>`: Directory to scan recursively for broken references

- **Example transformations**:

  ```text
  # Without --strip-prefix:
  wbs_create.tmpl → wbs/wbs_create.tmpl

  # With --strip-prefix:
  wbs_create.tmpl → wbs/create.tmpl
  work_package_list.tmpl → work/package_list.tmpl
  ```

#### Broken Reference Detection and Fixing

- **Automatic change tracking**: After grouping, generates `changes.json` with a complete record of all file operations
- **Interactive workflow**: Prompts user to scan for broken references after grouping
- **Reference scanning**: Scans codebase for references to moved/renamed files
  - Searches quoted strings, paths, template includes, config files
  - Supports common file types: Go, Python, JS/TS, Rust, Java, YAML, JSON, HTML, etc.
  - Automatically excludes `.git`, `node_modules`, `target`, etc.
- **Fix generation**: Creates `fixes.json` with proposed fixes including:
  - File location (path, line, column)
  - Context (surrounding code)
  - Old and new reference values
- **Fix application**: User reviews `fixes.json` and confirms before applying changes

- **Example workflow**:

  ```bash
  $ reformat group --strip-prefix templates/
  Created directory: templates/wbs
  Moved and renamed 'wbs_create.tmpl' -> 'wbs/create.tmpl'

  Changes recorded to: changes.json

  Would you like to scan for broken references? [y/N]: y
  Enter directories to scan: src

  Found 2 broken reference(s).
  Proposed fixes written to: fixes.json

  Review fixes.json and apply changes? [y/N]: y
  Fixed 2 reference(s) in 2 file(s).
  ```

- **Non-interactive mode**:

  ```bash
  reformat group --strip-prefix --no-interactive --scope src templates/
  ```

#### New Core Modules

- **FileGrouper** (`reformat-core/src/group.rs`)
  - `GroupOptions` struct for configuration
  - `GroupStats` and `GroupResult` for operation statistics and change tracking
  - `preview()` method for dry analysis
  - `process_with_changes()` for full change tracking
  - Full support for dry-run and recursive modes

- **ChangeRecord** (`reformat-core/src/changes.rs`)
  - Tracks all changes from refactoring operations
  - Serializable to JSON for persistence
  - Records: directories created, files moved, files renamed

- **ReferenceScanner** (`reformat-core/src/refs.rs`)
  - Scans files for references to moved/renamed files
  - Configurable file extensions and exclusion patterns
  - `FixRecord` for proposed fixes
  - `ReferenceFixer` for applying fixes

### Testing

- Added 12 new unit tests for `FileGrouper`
- Added 5 new unit tests for `ChangeRecord`
- Added 6 new unit tests for `ReferenceScanner` and `ReferenceFixer`
- Tests cover: basic grouping, prefix stripping, dry-run mode, recursive processing, custom separators, minimum count thresholds, reference detection, fix application
- All 94 tests passing

## [0.1.2]

### Added

#### Default Command (Combined Processing)

- **New default command** for efficient single-pass processing
  - `reformat <path>`: Process files without specifying a subcommand
  - `reformat -r <path>`: Process recursively
  - Combines three transformations in order:
    1. Rename files to lowercase
    2. Transform task emojis to text alternatives
    3. Remove trailing whitespace
  - **Performance**: ~3x faster than running individual commands separately
  - Single directory traversal instead of three separate scans

#### New Core Module

- **CombinedProcessor** (`reformat-core/src/combined.rs`)
  - Efficient single-pass file processing
  - Tracks and reports detailed statistics for all transformations
  - Returns `CombinedStats` with counts for files renamed, emojis transformed, and whitespace cleaned
  - Full support for dry-run and recursive modes
  - Handles path updates after file renaming automatically

### Changed

- CLI now accepts optional path argument at the top level
- Existing subcommands (`convert`, `clean`, `emojis`, `rename_files`) remain unchanged
- Updated help text to highlight new default command usage

### Testing

- Added 4 new unit tests for `CombinedProcessor`
- Added 4 new CLI integration tests for default command
- All 88 tests passing (37 CLI + 51 core + 11 library integration)
- Tests handle case-insensitive filesystems (macOS/Windows)

### Documentation

- Updated `CLAUDE.md` with architecture details for combined processing
- Added usage examples and performance notes
- Documented the transformation pipeline and benefits

## [0.1.1]

### Overview

This release represents a major architectural overhaul and feature expansion. The project has been restructured as a Cargo workspace with a library-first design, enabling both CLI and programmatic usage. Three new subcommands have been added (`convert`, `clean`, `emojis`), along with comprehensive logging and UI enhancements.

### Changed

- **BREAKING**: Restructured project as Cargo workspace
  - **reformat-core**: Core library for transformations
  - **reformat-cli**: Command-line binary
  - **reformat-plugins**: Plugin system foundation
- Library-first architecture enables programmatic usage
- CLI now supports modern subcommand architecture with three commands:
  - `reformat convert`: Case format conversion
  - `reformat clean`: Whitespace cleaning
  - `reformat emojis`: Emoji transformation
- Enhanced CLI with comprehensive logging and UI features
- Maintained full backwards compatibility for legacy CLI interface (direct flags still work)

### Added

#### New Transformers

**Whitespace Cleaning Transformer** (`clean` subcommand)

- Removes trailing whitespace from lines while preserving line endings
- Supports dry-run mode (`--dry-run`) for previewing changes
- Recursive processing (default: enabled, `-r` flag)
- Extension filtering with sensible defaults for common code files
- Automatically skips hidden files and build directories (`.git`, `node_modules`, `target`, etc.)
- Example: `reformat clean src/`

**Emoji Transformation** (`emojis` subcommand)

- Replaces task completion emojis with text alternatives for better compatibility
- **Smart emoji mappings**:
  - ✅ → `[x]` (white check mark)
  - ☐ → `[ ]` (ballot box)
  - ☑ → `[x]` (ballot box with check)
  - ✓ → `[x]` (check mark)
  - ✔ → `[x]` (heavy check mark)
  - ☒ → `[X]` (ballot box with X)
  - ❌ → `[X]` (cross mark)
  - ❎ → `[X]` (negative squared cross mark)
  - ⚠ → `[!]` (warning sign)
  - 📝 → `[note]` (memo)
  - 📋 → `[list]` (clipboard)
  - 📌 → `[pin]` (pushpin)
  - 📎 → `[clip]` (paperclip)
- Removes non-task emojis from documentation and code
- Configurable behavior:
  - `--replace-task`: Replace task emojis with text (default: true)
  - `--remove-other`: Remove non-task emojis (default: true)
- Support for markdown, text, and source code files
- Example: `reformat emojis README.md`

#### Logging & UI Enhancements

- **Multi-level verbosity control**:
  - Default: WARN level (minimal output)
  - `-v`: INFO level (shows progress and completion)
  - `-vv`: DEBUG level (detailed operation information)
  - `-vvv`: TRACE level (maximum verbosity)
- **Quiet mode** (`-q`): Suppresses all output except errors
- **File logging** (`--log-file <PATH>`): Write debug logs to file for troubleshooting
- **Progress indicators**: Animated spinners during file processing using `indicatif`
- **Automatic timing**: Operations log execution time at INFO level
  - Example output: `run_convert(), Elapsed=4.089125ms`
- **Color-coded output**: Structured, timestamped logs with `simplelog`
- **Global flags**: `-v`, `-q`, and `--log-file` work with all subcommands

#### Library Features

- **Public API** exports for all transformers:
  - `CaseConverter` and `CaseFormat` for case conversion
  - `WhitespaceCleaner` and `WhitespaceOptions` for whitespace cleaning
  - `EmojiTransformer` and `EmojiOptions` for emoji transformation
- Modular workspace structure for easier feature additions
- Plugin system foundation in `reformat-plugins`
- Comprehensive inline documentation and module docs
- Example library usage in integration tests

### Testing

**Comprehensive Test Coverage**:

- **Unit tests** (24 total):
  - 12 tests for case conversion module (`case.rs`, `converter.rs`)
  - 6 tests for whitespace cleaning module
  - 6 tests for emoji transformation module
- **Library integration tests** (7 total):
  - Tests for programmatic API usage
  - Validation of library behavior
- **CLI integration tests** (20 total):
  - 13 tests for case conversion CLI
  - 7 tests for whitespace cleaning CLI
  - Tests cover: version, help, basic operations, dry-run, recursive processing, error handling
- **Total: 51 tests** - all passing with zero functional regressions

**Test Features**:

- Isolated test environments using temp directories
- Tests for dry-run modes across all transformers
- Extension filtering validation
- Hidden file and build directory skipping
- Pattern matching and glob filtering
- All edge cases covered

### Technical Details

**Architecture**:

- Split monolithic `src/main.rs` (437 lines) into organized modules across 3 crates
- **Core modules**:
  - `reformat-core/src/case.rs` - Case format definitions and conversion logic
  - `reformat-core/src/converter.rs` - File processing and pattern matching
  - `reformat-core/src/whitespace.rs` - Trailing whitespace removal
  - `reformat-core/src/emoji.rs` - Emoji detection and replacement
  - `reformat-core/src/lib.rs` - Public API exports
- **CLI module**:
  - `reformat-cli/src/main.rs` - Clap-based CLI with subcommands and logging

**Implementation Highlights**:

- Whitespace cleaner preserves file line endings (CRLF/LF)
- Emoji transformer uses Unicode regex patterns for comprehensive detection
- Smart emoji replacement mappings maintain markdown compatibility
- Manual character iteration for camelCase/PascalCase splitting (Rust regex limitation)
- Regex-based pattern matching for case format identification
- Glob matching supports both filename and relative path patterns

**Dependencies Added**:

- `log` (0.4) - Logging facade
- `simplelog` (0.12) - Logging implementation with color support
- `indicatif` (0.17) - Progress bars and spinners
- `logging_timer` (1.1) - Automatic function timing

**Performance**:

- All transformations complete in milliseconds for typical projects
- Example timing: `run_convert(), Elapsed=4.089125ms`
- Efficient regex-based pattern matching
- Minimal memory overhead with streaming file processing

## [0.1.0]

### Added

- Initial Rust implementation of reformat CLI tool with Python-compatible API
- Support for 6 case format conversions:
  - camelCase
  - PascalCase
  - snake_case
  - SCREAMING_SNAKE_CASE
  - kebab-case
  - SCREAMING-KEBAB-CASE
- Core conversion features:
  - Single file and directory processing
  - Recursive directory traversal (`-r, --recursive`)
  - Dry-run mode for previewing changes (`-d, --dry-run`)
  - Custom file extension filtering (`-e, --extensions`)
  - Glob pattern filtering for file selection (`--glob`)
  - Regex pattern filtering for selective word conversion (`--word-filter`)
  - Prefix and suffix support for converted identifiers (`--prefix`, `--suffix`)
- Default support for common file extensions: `.c`, `.h`, `.py`, `.md`, `.js`, `.ts`, `.java`, `.cpp`, `.hpp`
- Comprehensive unit test suite (8 tests) covering:
  - Bidirectional conversions between formats
  - Pattern matching accuracy
  - Prefix/suffix functionality
- CLI built with clap v4.5 using derive macros, matching Python argparse API:
  - `--from-camel`, `--from-pascal`, `--from-snake`, etc.
  - `--to-camel`, `--to-pascal`, `--to-snake`, etc.
- Project documentation:
  - README.md with usage examples
  - CLAUDE.md with architecture details
  - Inline code documentation

### Technical Details

- Manual character-by-character word splitting for camelCase/PascalCase (Rust regex doesn't support lookahead/lookbehind)
- Regex-based pattern matching for identifying case formats
- Glob matching supports both filename and relative path patterns
- Error handling with user-friendly messages

