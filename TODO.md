# TODO

## Critical

## High

## Medium

- [ ] `rename_files` splits the extension at the last `.` (`FileRenamer::target_path`). `--add-prefix x_` turns `.env` into `x_.env`, which stops it being hidden. `a.tar.gz` gets its suffix before `.gz`. #rename
- [ ] `group` overwrites an existing `changes.json` or `fixes.json` after only a warning. Moves from an earlier run whose references are not yet fixed lose their record. Refuse, or write a new name. #group
- [ ] `group`'s walk does not read `.gitignore`, so it can move ignored files. Git cannot restore those, but the guard does not check them for `group`: including them would refuse any directory holding ignored build output. #group #safety
- [ ] `clean` defaults include `.md`, so it removes Markdown hard line breaks (two trailing spaces). pre-commit's `trailing-whitespace` hook has `--markdown-linebreak-ext` for this. #clean
- [ ] `indent --style tabs` defaults include `.yaml` and `.yml`, where tabs are invalid. #indent

## Low

- [ ] Split `main.rs` (about 2100 lines) into `cli.rs`, `pipeline.rs` and `group_cmd.rs`. `run_group` takes 14 parameters. #cli
- [ ] Adding a step means editing 4 hand-written lists: `step_recursive`, `widen_extensions_for_include`, `build_content_step` and `NEEDS_CLEAN_TREE`. Replace them with a step registry on `Preset`. #cli
- [ ] One "may write" check on the resolved path (not in `.git`, tracked, clean), called from `write_atomic` and the renamer. It would replace the per-command `NEEDS_CLEAN_TREE` list and the separate `.git` checks. Cost: git awareness moves into `reformat-core`, or the check is passed in as a callback. #safety
- [ ] `--allow-dirty` is declared 5 times. `reformat --allow-dirty replace ...` silently ignores the top-level copy. Make it one global flag. #cli
- [ ] The dirty-tree guard runs one `git status` per path, so `$(git ls-files)` spawns N full scans. Batch the paths per repository. #safety
- [ ] Timestamp renames prepend another date on every run. #rename
- [ ] `group` can create a prefix directory named after an entry in `DEFAULT_SKIP_DIRS`, such as `build/`. Later walks and reference scans skip it. #group
- [ ] Change counts are off in places. `clean` counts a whitespace-only trailing line twice (`whitespace.rs`). `replace` counts matches replaced by identical text. `editorconfig` adds lines and line endings into one total (`style.rs`). #output
- [ ] `reformat-plugins` is an empty crate published to crates.io. Drop it until a plugin API exists, or stop publishing it. #release
- [ ] `CombinedProcessor` aborts on the first error, while `run_content_steps` collects errors and continues. #library
- [ ] A malformed `.editorconfig` prints "All files already match" before the error line. #editorconfig
- [ ] Add a test for the guard with a submodule set to `ignore = dirty`. It needs a committed submodule fixture. #safety #tests
- [ ] On Windows, `same_entry` in `rename.rs` treats an existing case-only clash as one entry. That is wrong in directories with per-directory case sensitivity enabled. #rename #windows

- [ ] Support a `default` preset in `reformat.json` that overrides the default command behavior when present (i.e., `reformat <path>` uses the `default` preset automatically instead of the built-in emojis+clean pipeline) #presets
  - Needed? `reformat -p default <path>` already works.
  - Implicit or opt-in? A `reformat.json` in any ancestor would change the bare command. An opt-in `REFORMAT_PRESET` env var avoids that but cannot be shared through the repo.
  - Escape hatch: how to run the built-in pipeline when a `default` preset exists (e.g. `--no-config`)?
  - `-r`: override per-step `recursive`, be ignored, or error?
  - A malformed config would now fail every bare run. Error or fall back?
  - Existing presets named `default` start running implicitly. Needs a Changed entry.
  - Output: keep the default command's summary or use per-step pipeline output?
  - `reformat presets` should mark the preset the bare command uses.
