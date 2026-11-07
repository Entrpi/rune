import type { ReactNode } from "react"

export type Spec = { label: string; values: ReactNode[]; mono?: boolean }
/** A file or folder in this repo that implements an idea. The prerender fails if the path doesn't exist. */
export type CodeRef = { path: string; note?: string }
export type Entry = { slug: string; title: string; body: ReactNode; spec?: Spec[]; code?: CodeRef[] }

const at = (path: string, note?: string): CodeRef => ({ path, note })

export const principles: Entry[] = [
  {
    slug: "feel-first",
    title: "Feel is the product",
    body: (
      <>
        <p>
          Rune is judged first on how it feels in the hand: 120&nbsp;Hz, springs that can be interrupted at any point, and
          nothing that pops, jumps or is cut off by a hard edge. The bar is SwiftUI on a ProMotion iPhone.
        </p>
      </>
    ),
    spec: [
      { label: "Frame", values: ["8.33 ms at 120 Hz"], mono: true },
      { label: "Target", values: ["event → frame p50 ≤ 8.3 ms"], mono: true },
    ],
    code: [
      at("crates/rune-motion/src/spring.rs", "closed-form springs"),
      at("examples/settings/src/main.rs", "the smoke test"),
    ],
  },
  {
    slug: "motion-defaults",
    title: "Motion rules are runtime defaults",
    body: (
      <>
        <p>
          Kit's motion rules are what the framework does when the app says nothing. Nothing animates on first mount,
          drags apply 1:1, removed elements play their exit before they unmount, stationary edges never move, and
          reduced motion swaps travel for a short fade.
        </p>
        <p>
          Animated properties change only through spring tracks, so a one-frame jump can't be written. In tests, the
          motion-lint rules (pops, jumps, reversals, jitter, hard clips) are failures.
        </p>
      </>
    ),
    spec: [
      { label: "Spring", values: [<code>Spring::new(duration, bounce)</code>], mono: true },
      { label: "Default", values: ["bounce 0"], mono: true },
    ],
    code: [
      at("crates/rune-motion/src/tokens.rs", "the motion palette"),
      at("crates/rune-motion/src/transition.rs", "exits before unmount"),
    ],
  },
  {
    slug: "animation-off-app",
    title: "Animation never runs through app code",
    body: (
      <p>
        Views state targets, and Rune animates toward them. Once per frame, on the caller's thread (the main thread in
        a window), Rune runs commit and layout when state changed, then motion sampling, paint and present. Sampling
        springs runs no app code: no view re-run, no layout, no diff. Scrolling can: it calls{" "}
        <code>on_scroll_offset</code>, mounts virtual rows, and re-runs the view when a layout read moved. Today a slow
        handler on the main thread holds a running animation too; the target is a render thread, so a slow handler
        could delay the next change but not stall one that's already moving.
      </p>
    ),
    spec: [{ label: "Target", values: ["app work per animated frame: 0"], mono: true }],
    code: [
      at("crates/rune-motion/src/scene.rs", "spring tracks"),
      at("crates/rune/tests/live.rs", "no view, layout or shaping"),
    ],
  },
  {
    slug: "native-behaviour",
    title: "Native behaviour, one look",
    body: (
      <>
        <p>
          Text input, accessibility, scroll momentum, clipboard and window management behave the way each platform
          expects. Controls, type, motion and materials come from the app's theme and look the same everywhere, Mac
          included.
        </p>
        <p>The core ships no brand of its own. OpenCode's theme is the first theme built on it and the main example.</p>
      </>
    ),
    spec: [
      { label: "Native", values: ["IME, accessibility, scrolling, clipboard, windows"] },
      { label: "Themed", values: ["controls, type, motion, materials"] },
    ],
    code: [
      at("crates/rune/src/edit.rs", "text input and IME"),
      at("crates/rune/src/a11y/tree.rs", "the accessibility tree (AccessKit)"),
      at("crates/rune/src/theme.rs", "themes as data"),
      at("themes/opencode", "OpenCode's theme"),
    ],
  },
  {
    slug: "agents",
    title: "Written and checked by agents",
    body: (
      <>
        <p>
          Agents write most of Rune and most Rune apps, so they need to see and test their own work without a person.
          Every component has named scenes, and every scene renders headless and deterministically. The target is an
          inspector that answers queries about nodes, rects, springs and timings; until it's built,{" "}
          <code>--tree</code>, <code>--trace</code>, scripted input and <code>--hot-control</code> stand in.
        </p>
        <p>
          The code is optimised for performance, correctness and speed of change rather than for a human reading it.
          Comments state invariants for the next agent.
        </p>
      </>
    ),
    spec: [{ label: "Render", values: [<code>--snap out.png --at 0.25</code>], mono: true }],
    code: [
      at("crates/rune/src/tooling.rs", "--snap, --scene, --script, --trace"),
    ],
  },
  {
    slug: "one-way",
    title: "One way to do things",
    body: (
      <p>
        Views are plain Rust functions that return a keyed tree. There is no view macro language, no second way to
        hold state and no notify call to forget. Errors should name the fix, such as{" "}
        <code>Text has no key inside a dynamic list; add .key(id)</code>.
      </p>
    ),
    code: [at("crates/rune/src/el.rs", "the whole view API")],
  },
  {
    slug: "fast-and-reference",
    title: "A fast path and a reference path",
    body: (
      <p>
        The target is a slow, obviously correct reference implementation beside the fast one for layout, text, the
        renderer and springs, with the fast path tested against it on generated inputs, so it can be specialised,
        duplicated or unsafe where a measurement justifies it. Today the renderer has one:{" "}
        <code>crates/rune-ref</code> and its GPU differential test on generated scenes, run by{" "}
        <code>scripts/check-reference.sh --gpu</code>, with no CI job. rune-motion's tests check the springs' closed
        form against ODE integration.
      </p>
    ),
    spec: [{ label: "Reference", values: ["CPU renderer, no GPU tricks"] }],
    code: [
      at("crates/rune-ref/src/shader.rs", "the CPU renderer"),
      at("crates/rune-ref/tests/differential.rs", "GPU against CPU"),
    ],
  },
  {
    slug: "observable",
    title: "Everything is observable",
    body: (
      <p>
        Every frame, node, layout rect, spring and timing can be queried and traced (<code>--tree</code>,{" "}
        <code>--trace</code>). The target is that a pixel traces back to the node and the <code>file:line</code> that
        drew it, and that a recorded session replays to the same pixels; today <code>Rewind</code> records the last
        minute of presented rects and opacity, not inputs. The agent's view of the app is the accessibility tree, so
        every agent test exercises accessibility too.
      </p>
    ),
    code: [
      at("crates/rune-motion/src/trace.rs", "motion traces"),
      at("crates/rune/src/tooling.rs", "--tree, --trace"),
    ],
  },
  {
    slug: "hot-reload",
    title: "Edits reach the screen with state intact",
    body: (
      <>
        <p>
          Structural Rust edits hot-patch on the Mac with the opt-in <code>hot</code> feature, keeping app state:
          about 8&nbsp;s median from save to GPU for Settings today, and the goal is 300&nbsp;ms. Token and literal
          edits that apply live with no rebuild are the target.
        </p>
        <p>
          A phone can't load new code. The target is a development mode that runs the render thread on the iPhone and
          the app side on the Mac, connected by the channel the two threads would use in production. Neither is built
          yet.
        </p>
      </>
    ),
    spec: [
      {
        label: "Targets",
        values: ["tokens ≤ 50 ms", "literals ≤ 100 ms", "Rust ≈ 8 s today, goal 300 ms"],
        mono: true,
      },
    ],
    code: [at("spikes/hot-patch", "989 ms median")],
  },
  {
    slug: "budgets",
    title: "Performance budgets are gates",
    body: (
      <p>
        The target is each scene's cost counted exactly (instructions, instances, glyphs, layout nodes and
        allocations) and bisectable, so CI can fail the change that pushed a scene over its frame budget. Today{" "}
        <code>FrameStats</code> counts commits, layout nodes, glyphs and shaped layouts, and the allocation tests run in
        the macOS CI job; there is no budget gate yet. Once settled, an app does no work at all.
      </p>
    ),
    spec: [
      { label: "At rest", values: ["0 ticks, 0 GPU submissions"], mono: true },
      { label: "Per frame", values: ["0 allocations"], mono: true },
    ],
    code: [at("crates/rune/tests/alloc.rs", "0 allocations a frame"), at("crates/rune-motion/tests/alloc.rs")],
  },
  {
    slug: "owned",
    title: "Owned, not wrapped",
    body: (
      <p>
        Rune depends on foundational crates and never on another UI framework. It starts fresh instead of forking Zed's
        GPUI, and has no rope or sum tree: a field's text is a plain <code>String</code>, and input methods come through
        winit on the desktops and a hidden textarea on the web.
      </p>
    ),
    spec: [{ label: "Uses", values: ["wgpu, Taffy, Parley, AccessKit"] }],
    code: [at("crates/rune/Cargo.toml", "dependencies")],
  },
]

export const layers: Entry[] = [
  {
    slug: "tree",
    title: "Core tree and state",
    body: (
      <>
        <p>
          A view is a plain Rust function that returns an <code>El</code> tree with explicit keys. App state is one
          plain value outside any view. Handlers are queued and run between frames with <code>&mut</code> access to
          the state, then the view re-runs and the new tree is diffed against the old one by key.
        </p>
        <p>
          The toggle below, from the settings example, states where the knob should be and animates nothing itself.
          The <code>.motion(motion::SNAPPY)</code> token names the spring that gets it there.
        </p>
      </>
    ),
    spec: [
      { label: "Uses", values: ["keyed retained tree, one plain state value"] },
      { label: "Not", values: ["signals, GPUI entities, immediate mode"] },
    ],
    code: [at("crates/rune/src/el.rs", "El, keys, handlers"), at("crates/rune/src/runtime.rs", "queue, view, diff")],
  },
  {
    slug: "layout",
    title: "Layout",
    body: (
      <p>
        Taffy computes flexbox, grid and block layout. Today its solver runs over <code>rune_taffy::LayoutStore</code>,
        Rune's compact layout store beside the runtime's node list; the target is Taffy over Rune's own node arena, so
        there is one tree. In the common case layout runs once per commit, at the target state; it solves again for{" "}
        <code>aspect_fit</code> and{" "}
        <code>anchored_fit</code>, while estimated virtual rows leave a viewport short, and when focus, a caret or a
        layout read moved. Moving between layouts is the spring tracks' job. Long lists build only the visible rows
        plus overscan and keep the rest as estimated heights.
      </p>
    ),
    spec: [
      { label: "Uses", values: ["Taffy"] },
      { label: "Not", values: ["own engine, SwiftUI-style proposals"] },
    ],
    code: [
      at("crates/rune-taffy/src/lib.rs", "LayoutStore, beside the nodes"),
      at("crates/rune/src/el/virtual_list.rs", "visible rows only"),
    ],
  },
  {
    slug: "text",
    title: "Text",
    body: (
      <p>
        Parley handles shaping, bidi and line breaking, with shaped runs cached apart from wrap points so resizing never
        re-shapes. Glyphs rasterise into a greyscale atlas: on the Mac through a port of CoreGraphics' glyph pipeline,
        with font smoothing's dilation set by the ink's luminance so dark text isn't thin, and elsewhere from their
        outlines with Chromium's weight (<code>FontSmoothing::WindfoilChromium</code>), on quarter-pixel pens, with
        small CJK faces hinted onto whole pixels. Overflow
        is cut only when an element asks: <code>El::truncate_fade</code> fades it out, and{" "}
        <code>El::truncate</code>{" "}
        and <code>El::max_lines</code> end it with an ellipsis.
      </p>
    ),
    spec: [
      { label: "Uses", values: ["Parley, swash, skrifa"] },
      { label: "Not", values: ["CoreText only, cosmic-text"] },
    ],
    code: [
      at("crates/rune/src/text.rs", "Parley, swash, the atlas"),
      at("research/23-text-quality.md", "against CoreText"),
    ],
  },
  {
    slug: "render-thread",
    title: "Spring tracks and the presented tree",
    body: (
      <>
        <p>
          The runtime's <code>Scene</code> owns the presented tree, which is what's on screen this frame. Today it runs
          on the main thread with the rest of the frame; the target moves it to a render thread. Every animated
          property is a track of analytic spring segments, so position and velocity are exact at any time and a
          retarget starts the new spring from where the value really is.
        </p>
        <p>
          Layout animates per edge, so an edge that isn't moving gets no track. Removed nodes leave the flow, play their
          exit and unmount at opacity 0. Drags and scrolling are applied here, 1:1, and one clock with time scale and
          stepping is shared by tests and traces.
        </p>
      </>
    ),
    spec: [
      { label: "Spring", values: ["ω = 2π / duration", "ζ = 1 − bounce"], mono: true },
      { label: "Snap", values: ["both edges to 1/64 px"], mono: true },
    ],
    code: [
      at("crates/rune-motion/src/spring.rs"),
      at("crates/rune-motion/src/scene.rs", "today on the main thread"),
      at("crates/rune-motion/src/transition.rs", "presence"),
    ],
  },
  {
    slug: "renderer",
    title: "Renderer",
    body: (
      <>
        <p>
          One instanced pipeline draws rounded rects, borders, analytic shadows, glyphs and vector strokes and fills, so
          a typical frame is one render pass, with a draw call for each run of instances on the same image or
          glyph-atlas page. Every instance carries a clip with soft gradient edges, which makes "no hard edges" free.
          Rune blends in sRGB-encoded space by default, as CSS and Core Animation do;{" "}
          <code>App::srgb_blend(false)</code> blends in linear light.
        </p>
        <p>
          In the text modes drawn from outlines (the default everywhere but macOS and a Mac's browser), text whose
          scale moves draws straight from its outlines on the GPU, and in-app blur is a backdrop pass (
          <code>El::backdrop_blur</code>). Icons will draw from curves with Slug, and complex vectors with Vello's
          sparse strips.
        </p>
      </>
    ),
    spec: [
      { label: "Uses", values: ["wgpu; Rune's own Metal backend on the Mac"] },
      { label: "Not", values: ["Skia, compute Vello, raw Metal first"] },
    ],
    code: [
      at("crates/rune/src/render.rs"),
      at("crates/rune/src/shader.wgsl", "one uber-quad pipeline"),
      at("crates/rune/src/shape.rs", "strokes and fills"),
    ],
  },
  {
    slug: "shells",
    title: "Platform shells",
    body: (
      <p>
        A shell owns windows and panels, menus, clipboard, drag and drop, the text input protocol, the accessibility
        adapter and the app lifecycle. Rune runs on winit today, and input methods come through winit's IME on the
        desktops and a hidden textarea on the web. The
        target is thin shells per platform that implement each one's text protocol directly (
        <code>NSTextInputClient</code>, TSF, <code>text-input-v3</code>, <code>UITextInput</code>), borrowing from Zed's
        shells.
      </p>
    ),
    spec: [
      { label: "Uses", values: ["winit and its IME, then own shells; AccessKit"] },
      { label: "Not", values: ["SDL3"] },
    ],
    code: [
      at("crates/rune/src/window.rs", "winit today"),
      at("crates/rune/src/platform/macos.rs"),
      at("crates/rune/src/platform/win32.rs", "overlays"),
      at("crates/rune/src/platform/layer_shell.rs", "Wayland overlays"),
      at("crates/rune/src/a11y/mod.rs", "UI Automation, NSAccessibility and AT-SPI adapters"),
    ],
  },
  {
    slug: "native-layers",
    title: "Native layers on Apple",
    body: (
      <p>
        Liquid Glass samples what's behind the window, so only the window server can draw it. For Mac surfaces that want
        it, the Apple shell places system glass views behind transparent regions of Rune's layer and moves them in the
        same Core Animation transaction as the frame. This is optional; every platform gets Rune's own
        blur and materials.
      </p>
    ),
    spec: [{ label: "Uses", values: ["NSGlassEffectView, CATransaction"] }],
  },
  {
    slug: "tooling",
    title: "Agent tooling",
    body: (
      <p>
        Scenes are named states with fixture data. They render in the app, offscreen in tests and in a gallery, with
        pinned fonts and a fixed clock. The target is an inspector that speaks versioned JSON over a socket: query
        nodes by role, key or text, read rects, springs and source locations, and inject input with exact timestamps.
        Until it's built, <code>--tree</code>, <code>--trace</code>, scripted input and <code>--hot-control</code>{" "}
        stand in. Motion traces come
        straight from the runtime's <code>Scene</code>.
      </p>
    ),
    code: [
      at("crates/rune/src/tooling.rs", "scenes and flags"),
      at("crates/rune-motion/src/trace.rs", "motion traces"),
    ],
  },
  {
    slug: "playground",
    title: "Playground",
    body: (
      <p>
        A storybook built into the window, on ⌘⇧D. The app's scenes and its own stories, in a tree of groups nested by
        their paths, show one at a time on a plain stage, live and interactive, with their triggers and controls beside
        them. The keyboard is in one column at a time, the list, the stage or the controls, so a story's own keys work on
        its stage. Each story has its own state, so the real one is never touched. R replays a story from the start, its
        timed steps running on the logical clock, the same every time.
      </p>
    ),
    code: [at("crates/rune/src/playground.rs", "stories, triggers and replay")],
  },
]

export type Decision = { area: string; chose: string; rejected: string; why: string }

export const decisions: Decision[] = [
  {
    area: "Language",
    chose: "Rust",
    rejected: "Zig, C++, Swift",
    why: "The only option with production text, layout and accessibility crates, no garbage collector, wasm, and a working hot-patcher.",
  },
  {
    area: "GPU",
    chose: "wgpu's API everywhere; Rune's own Metal on the Mac",
    rejected: "Raw Metal first, hand-written Vulkan and D3D12",
    why: "One API ships Mac, Windows, Linux, iOS and the web. On the Mac, measurements showed wgpu's per-frame cost, so Rune's own Metal backend is the default there, plugged in through wgpu's custom backend so apps keep wgpu's types.",
  },
  {
    area: "Renderer",
    chose: "One instanced pass, glyph atlas",
    rejected: "Skia, compute Vello",
    why: "A typical frame is one render pass, and soft clip edges cost nothing extra.",
  },
  {
    area: "Look",
    chose: "Token-driven themes",
    rejected: "Per-OS look, a built-in brand",
    why: "Apps carry their own design language on every platform. OpenCode's is one theme, kept outside the core.",
  },
  {
    area: "Text",
    chose: "Parley, swash, skrifa",
    rejected: "CoreText only, cosmic-text",
    why: "The same text on every platform, and resizing never re-shapes.",
  },
  {
    area: "Layout",
    chose: "Taffy, over a compact layout store",
    rejected: "An own engine, SwiftUI-style proposals",
    why: "Agents already know flexbox. Today rune_taffy::LayoutStore keeps a second tree beside the nodes; one tree, on Rune's arena, is the target.",
  },
  {
    area: "State",
    chose: "Keyed tree from plain functions",
    rejected: "Signals, GPUI entities, immediate mode",
    why: "Models outside views survive hot patches, and handlers never run inside render, so borrows can't collide.",
  },
  {
    area: "Animation",
    chose: "Spring tracks outside the view; a render thread as the target",
    rejected: "Re-rendering views each frame",
    why: "Sampling a spring never re-runs a view, and retargets keep velocity. On a render thread, animations would keep running while the app is busy.",
  },
  {
    area: "Accessibility",
    chose: "AccessKit",
    rejected: "Own bridges",
    why: "The same tree is in --tree's dump, so agent tests exercise it.",
  },
  {
    area: "Hot reload",
    chose: "Subsecond patches; live values and a split process on iPhone as the target",
    rejected: "A view language interpreter, loading code on the device",
    why: "Views stay plain Rust, and iOS won't load new code on a device.",
  },
]

export type Milestone = { id: string; title: string; status: "done" | "now" | "started" | "next" | "later"; body: ReactNode }

export const milestones: Milestone[] = [
  {
    id: "m0",
    title: "Spikes",
    status: "now",
    body: (
      <p>
        Short experiments that each answer one question with a number: hot-patch speed, glass underlay sync, frame
        pacing, the iPhone split mode, and text quality against CoreText. Hot patching is done, at 989&nbsp;ms median from
        save to new output.
      </p>
    ),
  },
  {
    id: "m0-5",
    title: "Rendering smoke test",
    status: "now",
    body: (
      <p>
        A small settings window that proves the basics: rounded rects, borders, shadows, soft edges, crisp text, Retina
        scale, springs and clicks, checked by headless renders and then on a real window.
      </p>
    ),
  },
  {
    id: "m1",
    title: "Theming, gallery and this site",
    status: "now",
    body: (
      <p>
        The token schema and theme system, with OpenCode's theme generated from <code>@opencode/ui</code> as its first
        consumer. A gallery of components with the same names as the web kit, each diffed against its web story.
      </p>
    ),
  },
  {
    id: "m2",
    title: "Hex Settings",
    status: "next",
    body: (
      <p>
        The first real screen, on Mac, Windows and Linux: forms, toggles, a keycap recorder, lists and a text field. Own
        shells start here, along with AccessKit and goldens on all three platforms.
      </p>
    ),
  },
  {
    id: "m3",
    title: "OpenCode Desktop",
    status: "started",
    body: (
      <p>
        The main screens: sidebar and sessions, the streaming transcript, composer, diff view, command palette and
        menus, against real data. Done when it looks like OpenCode Desktop side by side and feels clearly better on all
        three desktops.
      </p>
    ),
  },
  {
    id: "m4",
    title: "Kit's Mac surfaces",
    status: "later",
    body: <p>An app's pill, tray and launcher, and their settings windows, with optional Liquid Glass.</p>,
  },
  {
    id: "m5",
    title: "Web",
    status: "started",
    body: (
      <p>
        The same apps in the browser over WebGPU, starting with live demos on this site. An experimental target builds
        and runs today (<code>WEB.md</code>, <code>examples/web-hello</code>).
      </p>
    ),
  },
  {
    id: "m6",
    title: "iOS, then Android",
    status: "later",
    body: (
      <p>
        An iOS shell with <code>UITextInput</code>, a view for embedding Rune in SwiftUI, and the split-process dev mode.
        The first surface ships inside OpenCode iOS. Android follows.
      </p>
    ),
  },
]

export const risks: Entry[] = [
  {
    slug: "risk-text",
    title: "Text input and text quality",
    body: (
      <p>
        Input methods on three desktops, then iOS selection, autocorrect and dictation, are where custom-rendered
        frameworks most visibly fail. Today input methods come through winit; the plan is to implement each platform's
        protocol directly. Text rendering is gated on a pixel diff against CoreText.
      </p>
    ),
  },
  {
    slug: "risk-look",
    title: "Matching OpenCode Desktop",
    body: (
      <p>
        The bar is a Chromium-rendered app. Each component is diffed against its <code>@opencode/ui</code> story,
        rendered by headless Chromium beside Rune's headless renderer.
      </p>
    ),
  },
  {
    slug: "risk-scope",
    title: "Scope",
    body: (
      <p>
        A renderer, text, input methods, accessibility, six shells and tooling is years of work for a human team. Agents
        write it inside a verification harness that was built first, and Rune ships one real surface at a time.
      </p>
    ),
  },
]
