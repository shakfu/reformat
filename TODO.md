# TODO

## Critical

## High

## Medium

## Low

- [ ] Support a `default` preset in `reformat.json` that overrides the default command behavior when present (i.e., `reformat <path>` uses the `default` preset automatically instead of the built-in emojis+clean pipeline) #presets
  - Needed? `reformat -p default <path>` already works.
  - Implicit or opt-in? A `reformat.json` in any ancestor would change the bare command. An opt-in `REFORMAT_PRESET` env var avoids that but cannot be shared through the repo.
  - Escape hatch: how to run the built-in pipeline when a `default` preset exists (e.g. `--no-config`)?
  - `-r`: override per-step `recursive`, be ignored, or error?
  - A malformed config would now fail every bare run. Error or fall back?
  - Existing presets named `default` start running implicitly. Needs a Changed entry.
  - Output: keep the default command's summary or use per-step pipeline output?
  - `reformat presets` should mark the preset the bare command uses.
