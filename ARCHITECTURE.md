# Architecture

This is a map of how Rune is put together: the crates, what one frame does from an OS event to
a presented drawable, which thread runs what, which struct owns which state, where hosts plug
in, and the invariants the commit keeps. It names files and functions so you can find them.
Features are described in `crates/rune/README.md`; direction and goals are in `PLAN.md`.

## Bird's eye view

An app is a state type `S` and a view, `fn(&S, &mut Cx<S>) -> El`. When state changes, Rune runs
the view, flattens the `El` tree into keyed nodes, lays them out with Taffy at their target
state (once per commit in the common case), and hands each node's target to a
`rune_motion::Scene`. Each frame samples the scene's tracks and builds a list of 128-byte
`Instance`s, which one instanced uber pipeline draws.

Everything for one `Runtime` runs on the thread that calls it: input, handlers, the view, layout,
animation, paint and recording the GPU frame. In a window that is the main thread, where winit's
event loop runs. There is no render thread. The one handoff is a macOS overlay's plain frame on
wgpu's Metal backend, which a present thread finishes, submits and presents once it is recorded; on
Rune's own Metal backend, the default, it presents on the caller. Other threads decode, talk to the
OS, run an app's `rune::task` work or do tooling work, and hand their results back through wakes.

## Crates

| Crate | Workspace dependencies (dev-dependencies left out) | Holds |
| --- | --- | --- |
| `crates/rune-motion` | none | springs, tracks, presence and `Scene`: data and math, no threads |
| `crates/rune-taffy` | none (Taffy 0.15) | `LayoutStore` (aliased `LayoutEngine`) and the 64-byte `CompactStyle` over Taffy's solver |
| `crates/rune-staging` | none | per-frame GPU uploads through reused staging memory |
| `crates/procwatch` | none | an app's own memory and CPU |
| `crates/rune-highlights` | none | Pierre's Highlights, the code highlighter OpenCode's desktop app draws with: its lexers for 80 languages, translated from WebAssembly to Rust at build time |
| `crates/rune` | rune-motion, rune-taffy, rune-staging; procwatch with `frame-profile` | views, runtime, text, renderer, windows, overlays, platform code, headless tooling |
| `crates/rune-kit` | rune, rune-motion, rune-highlights; rune-mermaid with `mermaid` | components that read semantic tokens (`Kit`) |
| `crates/rune-mermaid` | rune | Mermaid diagrams drawn with Rune shapes and text |
| `crates/rune-knobs` | rune | live tuning values and their panel |
| `crates/rune-luau` | rune | Luau scripts that build Rune UI: one sandboxed VM per `Script`, natively and on the web |
| `crates/rune-ref` | rune with `gpu` | scalar reference renderer for the instance stream |
| `crates/rune-fx` | rune-motion, rune-staging | effect layers |
| `crates/rune-light` | rune-staging | lighting |
| `crates/rune-i18n` | none | message catalogs generated from JSON, CLDR plurals, numbers and lists, the system's languages |
| `crates/rune-notify` | none | system notifications |
| `crates/rune-mac` | rune-notify | small macOS helpers |
| `crates/rune-perf` | procwatch | bounded local performance recording |
| `crates/rune-register` | none | one-line self-registration lists |
| `themes/opencode` (`rune-theme-opencode`) | rune, rune-kit | a rune-kit theme |

`examples/` and `www/app` are apps built on these. Inside `crates/rune/src`: `runtime.rs` and
`runtime/` hold the retained tree, commit, motion, input and paint; `window.rs` and `window/`
the native windows (specs, titlebars, pacing, drawing); `overlay.rs` and `overlay/` overlays;
`input.rs` and `input/` OS input (cursors, key chords, input arrival times, pointer helpers);
`image.rs` and `image/` decoding and the image atlas; `text.rs` and `text/` fonts, shaping and
glyph rasterising; `render.rs` and `render/` the GPU side; `gpu.rs` and `gpu/` the GPU backends
(Rune's own Metal one on macOS); `platform/` the per-OS code; `tooling.rs` and `tooling/`
headless runs, traces, scripts and the virtual desktop.

## One frame

A desktop window, from an OS event to a presented drawable.

1. **Input.** winit calls `Windows::window_event` (`crates/rune/src/window.rs`), which finds the
   host, opens the event's arrival-time scope (`input::time::arrived`,
   `crates/rune/src/input/time.rs`) and calls `Host::handle_window_event`
   (`crates/rune/src/window/events.rs`). That calls a `Runtime` input method such as
   `Runtime::pointer_move` (`crates/rune/src/runtime/input/pointer.rs`), which hit-tests the last
   frame's rects and queues an `Action` on `Runtime::queue`. The host asks for a redraw. Ordinary
   handlers wait in the queue for the frame. A few run at once: a pointer move past a `drag_out`
   element's threshold calls its item supplier (`Runtime::start_drag_out`, same file), and the
   window hooks `App::on_focus` (`Runtime::window_focus`, `crates/rune/src/runtime/frame.rs`),
   `App::on_visibility` (`Runtime::set_live_visible`) and `App::on_close` (`Host::close`).
2. **Wakes from other threads.** `AppHandle::update` (`crates/rune/src/app.rs`) queues on the app's
   `Shared` and sends `WakeEvent::App`. `Host::handle_user_event` (in `events.rs` too) calls
   `Host::drain` (`crates/rune/src/window/lifecycle.rs`), which runs each update through
   `Runtime::update_if`. A `Live` value wakes the loop and the frame latches it; a `Latest` answer
   wakes it and the app's `poll` takes the answer.
3. **Frame start.** `WindowEvent::RedrawRequested` reaches `Host::paint`
   (`crates/rune/src/window/events.rs`), and on macOS `WakeEvent::DisplayTick` from the display
   link (`crates/rune/src/platform/display_link.rs`) reaches `Host::redraw`; both call `Host::draw`
   (`crates/rune/src/window/draw.rs`). A covered or minimised window steps without drawing
   (`Host::covered_frame`, same file). Otherwise the frame acquires the drawable first (`acquire`,
   `crates/rune/src/window/pacing.rs`); a frame that only a timer asked for builds first, and takes
   no drawable when it would draw what is already on screen (`Presented::unchanged`,
   `crates/rune/src/window/presented.rs`). `FrameClock::advance` (`crates/rune/src/frame/clock.rs`)
   turns the frame's target time into a `catch_up` and a `dt` and calls `Runtime::frame_split`
   (`crates/rune/src/runtime/frame.rs`).
4. **Handlers.** `Runtime::frame_step` first steps motion already running by `catch_up`
   (`catch_up_motion`): `split` in `crates/rune/src/frame/clock.rs` caps the gap since the last
   frame at `MAX_GAP` and keeps back a last step of at most `RESUME_STEP`. It then calls the app's
   `poll` and due timers, runs the queued actions in arrival order (`Runtime::run_action`,
   `crates/rune/src/runtime/input/action.rs`), except that an OS drag's end goes after its own
   start (`order_drag_ends`) and file steps aimed at a tree that changed wait a frame
   (`hold_files`), and takes what the handlers asked of the shell (`latch_requests`).
5. **Commit**, only when state or the viewport changed, a layout read moved or a virtual list's
   window shifted. `Runtime::commit_for` (`crates/rune/src/runtime/commit/watch.rs`) calls
   `Runtime::commit` (`crates/rune/src/runtime/commit.rs`). `measure_commit`
   (`crates/rune/src/runtime/commit/relayout.rs`) picks one pass: `Measurement::relayout` when only
   the viewport changed, `Measurement::remount_virtuals`
   (`crates/rune/src/runtime/commit/remount.rs`) when only a virtual list's window moved, else
   `Runtime::measure` (`crates/rune/src/runtime/commit/measure.rs`). The shortcuts need the last
   tree's nodes to be reusable and fall back to a full pass. A full pass runs the view
   (`Measurement::root_view`), builds index-built rows (`Measurement::build_tree`), flattens `El`
   into `Node`s and Taffy slots (`flatten_with_buf`, `crates/rune/src/runtime/commit/flatten.rs`)
   and solves layout (`Measurement::solve`, in `relayout.rs`, which measures text in
   `compute_layout`, `crates/rune/src/runtime/commit/text_layout.rs`). A view that sets a new zoom
   (`Cx::zoom`) is measured again at once. `Runtime::settle_layout_reads`
   (`crates/rune/src/runtime/layout_read.rs`) measures again when a `Cx::layout` read moved.
   `Runtime::apply` (`crates/rune/src/runtime/commit/apply.rs`) then retargets the `Scene`, growth
   stacks, reveals and scroll viewports from the `Measured` result.
6. **Motion and paint.** `Runtime::step_motion` steps all motion, the handlers' new tracks
   included, by the last `dt`, and runs `Scene::frame` (`crates/rune-motion/src/scene.rs`),
   `Runtime::step_growth` and `Runtime::step_scroll` (`crates/rune/src/runtime/motion/tracks.rs`)
   and the reveals. It calls the app's `El::on_scroll_offset` callbacks for viewports that moved
   (`Runtime::tell_scroll_offsets`, `crates/rune/src/runtime/scroll/watch.rs`). `Runtime::build`
   (`crates/rune/src/runtime/paint/build.rs`) then fills `Runtime::instances`.
   `Runtime::report_layout` (`crates/rune/src/runtime/layout_read.rs`) queues `El::on_layout`
   callbacks, which run next frame.
7. **GPU.** Back in `Host::draw`: `Renderer::upload_images` and `Renderer::prepare_surfaces`
   (`crates/rune/src/render/upload.rs`), then the app's `Compositor::render`
   (`crates/rune/src/compose.rs`) or else `Renderer::prepare` (`crates/rune/src/render/upload.rs`)
   and `Renderer::draw` (`crates/rune/src/render.rs`), then `Renderer::submit` and wgpu's
   `Queue::present`. The first window's first frame records its build, encode and present in
   `rune::marks` (`crates/rune/src/marks.rs`), for an app's launch trace.
8. **Rest or next.** `FrameClock::presented` holds what the frame started for however late it
   presented, so the next frame moves it about one frame. `Presented::settle`
   (`crates/rune/src/window/presented.rs`) decides whether the window goes idle. If not, the
   host asks for the next frame: the display link on macOS, `request_redraw` elsewhere.
   `Windows::about_to_wait` sets winit's control flow from each host's `AnyHost::wake_deadline`,
   so an idle window sleeps until an event or a timer.

An overlay takes the same path through `Overlay::draw_frame` (`crates/rune/src/overlay/draw.rs`)
and `overlay::frame::step` (`crates/rune/src/overlay/frame.rs`). The browser host calls
`FrameClock::advance` from `Browser::window_event` (`crates/rune/src/platform/web.rs`), an
embedded view from `LayerHost::frame` (`crates/rune/src/platform/embed.rs`), and a headless run
(`tooling::headless`, `crates/rune/src/tooling/headless.rs`) calls `Runtime::frame` with a
virtual clock.

## Threads

The calling thread runs everything above. Every other thread `crates/rune/src` starts with
`std::thread` outside tests is listed here (31 spawn sites; `warm` has two).

| Thread | What runs on it | Spawn site |
| --- | --- | --- |
| the caller (main thread in a window) | the winit loop, every `Host` and `Overlay`, every `Runtime`: input, handlers, view, commit, layout, motion, paint, recording the GPU frame, and submit and present except as below | not spawned: `App::run` (`crates/rune/src/app.rs`) or `tooling::headless` runs on it |
| `rune present` | macOS overlays on wgpu's Metal backend (not Rune's own): finishes the encoder, submits and presents a recorded plain frame (`submit_and_present`, `crates/rune/src/overlay/frame.rs`) | `PresentWorker::spawn`, `crates/rune/src/frame/present_worker.rs` |
| `rune-gpu-prewarm` | macOS `prewarm_gpu`: a device and renderer before the first window | `Slot::start`, `crates/rune/src/render/prewarm.rs` |
| `rune-gpu-warm` | off macOS, `warm_gpu`: a device and renderer ahead of a surface, and `warm_gpu_with`'s closure | `warm` (two sites), `crates/rune/src/render/warm.rs` |
| `rune-gpu-parked` | off macOS: holds a hidden overlay's device until a surface takes it or `PARKED` passes | `park`, `crates/rune/src/render/warm.rs` |
| `rune-embed-prewarm` | Apple: the shared device and uber pipeline for the first `LayerHost` | `prewarm`, `crates/rune/src/platform/embed.rs` |
| `rune-images` | image decoding, up to `DECODERS` (4) threads | `ImageCache::new`, `crates/rune/src/image.rs` |
| `rune-latest` | `Latest` requests: at most four threads shared by every `Latest`; idle ones exit | `Pool::submit`, `crates/rune/src/latest/pool.rs` |
| named by the app | `rune::task::spawn`, `spawn_after` and `serve`: one thread for each piece of an app's work off the UI thread | `spawn`, `crates/rune/src/task.rs` |
| `rune-clipboard` | the OS clipboard through arboard | `Worker::spawn`, `crates/rune/src/clipboard/native.rs` |
| `rune-video-completion` | polls the device for native video frames' GPU completion | `SurfaceRenderer::new`, `crates/rune/src/native_surface/pipeline.rs` |
| `rune-media-poster` | macOS, one per clip: opens it and decodes the newest requested poster | `request_poster`, `crates/rune/src/video/macos.rs` |
| `rune-audio-tick` | macOS: posts the audio tick to the main queue while audio is busy | `start_ticking`, `crates/rune/src/video/macos.rs` |
| `rune-recorder` | macOS: one microphone recording, start to stop | `start`, `crates/rune/src/recorder/macos.rs` |
| `rune-tray` | Linux: the StatusNotifierItem over D-Bus | `Native::new`, `crates/rune/src/tray/sni.rs` |
| `rune-tray` | Windows: the tray icon's message window | `Native::new`, `crates/rune/src/tray/win32.rs` |
| `rune-occlusion` | Windows: hears windows move, show, hide or minimise (`SetWinEventHook`) and works out which of Rune's windows are covered | `start`, `crates/rune/src/platform/win32/occlusion.rs` |
| `rune-shortcuts-portal` | Wayland: the GlobalShortcuts portal | `Native::new`, `crates/rune/src/shortcuts/wayland.rs` |
| `rune-shortcuts-x11` | X11: listens for grabbed keys | `Native::new`, `crates/rune/src/shortcuts/x11.rs` |
| `rune-file-dialog` | Linux: one FileChooser portal request | `show`, `crates/rune/src/file_dialog/portal.rs` |
| `rune-appearance-portal` | Linux: the Settings portal's light or dark preference | `start`, `crates/rune/src/platform/portal_appearance.rs` |
| `rune-layer-watch` | polls a layer-shell or XWayland overlay's display socket and wakes the loop | `Watch::spawn`, `crates/rune/src/platform/layer_shell.rs` |
| unnamed | X11: raw pointer motion for overlays | `MouseWatch::new`, `crates/rune/src/platform/x11/pointer.rs` |
| unnamed | Windows: the low-level mouse hook's message loop | `MouseWatch::new`, `crates/rune/src/platform/win32/pointer.rs` |
| unnamed | Linux and BSD: reaps `xdg-open` | `open`, `crates/rune/src/open_url.rs` |
| `rune-instance` | Windows, after `single_instance`: takes a later launch's arguments from the per-user named pipe and hands its links or a reopen to the app | `listen`, `crates/rune/src/open_url/instance.rs` |
| `rune-interactions` | sends timed interactions to the local collector | `sender`, `crates/rune/src/tooling/interaction/export.rs` |
| `rune-watchdog` | samples the UI thread's stack while a frame runs long | `watchdog`, `crates/rune/src/tooling/interaction/watchdog.rs` |
| `rune-video` | headless recording: streams frames to ffmpeg | `Writer::encode`, `crates/rune/src/tooling/record.rs` |
| unnamed, up to 4 | headless recording: writes PNG frames | `Writer::png`, `crates/rune/src/tooling/record.rs` |
| unnamed, scoped, up to 12 | the motion gate's cells, in parallel | `run`, `crates/rune/src/motion_gate/gate.rs` |

Rune also runs code on threads it does not start: macOS's CVDisplayLink calls `tick`
(`crates/rune/src/platform/display_link.rs`), which only stores the target time and wakes the
loop, and AccessKit's action handlers, on any thread, push requests into a mutex-guarded inbox
(`Shared` in `crates/rune/src/a11y/mod.rs`) and wake the host. Work for the main thread from
elsewhere goes through a `Waker` (`crates/rune/src/runtime/cx.rs`), which in a window sends the
loop a `WakeEvent`, or on macOS through the main dispatch queue (`on_main`,
`crates/rune/src/platform/main_queue.rs`).

## Who owns what

| Struct | Where it is held | Owns (fields) |
| --- | --- | --- |
| `Windows` (`crates/rune/src/window.rs`) | the winit event loop | `hosts` (every window and overlay), `factory`, `proxy`, `gpu` (the weak GPU pool) |
| `Host<S>` (`crates/rune/src/window.rs`) | `Windows::hosts` | `rt`, `win`, `gpu` (device and `Renderer`), `clock`, `comp`, `shared`, `popup`, `file_dialog`, `clipboard`, `meter`, `trace` |
| `Win` (`crates/rune/src/window/win.rs`) | `Host::win` | `window`, `surface`, `config`, `presented`, `native`, `first_frame`, `backoff`, `timer_frame` |
| `Overlay<T>` (`crates/rune/src/overlay.rs`) | `Windows::hosts` | `rt`, `live`, `renderer`, `gpu`, `clock`, `compositor`, `shared`, `panel` |
| `Live` (`crates/rune/src/overlay.rs`) | `Overlay::live` | `surface`, `config`, `backing`, `present`, `pacing` |
| `Running<S>` (`crates/rune/src/platform/web.rs`) | `Browser::running` | `rt`, `renderer`, `gpu`, `surface`, `clock`, `compositor`, `on_ready` |
| `LayerHost<S>` (`crates/rune/src/platform/embed.rs`) | the embedding app | `rt`, `renderer`, `gpu`, `surface`, `clock`, `compositor` |
| `Gpu` (`crates/rune/src/render.rs`) | shared through `Arc`; the web host owns its own | `instance`, `adapter`, `device`, `queue` |
| `Renderer` (`crates/rune/src/render.rs`) | each host | `pipeline`, `uber` (which uber shader, `variant::Uber`), `instances` (buffer), `atlas`, `atlas_pages`, `images`, `layers`, `uploads`, `surfaces` |
| `Runtime<S>` (`crates/rune/src/runtime.rs`) | a host's `rt`, or the caller | 238 fields (a few only in debug builds or with `hot`), among them `state`, `view`, `poll`, `queue`, `pending`, `nodes`, `node_index`, `path_ids`, `focus`, `press`, `hovered`, `scrolls`, `grows`, `stacks`, `virtuals`, `reveals`, `instances`, `surfaces`, `t`, `clock` |
| `Scene` (`crates/rune-motion/src/scene.rs`) | `Runtime::scene` | `nodes`, `order`, `exited`, `store` |
| `AnimationStore` (`crates/rune-motion/src/store.rs`) | `Scene::store` | `now`, `l_target`, `l_value`, `l_vel` |
| `LayoutStore` (`crates/rune-taffy/src/store.rs`) | `Runtime::layout` | `slots`, `node_idx`, `curr_children` |
| `TextSystem` (`crates/rune/src/text.rs`) | `Runtime::text` | `font_cx`, `layout_cx`, `layouts`, `runs`, `atlas` (glyphs), `glyph_cache`, `smoothing` (the `FontSmoothing` mode), `sans`, `mono` |
| `ImageAtlas` (`crates/rune/src/image/pixels.rs`) | `Runtime::images` | `pages`, `slots` |
| `MemoCache` (`crates/rune/src/memo.rs`) | `Runtime::memo` | `entries`, `flattened`, `pending_scratch`, `reused_scratch`, `taken_paths_scratch` |
| `ActionQueue` (`crates/rune/src/runtime/input/action.rs`) | `Runtime::queue` | the queued `Queued` actions |

Smaller runtime subsystems are their own types (an enum for one with modes), held by one
`Runtime` field each: `KeyIndex` (`key_index`), `TimerWake` (`timer_wake`), `LoopClock`
(`loop_clock`), `Launches` (`launches`), `Glides` (`pixel_snap`), `Mask` (`exit_mask`), `Zoom`
(`zoom`), `Press` (`press`), `Secondary` (`secondary`), `WheelLock` (`wheel_lock`), `Touch`
(`touch`), `Requests` (`requests`), `KeyReleases` (`key_releases`), `FocusState` (`focus`),
`TypeSelect` (`type_select`), `Activation` (`held`), `ModalFocus` (`modal_focus`), `HoverDelays`
(`hover_delays`), `HoverHold` (`hover_hold`), `FileDrag` (`file_drag`), `HelpState` (`help`),
`LinkState` (`links`), `Held` (`text_sel`), `InputClick` (`input_click`), `Popups` (`popups`),
`DialogState` (`file_dialog`), `Titlebar` (`titlebar`), `PressRun` (`press_clicks`),
`PressRepeat` (`press_repeat`), `ModifierKeys` (`modifier_keys`), `AppIme` (`app_ime`),
`ScrollBarHost` (`scroll_bars`), `LayoutSeen` (`layout_seen`), `Feedback` (`feedback`), `Reuse`
(`reuse`), `ApplyScratch` (`apply_scratch`), `Page` (`page`), `Title` (`window_title`),
`FitWindow` (`fit`), `A11yTree` (`a11y`), `IdleMeter` (`idle_meter`), `LandingHolds`
(`landing_holds`), `Watch` (`watch`) and `FrameStats` (`stats`). New runtime state goes in the
type of the subsystem that owns it.

State with modes is one explicit state machine (`rune::machine`, `crates/rune/src/machine.rs`):
an enum whose variants hold only what is valid in them, and a `step` that takes an event and
returns the effects for its caller to carry out. Focus (`runtime/input/focus_machine.rs`), hover
delays, a window's draw backoff (`window/backoff.rs`) and first frame, an overlay's presence, a
clip's playback (`video/phases.rs`), the scroll requests that outlive a commit
(`runtime/scroll/requests.rs`) and which uber shader a renderer draws with (`render/variant.rs`)
are built this way (paths under `crates/rune/src`).

## Hosts

A host owns one `Runtime`, a surface and a `Renderer`, and drives the frame above.

- **Windows and overlays.** `AnyHost` (`crates/rune/src/window.rs`) is what the one event loop
  needs of a host. `Host<S>` implements it in `crates/rune/src/window/any_host.rs` and
  `Overlay<T>` in `crates/rune/src/overlay/any_host.rs`. Each host has its own state type; all
  share the loop's thread and `Windows::gpu`. A window runs its app's `App::turn_scope` hooks
  around each turn that can reach its state (`Host::in_turn`); the windows of one loop take
  turns, never two at once. Apps reach a running host from any thread through `AppHandle` or
  `OverlayHandle`, which queue work and wake the loop.
- **Compositor.** `Compositor::render` (`crates/rune/src/compose.rs`) draws in place of
  `Renderer::draw`, after `Runtime::frame` built the instances, in every host: windows,
  overlays, the web, embedded views and headless runs. Its `Frame` lends the device, queue,
  encoder, target, `rt` and `ui` (the `Renderer`).
- **Embedding.** On Apple targets `LayerHost::from_core_animation_layer`
  (`crates/rune/src/platform/embed.rs`) attaches a `Runtime` to a `CAMetalLayer` that UIKit,
  SwiftUI or AppKit owns. The embedding view calls `LayerHost::frame` from its own display link
  and feeds input through `LayerHost::runtime_mut`.
- **Web.** `Browser<S>` (`crates/rune/src/platform/web.rs`) is a single-threaded winit
  application handler on the page's thread. The page has no threads: `rune::task` runs its work
  there in timer tasks after the current event (`crates/rune/src/task/web.rs`), `Latest` in a
  `spawn_local` task, and `Browser::about_to_wait` drains served channels after each batch of
  events. `App::on_ready` runs once the first frame is presented, as in a window. The `--html`
  export (`crates/rune/src/runtime/web/export.rs`) writes a static first frame from a headless
  layout.
- **Headless.** `tooling::headless` runs the same `Runtime` without a window, on a virtual clock
  (`Runtime::wall_start` is `None`), for tests, scripts, traces and recordings.

### Instances

`Instance` (`crates/rune/src/render.rs`) is `#[repr(C)]`, eight 16-byte words, 128 bytes
(asserted at compile time), in device pixels: `rect`, `radii`, `fill`, `border`, `params` (border
width, kind, shadow sigma, glyph coverage exponent), `uv`, `clip` and `fade` (`[u32; 4]`: packed
edge-fade widths and offsets, and the clip radius). `params[1]` is the kind, `KIND_RECT` (0)
through `KIND_BACKDROP_BLUR` (13). `KIND_LAYER` and `KIND_LAYER_END` bracket isolated layers,
`KIND_EFFECT` marks where a compositor draws, and `KIND_BACKDROP_BLUR` where the renderer copies
and blurs what is below.

Two uber shaders read the layout (`crates/rune/src/render/variant.rs`). Classic is
`crates/rune/src/shader.wgsl` alone. Windfoil adds `crates/rune/src/outline.wgsl`, so a glyph with
`radii.w > 0` draws from its outline in the outline store (binding 4) instead of the atlas, and a
dual-source variant of it blends subpixel text by channel. Renderers start with Windfoil except on
macOS, where a Classic renderer builds Windfoil the first time text asks for outlines. The
precompiled Metal libraries (Classic and Windfoil) and the Windows DXBC (Windfoil) carry a hash of
their WGSL and of the pipeline layout (`crates/rune/src/render/uber/metallib_hash.rs`,
`windfoil_hash.rs` and `dxbc_hash.rs`); when either differs, Rune compiles the WGSL as it builds
that pipeline.

## `thread_local!` statics

`crates/rune/src` declares 82 `thread_local!` statics outside tests (paths in this table are
under `crates/rune/src`). Each thread has its own copy, so they split by what a second thread
would see:

| Kind | Count | Statics |
| --- | --- | --- |
| Scope of the build, frame or event underway: set by a guard, restored after | 10 | `memo.rs` `SCOPES`, `memo/snapshot.rs` `SCOPES`, `theme.rs` `SRGB`, `platform/scroll_bars.rs` `VIEW_SCOPE`, `input/time.rs` `CURRENT`, `lib.rs` `SCENE_CLOCK`, `runtime/scroll/watch.rs` `SCROLL_WATCH_STATE`, `text/measure.rs` `OVERRIDE`, `hot.rs` `ORIGINAL_ONLY`, `quit.rs` `LOOP` |
| Per-thread cache: rebuilt by any thread that needs it | 13 | `text/measure.rs` `MEASURER`, `text/measure/ink.rs` `CACHE`, `text/system_fonts.rs` `SYSTEM`, `spell.rs` `CHECKED`, `spell/windows.rs` `FACTORY` and `CHECKERS`, `input/cursor/os.rs` `CURSORS` and `CLEAR`, `input/cursor/web.rs` `MADE`, `platform/macos/cursor.rs` `IMAGES`, `TRANSPARENT` and `MOVE`, `input/key_diagnostics.rs` `HASHES` |
| Process state kept on the main thread: correct because every window and overlay, and the code that reads these, runs there | 43 | `window/spec.rs` `OPENED`, `text/families.rs` `CHOSEN`, `runtime/cx.rs` `COMMITS`, `input/time.rs` `LAST`, `input/cursor/os.rs` `PENDING`, `drag_out/macos.rs` `LIVE`, `native_menu/mod.rs` `BAR` and `NEXT_GUARD`, the 8 in `native_menu/default_macos.rs`, `native_menu/macos.rs` and `native_menu/standard_macos.rs`, `platform/accent.rs` and `platform/scroll_bars.rs` `WATCHING`, `platform/macos/cursor.rs` `DONE`, `platform/macos/drop.rs` `DESTS`, `platform/macos/panel_bridge.rs` `WINDOWS`, `VIEWS` and `LOST`, `platform/macos/traffic_lights.rs` `SPOTS` and `PLACING`, `platform/reopen.rs` `REOPEN` and `HOOKED`, `platform/web.rs` `INSETS` and `LARGE_VIEWPORT`, `platform/web/page.rs` `PAGE`, `task/web.rs` `QUEUE`, `time/page_clock.rs` `CLOCK`, `platform/win32.rs` `INPUTS`, `platform/win32/dpi_drag.rs` `MOVING`, `platform/win32/frame.rs` `FRAMED`, `platform/x11/overlay.rs` `SHOWN`, `popup_menu/win32.rs` `TURN`, `quit/mac.rs` `HOOKED`, `quit/win32.rs` `DELIVER` and `WINDOW`, `shortcuts/service.rs` `ENGINES`, `hot.rs` `INITIAL`, `input/key_diagnostics.rs` `WARNED` |
| Per thread by design: the thread is the unit | 7 | `video/mod.rs` and `recorder/mod.rs` `HEADLESS` (a headless run's clock), `recorder/fake.rs` `INSTALLED`, `recorder/macos.rs` `ON_RECORDER_THREAD`, `platform/win32/pointer.rs` `OBSERVERS` (on the hook thread), `platform/win32/occlusion.rs` `PASS` (on the occlusion thread), `tooling/interaction/watchdog.rs` `WATCHED` |
| Diagnostics and test overrides | 9 | `frame/profile.rs` `TIMES`, `tooling/interaction.rs` `ACTIVE` and `TIMES`, `tooling/perf_warn/budget.rs` `TIMES`, `DEPTH` and `PRINTED`, `render/dxbc.rs` `FORCE`, `render/metallib.rs` `FORCE_METALLIB` and `METALLIB_LOADS` |

Native callbacks that run on other threads (AccessKit's action handlers, the display link) don't
read these; they hand work to the host's thread. A `Runtime` driven from a second thread starts
with empty caches and does not see the main thread's process state: font families chosen with
`Tx::set_font_families`, `commit_generation`, windows queued by `open_window`.

## Invariants

- **Layout runs at the target state, once per commit in the common case.** It solves again inside
  one commit when `aspect_fit` or `anchored_fit` elements need their fitted size
  (`Measurement::solve`), when estimated virtual rows leave a viewport short
  (`Measurement::finish_from`), and when the view set a new zoom, or requested focus, the caret or
  a layout read moved (`Runtime::commit`). Sampling the `Scene`'s springs runs no app code. A frame
  still calls the app's `El::on_scroll_offset` callbacks while it steps motion
  (`Runtime::tell_scroll_offsets`), a moved layout read re-runs the view, and a moved virtual
  window builds the rows that enter (`Runtime::frame_step`).
- **`Measurement` cannot mutate the scene.** It borrows text and layout caches and virtual
  geometry only: no `Scene`, growth or reveal tracks, instances or `&mut S`
  (`crates/rune/src/runtime/commit.rs`). Its result, `Measured`, is owned and consumed once by
  `Runtime::apply`; an `El` that was not measured cannot reach `apply`.
- **Moved stubs are restored before a pass repeats.** A committing pass moves reused memo subtrees
  out of `Runtime::nodes` and leaves `Node::moved_stub`s holding each node's id, parent, geometry
  and track flags (`crates/rune/src/runtime/commit/memo_reuse.rs`). Every re-measure first puts
  them back (`restore_moved_nodes`) and returns the buffers: `Runtime::discard_measured` does it
  for a zoom, focus, caret or layout-read re-measure, and `Measurement::finish_from` for its own
  retry when virtual rows fell short.
- **Identity is the path.** A node's `NodeId` comes from its path of keys and `#index` segments
  (`Runtime::path_ids`); measuring and flattening use the same rule, so the same path keeps the
  same tracks, scroll and focus.
- **Parents precede children** in `Runtime::nodes`, and forward passes rely on it:
  `Runtime::apply` passes a remounting row's flag down its subtree in one walk.
- **Frame phases run in order:** queued input, then the commit if state changed, then `Scene`,
  growth, scroll, reveals, then the build. Before the handlers, motion already running steps the
  catch-up part of the gap (the gap capped at `MAX_GAP`, less at most `RESUME_STEP`); the last
  step of at most `RESUME_STEP` runs after them and moves what they started too. So a late
  frame's gap goes to old tracks, never to targets the frame first sets (`split`,
  `Runtime::frame_split`).
- **Routed input sees the tree its state builds.** Before a key, key-up, physical key, IME, paste
  or file action runs, a pending state change commits (`Runtime::frame_step`).
- **Hit tests read the last build's rects by node index.** After a commit moves the indices,
  the next build works hover out from fresh geometry instead (`Runtime::built`,
  `Runtime::built_ids`).
- **The first commit is instant**: initial layout and entrances land at once (`Runtime::apply`),
  while `El::looping` elements get their loop tracks in that commit and can already run.
- **A `Tx::live_only` handler must not change what the view builds.** Debug builds commit a
  `live_only` event that comes a quarter second or more after the last commit anyway, and compare
  what it built (`crates/rune/src/runtime/motion/live_check.rs`; `Runtime::check_live_only(false)`
  turns it off).
