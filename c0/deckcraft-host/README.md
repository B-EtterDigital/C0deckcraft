# C0deck: DeckCraft as a C0VIBE guest page

This page builds the existing `deckcraft-web` binary with `page-control`, with default features
disabled. It starts with DeckCraft's blank presentation (`?blank`), including its one empty title
slide. The surrounding HTML stays transparent until egui paints. It adds no second application,
server, disk API or cross-origin message listener.

From this directory:

```sh
smoac-slot run heavy 2 3600 -- env CARGO_BUILD_JOBS=2 BINARYEN_CORES=2 NO_COLOR=true trunk build --release
```

Serve `../../dist/c0host/` over HTTP. `Trunk.toml` uses relative URLs and hashed filenames. Chromium
smoke coverage is reproducible with `python3 c0/deckcraft-host/smoke.py` from the repository root.
Like the GridCraft page, the host alias stays undefined until Rust installs the underlying bridge.
Wait for `typeof window.__c0hostRequest === 'function'` before calling it.

## Page function and envelopes

```js
window.__c0hostRequest = (json) => window.__deckcraftControl(json);
const reply = JSON.parse(await window.__c0hostRequest(JSON.stringify({
  method: 'c0.inspect', params: {}
})));
```

Input is a JSON **string**, never an object. Output is a Promise of a JSON string. Optional request
ids are ignored: the Promise associates the reply with its request. Replies use DeckCraft's own
control envelope: `{"ok":true,"result":...}` or `{"ok":false,"error":"..."}`. Missing or null
params mean `{}`; other non-object params, malformed JSON and unknown methods return errors.

Ordinary requests are capped at 1 MiB; `c0.*` requests at 100 MiB. Files are capped at 72 MiB before
base64 encoding, filenames at 255 UTF-8 bytes. Filenames must be basenames: empty names, NUL, `/`,
`\`, `.` and `..` are errors. `ui.click` accepts at most 256 clicks and `ui.drag` at most 32 steps
(the underlying DeckCraft handlers may clamp further). There are at most 32 deferred requests;
deferred replies time out after 10 seconds and unclaimed commands are abandoned, preventing late
execution. Limits live in `c0_host` and `page_control`, shared with the chooser.

## When a request is answered

The bridge feeds `SlideApp::with_control`, the same queue used by the desktop control server.
It borrows an unpainted egui pass from eframe's event listener inside the call. Every `c0.*` reply
can therefore resolve while animation frames and timers are paused. Synthetic pointer/key input
gets up to 40 further passes to reach widgets. Screenshots additionally need an actual presented
frame; the ordinary DeckCraft screenshot timeout/error behavior still applies.

Open/New prepare a replacement and call the existing Close unsaved-changes path. A dirty document
remains intact and replies `{"pending":"saveChanges"}`; DeckCraft presents its existing Save /
Don't Save / Cancel dialog. Successful Close completes the pending replacement on the next logic
pass. Cancel or a failed/cancelled desktop Save retains the original and cancels replacement.
The browser's existing Save behavior is a download, after which its Close dialog proceeds. The
page does not interpret that download as a host-storage acknowledgement. Use `c0.documentBytes`
to take a read-only persistence snapshot for the surrounding host.

One replacement may be pending at a time. Other dialogs/background opens must finish first.
Invalid imports are rejected before showing a replacement prompt. Open uses all engine importers:
native `.deckcraft` (and `.slidecraft` alias), PPTX/POTX/PPSX, text outlines and Markdown. Encrypted
presentations are rejected with the engine's own error; this bridge opens no password dialog.

## Host methods

Names are case-sensitive. Slide indices are zero-based; numeric ids are stable within a document.

| Request | Params | `result` |
|---|---|---|
| `c0.inspect` | `{}` | `{title,dirty,revision,slides,currentSlide,slideCount,sections,canUndo,canRedo,panes,ribbonCollapsed,showing,theme}` |
| `c0.goTo` | `{slide:indexOrId}`, `{slide:"id"}` or `{slide:{id:number}}` | `{index}`; selects through `slide.go` and scrolls the thumbnails to that slide |
| `c0.documentBytes` | `{format?:"pptx"\|"deckcraft"}` | `{name,format,bytes,revision}`; default PPTX, base64 bytes, no Save side effects, dialogs or downloads |
| `c0.openFile` | `{name,bytes}` | `{document,missingFonts,damagedMedia,lostSlides}`, or `{pending:"saveChanges"}` |
| `c0.newDocument` | `{template?:"blank"}` | Existing `file.new` result (`{document,slides}`), or `{pending:"saveChanges"}`; empty/omitted template also means blank |
| `c0.cancelFileDialog` | `{}` | `{cancelled:boolean}`; retires the picker and outstanding application file tickets |
| `c0.show` | `{action:"start"\|"end",from?:index}` | `{showing:boolean}`; page-local reading show with no presenter window; default start follows the existing show settings |
| `c0.set` | `{themeTokens?,panels?}` | `{applied:[key],unknown:[key],invalid:[key]}` |

`title` is `DocState::title()`. `dirty` and `revision` come directly from `DocState`. Each slide is
`{index,id,title,hidden,section?}`, with the section name present when assigned. Each section is
`{name,first,count}`; `first` is null for an empty section. With no document, title/currentSlide
are null, revision/slideCount are zero and slides/sections are empty. `canUndo`/`canRedo` reflect
the engine's history. `panes` is `{notes,ruler,thumbnails}`. `theme` is UI brightness, `light` or
`dark`, independently of the presentation's document theme. `showing` is a boolean.

Navigation numbers prefer valid indices, then fall back to matching slide ids. Use a string id or
`{id:number}` when the same number could also be a valid index.

File payloads are base64 text without a data URL prefix. Snapshotting preserves dirty state,
revision, selection and history; it does not mark the document saved. Taking a snapshot of a notes
editor draft follows DeckCraft's model snapshot semantics: the draft is committed when focus
leaves the editor. Open/New commit any current notes draft before checking for unsaved work.

`cancelFileDialog` does not dismiss the unsaved-changes prompt. The browser owns its native
chooser window, which a page cannot close. A late chooser answer is ignored after cancellation.
Open/New also retire older chooser requests. Chooser files are size-checked before `arrayBuffer`
allocation, and pending inbox bytes share the same 72 MiB bound.
Page drop reads use the same limit and share an in-flight read budget with the chooser. Standalone
builds without `page-control` retain their existing drop behavior.

## Chrome settings

All five `Chrome` fields default to true, preserving upstream behavior and remaining outside
serialized preferences/documents. The host can change them at runtime:

```json
{"method":"c0.set","params":{"panels":{"title_bar":false,"file_tab":false,"start_screen":false,"community_links":false,"status_bar":false,"ribbon_collapsed":true,"notes":false,"ruler":false,"toolbar_double":true,"right_dock_collapsed":true}}}
```

| `panels` key | Meaning |
|---|---|
| `title_bar` | Draw DeckCraft's title bar |
| `file_tab` | Draw File and allow Backstage entry through commands/shortcuts |
| `start_screen` | Allow Backstage with no presentation open |
| `community_links` | Draw community/website/GitHub links, contributor profile links and Record's Learn More; expose `app.links` in commands/palette |
| `status_bar` | Draw the bottom status bar |
| `ribbon_collapsed` | Existing ribbon-collapse state |
| `notes` | Existing notes-pane state |
| `ruler` | Existing ruler state |
| `toolbar` | Default true. False folds the ribbon's command card to the tab row; tab clicks cannot bring it back while the host holds it off |
| `toolbar_double` | Default false. True draws the ribbon compact (scaled to 0.8) in the same dock |
| `toolbar_floating` | Default false. True lifts the ribbon into a movable strip over the canvas area (drag its grip row). Combines with `toolbar_double` |
| `right_dock_collapsed` | Default false. True folds the task pane to a narrow icon strip (one icon per pane, plus Unfold); the open pane is remembered and returns on unfold. An icon, or any other way of opening a pane, unfolds the dock |

All values must be booleans. Unknown keys are reported as `panels.<key>`; wrong types as invalid.
The four tool keys are host state: they are not saved in decks or preferences and every default is the
upstream layout. `c0.inspect.panes` reports `toolbar`, `toolbar_double`, `toolbar_floating` and
`right_dock_collapsed` beside `notes`, `ruler` and `thumbnails`; `toolbarFloat.pos` is the strip's
corner in points (`null` until it is moved). Code: `crates/ui-egui/src/c0_host/tools.rs`; tests:
`crates/ui-egui/tests/c0_tools.rs`.

About always retains DeckCraft's name, authors/contributors and MIT OR Apache-2.0 licence.
Disabling File also closes Backstage immediately; it cannot reopen until reenabled.

## Runtime theme tokens

`themeTokens` **replaces** the complete override set. `{}` or null clears it. Accepted colours
are `#rrggbb` and `#rrggbbaa`, case-insensitive. Valid siblings apply while unknown names and bad
values are reported as `themeTokens.<key>`. A malformed container causes an error before changes.
Unknown top-level settings are reported by their own key. Overrides are context-local, apply
to ordinary egui widget visuals too and survive `view.dark`/`ui.set {dark}` changes.

All 23 Color32 names are supported:

```text
chrome card card_border canvas text text_dim text_faint border separator hover
pressed selected accent accent_text contextual selection handle_fill handle_stroke
guide caret text_selection field_bg shadow
```

The non-colour `dark` field is not a token. To switch brightness use the ordinary control method
`ui.set {dark:true|false}`. The presentation's own theme remains under engine commands.

## Ordinary DeckCraft control methods

The entire existing control handler is available through this transport. See
[`docs/control-protocol.md`](../../docs/control-protocol.md) for parameter/reply details.
`engine.execute` / `command` / `ui.menu.invoke` run existing commands; `engine.commands` lists
engine/UI commands. `document.inspect`, `ui.inspect`, `ui.pointer`, `ui.key`, `ui.text`, `ui.ime`,
`ui.tool.select`, `ui.move`, `ui.click`, `ui.drag`, `ui.set`, `ui.dialog.open`, `ui.dialog.close`,
`ui.screenshot`, `ui.render`, `ui.resize`, `ui.focus`, `show.start`, `show.end`, `app.open`,
`app.save`, `app.export` and `app.quit` retain upstream replies. Disk-only methods need services
the page does not supply and return the upstream errors. Save/Export continue to use downloads.
The host family supplies dialog-free byte persistence and page-local slideshow control.

## Codec build and fonts

`patent-codecs` defaults ON in media and web; the engine/UI defaults forward it. Unmodified
standalone commands keep the original H.264, HEVC and AAC behavior. This page passes `--no-default-features`, leaving
those decoder dependencies out of the web graph. AV1, VP9, Opus, MP3, FLAC, Vorbis and PCM/WAV
remain available. Inserting or decoding restricted streams returns DeckCraft's unsupported-format
error containing “this format is not supported here”; mixed-codec containers are refused too.
Existing files can preserve such embedded bytes, but cannot play them in this build. Explicit Play
is rejected before queuing playback; muted/autoplay voices also retain a clear decoder error.
The existing host clock is unchanged.

The configured `../craft-fonts` checkout is absent here. The build warns and produces an empty
`CRAFT_FONTS` array. The UI uses egui's default fonts; `crates/fonts` still embeds its existing
Ubuntu Light last-resort face through `epaint_default_fonts`, already covered by ATTRIBUTION.md.
Browser presentation rendering has no system font discovery or optional Craft presentation/CJK
fonts. Requested fonts fall back to Ubuntu where no imported embedded face is available, changing
metrics and potentially missing glyphs; pixel and typography parity is not claimed. No font assets
were added.

## Branches, tests and upstream pulls

`main` remains upstream `a31b990d32f2`. All lane commits belong to `c0/main`, with no push.
Native host tests live in `crates/ui-egui/tests/c0_*.rs`; codec tests in
`crates/media/tests/c0_codecs.rs`. `cargo test -p deckcraft-web --features page-control` builds
the parser natively. Wasm checks use `--features page-control --no-default-features`.

Read [PATCH-DEBT.md](PATCH-DEBT.md) for the added-line ratchet, relocated methods and coupled
upstream internals to recheck when pulling. [LANE-REPORT.md](LANE-REPORT.md) records exact gates,
browser evidence, build sizes, load readings and unconfirmed assumptions.
