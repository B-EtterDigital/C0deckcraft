# Lane report: upstream pull 20261028 (55e5ee56b4b2)

Branch `fix`, merge `3fa1397` (upstream `55e5ee56b4b2c64973b78fda4182a78f9766fa62` into `c0/main` `b9a035e`), then
`05cd437` (PATCH-DEBT re-list) and this report. The merge descends from `c0/main`; nothing was pushed.

## Conflicted files and how each was resolved

Upstream split many files into directories. Every resolution took upstream's version and re-applied our hook in
its new home. Nothing was dropped and no file was resurrected.

| Conflict | Upstream new home | Resolution |
|---|---|---|
| `crates/engine/src/cmd/insert.rs` | `insert/picture.rs` (`media()`) | Re-applied (1 line). Needed: probe policy alone does not stop `video/*` and `audio/*` content types from being embedded. |
| `crates/ui-egui/src/backstage.rs` | `backstage/files.rs` (Home), `backstage/info.rs` (Account) | Re-applied (1 + 1 lines). |
| `crates/ui-egui/src/dialogs.rs` | `dialogs/about.rs` | Re-applied (1 line, three links to one call). |
| `crates/ui-egui/src/ribbon.rs` | File tab stays in `ribbon.rs`; Learn More in `ribbon/slide_show.rs` | Re-applied (1 + 1 lines). |
| `crates/ui-egui/src/lib.rs` | Impl moved to `app.rs`, `app/commands.rs`; types stay in `lib.rs` | Module line and fields kept in `lib.rs`; `preflight_media`, `app.links` guard in `app/commands.rs`; `tick`, `title_bar`, `bars` in `app.rs`. The upstream `open_backstage`/`sync_backstage` copies in `app.rs` were deleted because `c0_host/chrome.rs` already holds the host-guarded versions (bodies were unchanged upstream since `51eef99`). |
| `.github/workflows/packaging-lint.yml`, `windows-arm64.yml` (modify/delete) | — | Kept deleted (our side). Not resurrected. |

Clean auto-merges that moved with renames, verified by presence of each added line in its final file:
`credits.rs` → `dialogs/credits.rs` (2 hunks), `thumbs.rs` → `panels/thumbs.rs` (1 hunk).
`panes.rs` was deleted upstream (renamed to `panels/`); nothing of ours was in it.

`Cargo.lock` auto-merged without conflict. Not hand-edited. `cargo check` did not rewrite it.

Post-merge chore: not needed. The merge brought in no `docs/brand/` or `.github/workflows/` files (`c0/main` had
already deleted them and upstream did not change the rest). Nothing uses `include_bytes!` on brand files.

## PATCH-DEBT

- Before: 68 added lines in 21 files (base `51eef99`; ratchet line said 69).
- After: 68 added lines in 25 files (base `55e5ee56`). Changes are file moves plus `app.rs`/`lib.rs` splits; no new hunks.
- Ratchet line set to 68 (equal to the total). `debt.py --working` passes (total 68 < 70).
- Committed-form `debt.py` (vs `c0/main`) fails until this branch is promoted; that is expected.
- Debt growth: none. The rule permits growth only for lines a moved upstream hook forces. None was needed.
- Deletions in upstream files (not counted by the ratchet): `app.rs` −25 (Backstage lifecycle relocation),
  `video.rs` −75 (H.264/HEVC adapters moved unchanged to `c0_patent_codecs.rs`, as before), `ribbon/slide_show.rs` −5.

## Tests

Host load before/after: 22.8/15.5/16.0 (first sample) → ~16 (later samples; varied while gate was saturated).
Disk peaked at 85%, under the 88% stop line.

| Crate | Command | Result |
|---|---|---|
| deckcraft-engine | `cargo test -p deckcraft-engine` (under gate) | lib 458 passed, 1 ignored; `c0_recovery` 1; `it` 6; `show_paragraph_builds` 8 |
| deckcraft-media | same | lib 23; `c0_codecs` 3; `it` 20 |
| deckcraft-ui-egui | same | lib 417 passed, 4 ignored; `c0_chrome` 10; `c0_host_contract` 21; `c0_media` 1; `c0_theme` 4; `c0_tools` 11 |
| deckcraft-web | `cargo test -p deckcraft-web --features page-control` | 4 passed (parser tests are behind the feature; default build has 0 tests) |

Upstream-red tests: none. No failing test, so no comparison against a `55e5ee56` worktree was needed.

## Wasm and clippy

- `cargo check --target wasm32-unknown-unknown -p deckcraft-web --features page-control`: passed.
- Release wasm size: not measured. `trunk build --release` was not run (heavy; disk was near the stop line). The 33554432-byte cap is untested.
- `cargo clippy -p deckcraft-ui-egui -p deckcraft-web -p deckcraft-media -p deckcraft-engine`: exit 0, 37 warnings.
  - 6 are deprecated egui API uses in our `c0_host` files (`egui::Id::new`, `egui::WidgetType`). egui is pinned at 0.37.0 both before and after the merge, so these predate the pull. Not proven by building `c0/main`.
  - 1 is upstream dead code (`SlideApp::opening_generation` is written in `opening.rs` and never read; the same on upstream `55e5ee56` and on `c0/main`).
  - The rest are the environment's unused `[patch]` entries and missing craft-fonts.

## New upstream commands and menu items

None. The `cmd!` command ids (139) and the UI command table (50) are identical between `51eef99` and `55e5ee56`.
This was checked by parsing macro ids, not by running the app.

## What upstream added (plain words)

Features and fixes from the 86 non-merge commits and 6 merge commits:

- Installer: a per-user ArtCraft folder is now the default (`feat(installer)`). Wizard scope and folder fixes,
  architecture-specific machine folder check, install-policy hardening, ARM64 review fixes (`fix(installer)`).
- Windows: shortcut icon kept after an update (#1114, #1391).
- Export speed: PDF export reuses equal slide backdrops (#1461), plus bench timing.
- UI: `craft-ui` ComboBox used for every dropdown (#1457). This is a visible change to dropdown controls.
- Build and CI: nextest 0.9.150 pin, dependency refreshes, the pinned craft-update.

No commit by subject is about a crash, freeze, save, or undo fix. The rest is refactoring: module splits for
`cmd/`, `ribbon/`, `dialogs/`, `panels/`, `canvas/`, `show/`, `icons/`, `app/`, and codec/clippy cleanups
(`S3776`, `S1612`, `S107`). Diffs were not read individually, so behaviour changes inside those refactors are unverified.

## Clashes with our UI

- Home and Account community links: patched in `backstage/files.rs` and `backstage/info.rs`. Verified by compile
  and by the `c0_chrome` and `c0_host_contract` tests. Not visually inspected.
- Ribbon: File tab and Learn More moved into the per-tab layout. Our `c0_host/tools.rs` mirrors `ribbon::show`'s reserved height; that mirror was not re-checked against the split layout by eye.
- Task pane: `panes.rs` renamed to `panels/`. `c0_host/tools.rs` uses `app.ui.pane`, which still exists. Not re-verified visually.
- Menus and palette: `menus.rs` unchanged upstream; our filter still applies.
- Theme: `theme.rs` unchanged upstream. The ComboBox refactor (#1457) could change dropdown colours; not checked.
- Agent server: not reviewed in this pull.

## Assumptions I could not confirm

- The UI was not run or screenshotted. Visual clashes above are unverified.
- "Upstream unchanged at default switch values" is supported by the test suite, not by a behavioural diff.
- The six egui deprecation warnings predate the merge (egui version is unchanged), but c0/main was not built to prove it.
- The release wasm size and cap were not measured.
- The ratchet line changed from 69 to 68, because the committed value never matched `debt.py`'s equality check.
- Upstream refactors inside the 86 commits were not reviewed line by line.
- The gate was saturated for about 50 minutes; it did not affect results.
