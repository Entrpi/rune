# Examples

Each folder here is a workspace package that `cargo run -p NAME` runs (`luau-panel` has two
binaries, so it takes `--bin luau-panel`). Start from `hello`, then copy the example closest to
what you're building. Rows marked *lab* are experiments and measuring rigs, not patterns to copy
whole.

Each command in the Headless column runs as written from the repo root, with no window and
Rune's fixed virtual clock (`site-scenes` has no app; it draws at the time `--t` gives), and
writes any file it makes under the current folder. "Agent tooling" in
[`crates/rune/README.md`](../crates/rune/README.md) lists the flags (`--snap`, `--tree`,
`--script` and the rest). "Builds for" lists the platforms each package is set up for: macOS,
Windows and Linux natively, and the browser for the three with a wasm entry point. The notes
under the tables say where a feature does less.

## Start here

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`hello`](hello) | The smallest app: one piece of state, a view of it, and a switch that flips it on a spring. | `AppSpec`, `rune::run`, `el`, `col`, `text`, `.motion`, `.on_press`, `role` and `checked`, scenes, a `Runtime` test | macOS, Windows, Linux | `cargo run -p hello -- --snap hello.png --scene on` |
| [`settings`](settings) | A small Settings window drawn entirely by Rune: switches, a segmented control, accent swatches and a speed slider, with scenes and input scripts. | `tok` themes and motion, `on_drag` with `Drag` (the slider), `tx.instant`, `tx.color_motion`, accessibility roles, `scripts/*.json` | macOS, Windows, Linux | `cargo run -p settings -- --script examples/settings/scripts/toggle.json --tree settings.json --at 1` |

## Lists and scrolling

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`todo`](todo) | A list with state: keyed rows you add, tick off and remove. Rows animate in and out and the progress bar springs to its new width. | `.key` on rows, `.scroll(ScrollSpec { stick_bottom: true })`, `Spring` in `.motion` | macOS, Windows, Linux | `cargo run -p todo -- --snap todo.png` |
| [`frosted-bar`](frosted-bar) | A long list scrolling under a frosted bottom bar that blurs the rows behind it each frame. | `El::backdrop_blur`, `ScrollSpec`, `.pin`, scenes `strong` and `plain` | macOS, Windows, Linux | `cargo run -p frosted-bar -- --snap bar.png --scale 2 --theme dark` |

## Motion

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`matched`](matched) | Shared-element motion: a picture tile flies between a 2×2 grid and the whole stage, both plain flex layout. | `El::matched`, `Transition::scale`, `Pixels`, `pixels_fit`, `on_key` | macOS, Windows, Linux | `cargo run -p matched -- --script examples/matched/scripts/zoom.json --snap matched.png --at 1` |
| [`odometer`](odometer) | A count and a run time as rolling-digit odometers, and header rows whose usage group slides as its numbers change length. | rune-kit `RollingText` and `Button`, `width_leads`, `Spring` | macOS, Windows, Linux | `cargo run -p odometer -- --snap odometer.png` |
| [`live-values`](live-values) | Sixteen rings at 60 Hz and four remote cursors at 120 Hz, moved by values a background producer sets without rebuilding the view. | `Live<T>`, `live_opacity`, `live_offset`, `Live::smooth`, a producer thread from `setup`, `--bench` | macOS, Windows, Linux | `cargo run -p live-values -- --snap live.png --at 2` |
| [`motion-bento`](motion-bento) *lab* | A 1440×900 control center whose twelve tiles all move at once under a seeded chaos schedule, for comparing with a GPUI build of the same window. | rune-kit, `Live`, `App::on_frame_encoded`, `--emit-script`, stress mode (`--load`, `--ramp`) built from `memo` and `Live` values (`--naive` rebuilds it every frame) | macOS, Windows, Linux | `cargo run -p motion-bento -- --seed 7 --snap bento.png --at 2` |

## Overlays and windows

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`background`](background) | An app that outlives its window: it starts with none, a background thread feeds it, and commands on stdin open, focus, close and quit it. | `App::keep_running`, `windowless`, `on_close`, `AppHandle`, `App::overlay` with `OverlaySpec`, `accessory`, `release_gpu_when_closed` | macOS, Windows, Linux | `cargo run -p background -- --snap background.png` |
| [`overlay`](overlay) | One control window and a transparent, click-through, always-on-top overlay per display, sharing one live remote cursor. | `run_windows`, `WindowSpec`, `WindowOptions::overlay`, `set_click_through`, `window_number`, `Live` | macOS, Windows, Linux | `cargo run -p overlay -- --window overlay-0 --snap overlay.png --at 2` |
| [`launcher`](launcher) | A notes app with a ⌘Space launcher panel: windows that `--desktop` records together, as a screen recording would show them. | `run_windows`, `WindowSpec::from_app`, `App::global_shortcut`, `WindowOptions::launcher_panel`, `AppHandle::place`, `show` and `focus`, `--desktop` | macOS, Windows, Linux | `cargo run -p launcher -- --desktop examples/launcher/backdrop.png --script examples/launcher/scripts/demo.json --snap launcher.png --at 2.5` |
| [`glass-panel`](glass-panel) | A floating card on a system material, built the way a native AppKit panel is: it flies across one big window, hands off to a card-sized window and fades out. | `El::system_material`, `App::overlay` with `OverlayPlacement::Rect`, `macos_level`, `srgb_blend`, `OverlayHandle::fade_in` and `fade_out`, `continuous_corners`, `Shadow::core_animation` | macOS, Windows, Linux | None: the card draws only in its overlay panels, and a headless run renders the windowless host, which is 1×1 pt |
| [`glass-card`](glass-card) *lab* | A system material card in a transparent, borderless window, flying over a busy test pattern in a second window and docking, to measure how the material keeps up with the frame. | `El::system_material`, `run_windows`, `MacWindowOptions::shadow`, `LoopSpec`, `RUNE_SYSTEM_MATERIAL` | macOS, Windows, Linux | None: both windows set their own `App::args`, so tooling flags don't reach them and the app opens its windows |
| [`overlay-lab`](overlay-lab) *lab* | A transparent pill on the display under the pointer, where only its chips take clicks, and an area picker on every display, for testing overlay input, levels and capture exclusion. | `run_windows_with`, `OverlayOptions` (placement, level, input, keys, `prewarm`, `exclude_from_capture`), `OverlayInput::Regions`, `.hit()`, `OverlayHandle` | macOS, Windows, Linux | `cargo run -p overlay-lab -- --scene picker --size 960x600 --snap picker.png` |

## Text and Markdown

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`kit-gallery`](kit-gallery) | Every rune-kit component, one scene per `@opencode/ui` Storybook story, with Markdown that streams in and Mermaid diagrams. | rune-kit `Kit` and its components, `Markdown` (`streaming`, `reveal`), Mermaid, `rune_theme_opencode`, `Kit::from_palette`, `--list-scenes` | macOS, Windows, Linux | `cargo run -p kit-gallery -- --scene rune-markdown--streaming --snap markdown.png` |
| [`rich-clipboard`](rich-clipboard) | Copy and paste formatted text: a panel shows which forms (text, HTML, RTF) a paste brought. | `ClipboardContent`, `tx.copy_rich`, `tx.read_clipboard`, `on_input_paste`, `text_input`, `clipboard::memory` in tests | macOS, Windows, Linux | `cargo run -p rich-clipboard -- --snap clipboard.png` |

## Drag and drop

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`drag-out`](drag-out) | Drag a file, an app bundle or a line of text out of a window into another app. | `El::drag_out`, `DragItem`, `on_drag_start`, `on_drag_end`, `DragOutcome` | macOS, Windows, Linux | `cargo run -p drag-out -- --tree drag-out.json` |
| [`drop-files`](drop-files) | Drop files into a window from another app: one zone takes pictures only, the other anything, and the window takes the rest. | `on_drop_files`, `on_drop_hover`, `accepts_extensions`, `drag_files` and `drop_files` script steps | macOS, Windows, Linux | `cargo run -p drop-files -- --script examples/drop-files/scripts/drop.json --frames 90 --every 6 --cols 5 --strip strip.png` |

## Media

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`video-player`](video-player) | Play a movie, an audio file or a URL, and tap to pause. With no argument it plays a four-frame test clip. | `Player`, `El::video`, `SurfaceFit`, the `setup` waker, `poll` with `tx.wake_after` | macOS, Windows, Linux | `cargo run -p video-player -- --snap video.png --at 0.6` |
| [`video-tiles`](video-tiles) | Four video tiles fed by a background producer: the producer/view boundary to copy into a VideoToolbox app. | `NativeSurface`, `El::native_surface`, `ByteFrame`, `push` and `push_bytes`, `set_waker`, `--bench-native` | macOS, Windows, Linux | `cargo run -p video-tiles -- --snap tiles.png --scale 2` |

## Drawing and effects

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`shapes`](shapes) | Vector shapes: a filled waveform envelope, polylines, meters, a Bézier path and a concave polygon. | `canvas`, `Shape`, `Path` (`cubic_to`, `quad_to`), `LineCap`, `view_box`, `poll` driving time | macOS, Windows, Linux | `cargo run -p shapes -- --snap shapes.png --at 1.5` |
| [`sketch`](sketch) | A pencil cursor over a canvas: drag to draw. | `Cursor::Image`, `CursorImage::png`, `canvas`, `on_drag` with `Phase` | macOS, Windows, Linux | `cargo test -p sketch` |
| [`cursed-settings`](cursed-settings) | A settings window where every control does what its label says: the Firewall switch builds a wall of fire. rune-fx and rune-light draw inside Rune's frame through the compositor hook. | `rune::run_with` and a compositor, rune-fx, rune-light, rune-kit, `rune_theme_opencode` | macOS, Windows, Linux | `cargo run -p cursed-settings -- --scene fire --snap fire.png --at 1` |
| [`showreel`](showreel) | 960×540 scenes for Rune's showcase video. Each animates itself from the clock, so a headless `--video` run records it frame-exact. | `App::compositor`, rune-fx, `poll` reading the virtual clock, scenes, `--video` | macOS, Windows, Linux | `cargo run -p showreel -- --scene deploy --snap deploy.png --at 2` |
| [`site-scenes`](site-scenes) | The material stills on Rune's home page (liquid metal, glass and light), drawn with Rune's renderer and no app or window. It only runs headless. | `render::{Gpu, Offscreen, Renderer, write_png}`, `TextSystem`, rune-fx `FxRenderer`, rune-light `LightRenderer` and `GlassRenderer` | macOS, Windows, Linux | `cargo run -p site-scenes -- --only metal` (writes `target/site-scenes/metal.png`) |
| [`lab`](lab) *lab* | rune-light and rune-fx composited live inside Rune's renderer, with knobs: a lamp, a flashlight, global illumination, keys, god rays, glow, pulses, electricity, fire, a thrown card and a lens. | `App::compositor`, rune-light, rune-fx, rune-motion `Animated` and `VelocityTracker`, `App::modifier_keys`, scenes | macOS, Windows, Linux | `cargo run -p lab -- --scene lamp --snap lab.png` |

## Tuning and stories

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`playground`](playground) | A small preferences window with a playground (⌘⇧D): scenes, a live page, a story with its own state and timer, and an overlay, with variants and a side-column control. | `App::playground`, `Playground`, `Story`, `Story::overlay`, `Stage`, `Play`, `--playground`, `--variant` | macOS, Windows, Linux | `cargo run -p playground -- --snap playground.png --playground "Upload"` |
| [`knobs`](knobs) | A card tuned live with rune-knobs: ⌘⇧K slides a panel of sliders and choices in beside the page. | rune-knobs `Knob`, `Knobs`, `Group` and `Preset`, `rune_knobs::side`, `rune_knobs::inline` in a playground side column | macOS, Windows, Linux | `cargo run -p knobs -- --snap knobs.png --scene open` |

## Scripting

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`luau-panel`](luau-panel) | A Rune app whose UI is a Luau script: a header with buttons, a pulse driven by `task.wait`, a signal and a 1,000-row virtual list. Saving a `.luau` file reloads the panel in place, and `luau-bench` measures the performance gates. | rune-luau `Script`, `Sources`, `ScriptOptions`, `Script::view`, `Script::poll`, `Script::set_waker`, `Script::watch`, `ui.virtualList` | macOS, Windows, Linux, web | `cargo run -p luau-panel --bin luau-panel -- --script examples/luau-panel/demo.json --snap luau.png --at 2` |

## Web

| Example | What it teaches | Main APIs | Builds for | Headless |
|---|---|---|---|---|
| [`web-hello`](web-hello) | A scrollable message list with a composer, Send and Copy last, from one crate for the browser and the desktop. | a `cdylib` with a `wasm_bindgen(start)` entry, `scripts/web-build.sh`, `cx.safe_area`, `text_input`, `tx.scroll_to_bottom`, `tx.copy`, `rune::web::open_url` | macOS, Windows, Linux, web | `cargo run -p web-hello -- --snap web-hello.png` |
| [`web-showcase`](web-showcase) | The live panel on Rune's website: a switch, a spring picker and a puck you throw along a track, drawn on a canvas the page already has. | `App::canvas_id`, `fit_content_height`, `try_run`, `on_drag` with `Drag`, `Cx::layout_size`, `tx.instant` | macOS, Windows, Linux, web | `cargo run -p web-showcase -- --snap poster.png --size 560x320 --scale 2` |

## Notes

- `web-hello`, `web-showcase` and `luau-panel` have a wasm entry point. `scripts/web-build.sh`
  builds them for the browser (see [`WEB.md`](../WEB.md)); `luau-panel` also needs WASI SDK
  (`WASI_SDK_PATH`) for Luau's C++. `web-showcase` mounts on a page's `<canvas id="rune-live">`.
  The script can build another example's `main` too, but no other example is set up or checked
  there. None of these is an iOS app: on iOS, a host app embeds Rune through `embed::LayerHost`
  (see `crates/rune/examples/embed_cost.rs`).
- `drag-out`: `drag_out` works on macOS only. Elsewhere the tiles keep their press.
- `drop-files`: file drops arrive on macOS, Windows and X11. Wayland reports no file drags.
- `glass-card`, `glass-panel`: the system material is a native view on macOS only. Other
  platforms, opaque windows and headless renders draw the element's fill instead.
- `video-player`: clips play on macOS only. Other platforms report the clip as failed and draw a
  grey placeholder.
- `video-tiles`: macOS imports IOSurfaces; other platforms upload byte planes to the same shader.
- `rich-clipboard`: only macOS copies RTF, and reads a pasted RTF as HTML. Windows, Linux and
  the web carry text and HTML.
- Examples with tests also run with `cargo test -p NAME`: `drag-out`, `drop-files`,
  `frosted-bar`, `hello`, `kit-gallery`, `knobs`, `matched`, `motion-bento`, `rich-clipboard`,
  `settings` and `sketch`. `luau-panel` has no tests; its `luau-bench` binary prints its checks
  beside the performance numbers.
- Smaller probes and benchmarks of single features live in `crates/rune/examples`
  (`cargo run -p rune --example NAME`).
