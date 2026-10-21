# C0deck guest-page lane report

Branch: `c0/main`. Upstream base: `a31b990d32f2c8562f1588562f922960c3fde48b`.
No push, shared-branch commit, fleet redispatch, SSB logging or control-plane mutation was performed.

## Result

The existing DeckCraft web binary is a blank C0VIBE guest page with the opt-in `page-control`
transport. Its Promise bridge feeds the ordinary control queue and answers all eight `c0.*`
methods in unpainted egui passes. File snapshots are read-only; Open/New use the existing
unsaved-changes dialog. Browser file reads have cancellation and shared allocation bounds.

Five default-true chrome switches, eight panel settings and all 23 colour tokens are supported.
Community suppression includes contributor GitHub links in both layouts, About, Backstage,
Record's Learn More and the command palette. DeckCraft identity, authors and licence remain.

The page disables the default-on patent-codec feature. Its resolved wasm dependency graph has
no `deckcraft-h264`, `deckcraft-hevc` or `symphonia-codec-aac`; AV1, VP9, Opus, MP3, FLAC,
Vorbis and PCM/WAV remain. Restricted insertion and imported-media playback return the app's
clear unsupported-format error. Other builds retain their original defaults.

The scout's recovery concern was confirmed in Chromium before applying the one-line wasm
guard: startup panicked with `no pids on this platform`. The guarded build starts normally.
The other process-id call is native test scaffolding. `player::web_now()` needed no clock
patch: wasm's non-threaded `wait_loaded` returns before that polling loop, and playback uses
the host clock passed to `Player::tick(now)`.

## Commits

- `1c7d174b203d597dac76f615f04d50bc49957add` —
  `feat(c0deck): embed DeckCraft as a controlled C0VIBE guest`.
- This report is a separate `docs(c0deck): record guest-page validation and hand-back` commit.
  Both final commit hashes are emitted in the lane's stdout hand-back.

## Validation

The full native engine/UI/media/web run passed: **634 passed, 5 ignored, 0 failed**.
This includes 332 engine unit tests plus 7 integration tests, 216 UI unit tests plus 29 host
tests (chrome 10, contract 14, theme 4, imported-media policy 1), 46 media tests and 4 native
page-parser tests. The 5 ignored tests were already ignored upstream. Log:
`/tmp/c0deck-native-full-final-v3.log`.

Additional gates passed:

| Command | Result |
|---|---|
| `cargo fmt --all` and CI's fmt check | Passed |
| `cargo test -p deckcraft-web --features page-control` (K-41) | 4 passed, native compilation succeeded |
| `cargo check --target wasm32-unknown-unknown -p deckcraft-web --features page-control --no-default-features` | Passed |
| `cargo clippy -p deckcraft-ui-egui -p deckcraft-engine -p deckcraft-media -p deckcraft-web --all-targets --features deckcraft-web/page-control -- -D warnings` | Passed |
| `cargo clippy --target wasm32-unknown-unknown -p deckcraft-web --features page-control --no-default-features -- -D warnings` | Passed |
| `cargo xtask ci` | All 7 gates passed; workspace tests: 1,766 passed, 6 ignored, 0 failed |
| `cargo test -p deckcraft-ui-egui --no-default-features` | 245 passed, 4 existing ignored, 0 failed |
| `cargo test -p deckcraft-media --no-default-features -- --exact` with all six `c0_codecs` function names | 6 policy tests passed; 43 unrelated tests filtered; 0 failed |

CI also confirmed all 47 asset files are attributed, all 28 crates obey layering, and every
wasm-eligible crate checks successfully. Its six existing ignored cases are the engine/UI long
soaks, three manual screenshot writers and an H.264 speed benchmark.
Logs: `/tmp/c0deck-quality-final.log`, `/tmp/c0deck-codec-off-final.log`.
A fresh `cargo xtask ci` before the report commit also passed all seven gates; log:
`/tmp/c0deck-report-ci.log`.
The codec-disabled tests cover imported H.264/HEVC/AAC playback errors, muted-media error slots,
supported audio/video decoding, default feature semantics and the supplied playback clock.
All Cargo/Trunk work uses the requested
slot wrapper with `CARGO_BUILD_JOBS=2`; native tests also use `RUST_TEST_THREADS=2`. File-filtered
test invocations were automatically promoted to gate plus heavy admission by the wrapper;
its landing reservation and older build intent delayed those invocations. The required full
suites ran under the normal heavy admission. No reservation or host capacity was changed.

Chromium 155.0.8059.79 (CDP 1.3), headless WebGL/SwiftShader: **9 smoke groups passed**:

1. Blank one-slide startup, clean state, no process-id panic.
2. All eight `c0.*` methods, PPTX/native open/snapshot round trips, read-only snapshots,
   themes and chrome while `requestAnimationFrame` and `setTimeout` are disabled.
3. Real native chooser cancellation, DOM/listener retirement and inert late change events.
4. DeckCraft-generated H.264/AAC PPTX imports successfully; explicit Play returns the app's
   unsupported-format error.
5. H.264, HEVC, ADTS AAC and MP4 AAC insertion return the clear unsupported-format error.
6. MP3 and VP9 playback positions advance using the existing host clock.
7. Cancelling dirty Open/New preserves the document and revision, with the original Close dialog.
8. Malformed/unknown requests fail clearly; hidden `app.links` is neither executable nor listed.
9. A fresh unpaused page paints correctly, with zero JavaScript exceptions. Screenshot
   `/tmp/c0deck-browser.png` was opened and visually inspected: title/File/status chrome is absent,
   the accent override is visible, and About retains DeckCraft/credits/licence without links.

The smoke reloads before its final visual check because deliberately pausing RAF strands eframe's
scheduled frame callback. This avoids mistaking the prior slideshow framebuffer for current UI.
The codec-enabled comparison also passed browser startup and exported the restricted-media fixture.
Logs: `/tmp/c0deck-browser-final.log`, `/tmp/c0deck-browser-codecs-final.log`; original confirmed
regression: `/tmp/c0deck-browser-unpatched.log`.

## Build sizes and machine load

Both optimized release variants are measured from the same implementation; the committed
HTML selects only `page-control` with default features disabled. The codec-enabled comparison
is a separate local artifact in `dist/c0host-codecs`, never the delivered page in `dist/c0host`.
Sizes below count actual uncompressed files, not HTTP transfer compression.

| Variant | Wasm bytes | Total distribution bytes |
|---|---:|---:|
| Page, patent codecs OFF (`dist/c0host`) | 29,910,062 | 30,069,382 |
| Comparison, patent codecs ON (`dist/c0host-codecs`) | 30,599,935 | 30,759,255 |

Codec-enabled comparison build: `NO_COLOR=true trunk build --release --dist ../../dist/c0host-codecs`
under `smoac-slot run heavy 2 3600`, with two Cargo jobs and two Binaryen cores. It passed.
`cat /proc/loadavg` immediately before and after that build:

```text
before: 21.04 20.06 19.88 22/1736 2712889
after:  21.56 21.36 21.13 21/2274 3732971
```

Final codec-disabled page build: `NO_COLOR=true trunk build --release`, under the same slot,
Cargo-job and Binaryen-core limits. It passed. The wasm reduction is **689,873 bytes (2.25%)**.
The final distribution contains one hashed JS file (157,742 bytes), one hashed wasm file and
`index.html` (1,578 bytes). `cat /proc/loadavg` before and after the final page build:

```text
before: 19.01 20.94 20.99 30/2243 3691031
after:  12.15 15.02 18.08 8/2329 217962
```

## Upstream patch debt

The complete file-by-file added-line table, rationale and upstream-offer disposition are in
[PATCH-DEBT.md](PATCH-DEBT.md). Its verifier uses the requested added column of
`git diff --numstat --diff-filter=M main c0/main -- . ':!c0'`; new implementation/test files are
outside that modified-upstream-file ratchet. The measured total is **69**, below the target 70.

| Modified upstream file | Added lines |
|---|---:|
| `apps/deckcraft-web/Cargo.toml` | 8 |
| `apps/deckcraft-web/src/main.rs` | 2 |
| `apps/deckcraft-web/src/web.rs` | 7 |
| `crates/engine/Cargo.toml` | 4 |
| `crates/engine/src/cmd/insert.rs` | 1 |
| `crates/engine/src/recovery.rs` | 1 |
| `crates/media/Cargo.toml` | 7 |
| `crates/media/src/audio.rs` | 1 |
| `crates/media/src/lib.rs` | 1 |
| `crates/media/src/player.rs` | 1 |
| `crates/media/src/probe.rs` | 1 |
| `crates/media/src/video.rs` | 6 |
| `crates/ui-egui/Cargo.toml` | 5 |
| `crates/ui-egui/src/backstage.rs` | 2 |
| `crates/ui-egui/src/control.rs` | 2 |
| `crates/ui-egui/src/credits.rs` | 2 |
| `crates/ui-egui/src/dialogs.rs` | 1 |
| `crates/ui-egui/src/lib.rs` | 10 |
| `crates/ui-egui/src/menus.rs` | 1 |
| `crates/ui-egui/src/ribbon.rs` | 2 |
| `crates/ui-egui/src/theme.rs` | 3 |
| `crates/ui-egui/src/thumbs.rs` | 1 |
| **Total / Ratchet** | **69** |

## Fonts and assets

`../craft-fonts` does not exist on this host. Builds report that absence and embed an empty
optional Craft font list. Egui's default UI fonts and the fonts crate's existing Ubuntu Light
last-resort face remain. Browser presentation text has no system-font discovery or optional
Craft/CJK collection; fallback changes metrics and can leave glyphs unavailable. Typography
parity is not claimed. No fonts, vendor assets or external sample documents were added.
Media tests use the repository's existing CC0 fixtures. Browser PPTX fixtures are generated
by DeckCraft itself from those fixtures, never produced by Microsoft software.

## Assumptions I could not confirm

- The outer C0VIBE module/iframe integration is not present here. The standalone page contract
  and readiness alias are verified; deployment-specific host storage and mount behavior are not.
- The shared `../../craftrules` checkout, local `plan/STATUS.md` and private UI observations
  are absent. The attempted public shared-rules URL also returned 404. Available repo rules
  and executor skills governed this lane; no private observation images were invented or added.
- Headless Chromium exercises the WebGL path. Hardware-specific WebGPU behavior and other
  browsers/platforms are outside this host's smoke coverage.
- `template` currently means omitted, empty or `blank`; DeckCraft supplies no host template
  registry here. Unknown template names return an explicit error.
- Browser Save retains DeckCraft's download semantics. Completing a download is not an
  acknowledgement that the outer host has persisted bytes; hosts should use `c0.documentBytes`.

Reproduction commands, complete request/reply schemas, limits and upstream-pull couplings are
in [README.md](README.md) and [PATCH-DEBT.md](PATCH-DEBT.md).
