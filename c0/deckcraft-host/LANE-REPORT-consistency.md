# Lane report: tool modes and a foldable right dock (SOW consistency)

## Tool-mode decision

DeckCraft's tools are one ribbon: a tab row over a command card of fixed height, groups running left
to right. It has no column to count, so PhotoCraft's single/double column wording does not carry
over literally. Mapping onto the same three key names (defaults = upstream):

| Key | Meaning here |
|---|---|
| `toolbar` (true) | Command card shown. False: folded to the tab row (the host owns it; tab clicks cannot reopen it). |
| `toolbar_double` (false) | Compact: the unchanged ribbon drawn at 0.8 scale in the same dock. This is the closest honest reading of "two columns = denser"; the ribbon cannot be re-flowed without rewriting `ribbon.rs`. |
| `toolbar_floating` (false) | The ribbon lifted into a movable strip (drag its grip row) over the canvas area. DeckCraft has no floating palette of its own, so this is the "movable strip" fallback. Combines with compact. |
| `right_dock_collapsed` (false) | The task pane (Format Shape, Animation, Selection, Comments, Design Ideas, ...) folds to a 36-point icon strip; each icon opens its pane and unfolds; the open pane is remembered and returns on unfold. Immediate, no animation. |

Fits well: `toolbar` off, floating, dock fold. Weakest: `toolbar_double` (scale, not a re-layout).

## Commits (c0/main)

- `ab62fcd` feat(c0deck): ribbon tool modes and a foldable right dock (code + `tests/c0_tools.rs`)
- docs/smoke commit: README keys, PATCH-DEBT, `smoke.py` tool-mode check, this report

Code: `crates/ui-egui/src/c0_host/tools.rs` (new). Upstream lines: ratchet stays **69** (the one `lib.rs`
call `c0_host::bars` replaces the previous status-bar seam line).

## Tests

- `cargo test -p deckcraft-ui-egui`: 216 unit + 10, 14, 1, 4, 11 (`c0_tools`), 0: all pass, 0 failed.
- `c0_tools.rs` (11): defaults, set/inspect/reset for each key, unknown + mistyped keys, not saved in the
  document, toolbar off survives tab clicks, compact scale 0.8 and still clickable, floating moves by
  grip drag and returns, floating+compact, dock fold/unfold restoring the pane, strip icon opens pane.
- `cargo test -p deckcraft-web --features page-control`: 4 pass.
- `cargo clippy -p deckcraft-ui-egui -p deckcraft-web --all-targets --features deckcraft-web/page-control -- -D warnings`: clean.
- Wasm check (`--target wasm32-unknown-unknown --features page-control --no-default-features`): passes.
- Page build `trunk build --release` in `c0/deckcraft-host`: passes.
- Headless Chromium `smoke.py`: all PASS, including "tool modes and right dock set and read back"; zero JS exceptions.
- `debt.py`: Total 69, under 70.

## Wasm size

`dist/c0host/deckcraft-web-2252b83efa7b5dfa_bg.wasm`: **29,836,089 bytes** (previous report: 29,910,062).

## Load

Before the gate script: `12.96 11.30 12.37 17/1658`; after the trunk build: `12.02 10.15 10.90 5/1738`.
Disk 84% before, 85% during the page build (under the 88% stop).

## Assumptions I could not confirm

- The "pin cap" for wasm size is not stated in the repo; I only compared with the previous lane's size.
- Compact as 0.8 scale and the 1100-point strip width are my choices, not a DeckCraft convention.
- Folding changes the canvas width by the strip/pane width, as opening a pane always did upstream; the canvas
  content is not otherwise moved, but I did not measure pixel-level reflow in a real browser.
- Pointer drag of the floating strip is covered by a kittest drag, not by a real-browser drag.
- The folded pane is hidden by taking `ui.pane`, so `ui.inspect` shows no pane while folded.
