# Patch debt: C0deck guest page

Base: upstream `main` at `a31b990d32f2c8562f1588562f922960c3fde48b`. Count the **added**
column, not net changes. New files, including all host implementation/tests, do not enter this
upstream-file ratchet. No existing asset or dependency source was changed.

| File | Lines | What | Why | Offered upstream |
|---|---:|---|---|---|
| `apps/deckcraft-web/Cargo.toml` | 8 | Opt-in page feature, default codec forwarding, explicit browser DOM capabilities | Build the existing binary for the guest page | No: page transport; codec option proposed |
| `apps/deckcraft-web/src/main.rs` | 2 | Feature-gated page module, including native parser tests | K-41: native `cargo test` must compile | No |
| `apps/deckcraft-web/src/web.rs` | 7 | Retain the WebRunner, attach the control queue, lend picker filters, bound page drop reads | Unpainted control passes and bounded browser files without a second app | No |
| `crates/engine/Cargo.toml` | 4 | Forward codec defaults while permitting a disabled dependency | Preserve standalone defaults; disable codecs transitively | Proposed: packaging option |
| `crates/engine/src/cmd/insert.rs` | 1 | Check the codec policy before insertion | Return a clear app error instead of embedding restricted new media | Proposed: packaging option |
| `crates/media/Cargo.toml` | 7 | Default-on `patent-codecs`, optional video decoder deps, optional symphonia AAC | Page graph excludes H.264/HEVC/AAC decoders | Proposed: packaging option |
| `crates/media/src/audio.rs` | 1 | Policy at audio decode entry | Imported restricted streams also fail clearly | Proposed: packaging option |
| `crates/media/src/lib.rs` | 1 | Register the policy module | Share one policy across probing, decoding and insertion | Proposed: packaging option |
| `crates/media/src/player.rs` | 1 | Populate a decoder-error slot even for restricted muted media | Autoplay/silent video must report an error instead of a blank frame | Proposed: packaging option; clock unchanged |
| `crates/media/src/probe.rs` | 1 | Policy at probe entry | Restricted formats are not advertised as playable | Proposed: packaging option |
| `crates/media/src/video.rs` | 6 | Include unchanged adapters gated in their own file, gate match arms and decode policy | Exclude decoder references when disabled | Proposed: packaging option |
| `crates/ui-egui/Cargo.toml` | 5 | Forward codec defaults; depend on opt-out-capable engine/media | Native behavior unchanged, web feature propagation | Proposed: packaging option |
| `crates/ui-egui/src/backstage.rs` | 2 | Delegate Home/Account community links | Home and Account honor the host switch | No |
| `crates/ui-egui/src/control.rs` | 2 | One `c0.` dispatch arm; filter hidden `app.links` | Same control envelopes/queue | No |
| `crates/ui-egui/src/credits.rs` | 2 | Render contributor names as plain labels when links are hidden | Covers both grab-bag and table GitHub links | No |
| `crates/ui-egui/src/dialogs.rs` | 1 | About-link draw seam | Keep identity/credits/licence while removing links | No |
| `crates/ui-egui/src/lib.rs` | 10 | Host state, lifecycle hook, title draw seam, one `c0_host::bars` call that draws ribbon, status bar and the folded dock strip, guarded app.links, media preflight; relocate Backstage lifecycle | Keep host implementation in new files; reject restricted playback | No |
| `crates/ui-egui/src/menus.rs` | 1 | Filter the palette through host policy | Hidden app.links has no palette entry | No |
| `crates/ui-egui/src/ribbon.rs` | 2 | Gate File; delegate the existing Learn More group | No File/Backstage or promotional button when disabled | No |
| `crates/ui-egui/src/theme.rs` | 3 | Register overrides, one-line Tokens::get resolver, resolve widget styles on brightness changes | Runtime palette survives light/dark | Proposed: generic context-local palettes |
| `crates/ui-egui/src/thumbs.rs` | 1 | Consume host navigation scroll target | Selected slide is brought into the thumbnail viewport | Proposed: general navigation improvement |

Total: 68 added lines in modified upstream files.

Ratchet: 69

“Proposed” and “Yes” identify changes suitable/intended for upstream. This executor opens no PR,
sends no upstream messages and pushes nothing. The controller owns any actual submission.

Reproduce after the lane commit:

```sh
git diff --numstat --diff-filter=M main c0/main -- . ':!c0'
python3 c0/deckcraft-host/debt.py
```

## New files and relocated code

All page HTML/config/docs and browser smoke tooling live under `c0/deckcraft-host/`.
`apps/deckcraft-web/src/page_control.rs` and `page_control/*` own parsing, transport and the bounded
native browser chooser. UI host methods, chrome/picker/replacement state and settings live in
`crates/ui-egui/src/c0_host.rs` and `c0_host/*`; colours live in `theme/host.rs`.
`crates/media/src/c0_codecs.rs` owns the single format policy.

The existing H.264/HEVC adapters are relocated unchanged from `video.rs` to
`crates/media/src/c0_patent_codecs.rs`, included after `plane_pic!` with feature gates beside the
adapters in that new file. Upstream Backstage opening/sync
methods are relocated to `c0_host/chrome.rs`, with the host visibility guard before their existing
lifecycle. About links and the Learn More group are also drawn through that file. These moves
are intentional, visible deletions in the upstream diff, not independent replacement implementations.

Native regression tests are exclusively new `crates/*/tests/c0_*.rs` files. No sibling app code
was copied; GridCraft and WordCraft were read for transport/contract patterns only.

## Couplings to recheck on every upstream pull

- `c0_host/tools.rs` mirrors `ribbon::show`'s reserved height (`28 + 4` collapsed, else `RIBBON_H + 20`)
  and draws the unchanged `ribbon::show` inside a scaled/movable layer; `ribbon_collapsed` is forced
  on for the call when `toolbar` is off. The folded dock hides the task pane by taking `ui.pane`
  and restoring it on unfold. Recheck all three if `ribbon.rs` layout or `panes.rs` change.

- `SlideApp::logic` must drain the ordinary control queue inside `WebRunner`'s unpainted logic
  pass. The bridge listener and `AppRunner::logic` API are eframe-version-sensitive. Maintain
  the native parser gate and browser smoke with requestAnimationFrame/setTimeout disabled.
- Synthetic input still goes through `raw_input_hook`; screenshots need a presented frame.
  Recheck bounded inline passes and ordinary/deferred response timeouts if upstream changes them.
- Replacement is a continuation of the existing `close` dialog. Save/Don't Save must close the
  captured document only on success; cancelling/dismissing the dialog must retain it. Recheck
  notes drafts, document switching, Save-as service semantics and browser downloads.
- Picker cancellation uses both the context ticket and upstream `pending_files` identities.
  Retired DOM listeners must be detached before dropping their Rust closures. The browser owns
  the native chooser; removing the hidden input cannot close browser-owned UI.
- `open_checked`, `file.openBytes` and `file.new` own imports and creation. Keep importer damage
  reporting and read-only save snapshots intact. Recheck new importer formats, limits and reply keys.
- Slide ids are numeric; `c0.goTo.slide` prefers indices then numeric ids; string ids disambiguate them.
  Recheck `slide.go` selection behavior and the thumbnail-row scroll hook after pane changes.
- `Chrome` defaults must stay true and remain outside UiState serialization. Check all new
  Backstage entry points, About/profile links, palette enumeration and the Learn More group.
- All 23 Color32 fields must remain registered. Brightness application must resolve the same
  context override set for ordinary egui widgets as `Tokens::get` uses for custom painting.
- Optional codec dependency defaults must not re-enable H.264/HEVC/AAC in the isolated web graph.
  Check Cargo's resolved graph as well as both feature modes and the native media regressions.
  New containers/aliases must share `c0_codecs::check`; mixed streams must fail clearly too.
- Recovery entropy must not call native process/time APIs on wasm. The other process-id call
  in recovery.rs belongs to native unit-test scratch-directory construction, not document startup.
- The scout's `player::web_now` concern does not require a patch: only `wait_loaded` uses it,
  with a non-threaded early return on wasm. `Player::tick(now)` gets the egui host clock. Keep
  the native and real-browser playback-position checks when changing either path.
- `../craft-fonts` and the shared craftrules checkout are absent on this host. A future release
  that embeds fonts must rerun licence/asset gates and visual checks; this lane adds no fonts.
