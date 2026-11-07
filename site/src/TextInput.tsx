import type { ReactNode } from "react"
import type { CodeRef } from "./content"
import { CodeRefs, Pager } from "./pages"
import { REPO } from "./routes"
import { PageHead, SectionHead } from "./system/Frame"

const issue = (n: number) => <a href={`${REPO}/issues/${n}`}>#{n}</a>

type Method = { platform: string; route: ReactNode; fields: ReactNode; editors: ReactNode; overlays: ReactNode }

/** Input methods by platform, each cell written from the code listed under the table. */
const methods: Method[] = [
  {
    platform: "macOS",
    route: <>winit's view, as an <code>NSTextInputClient</code></>,
    fields: "Composition and commit, with the candidate window at the caret",
    editors: "The same",
    overlays: "Keys only, once it asks for them",
  },
  {
    platform: "Windows",
    route: "IMM32 through winit; no TSF",
    fields: "Composition and commit, with the candidate window at the caret",
    editors: "The same",
    overlays: <>No keys: <code>request_keys</code> isn't supported</>,
  },
  {
    platform: "Linux, X11",
    route: "XIM through winit",
    fields: "Composition and commit; XIM gets the caret's position",
    editors: "The same",
    overlays: <>No keys: <code>request_keys</code> isn't supported</>,
  },
  {
    platform: "Linux, Wayland",
    route: (
      <>
        <code>text-input-v3</code> through winit
      </>
    ),
    fields: (
      <>
        Composition and commit, with the candidate window at the caret. A focused field keeps the window drawing every
        frame while the input method answers each caret update ({issue(225)}).
      </>
    ),
    editors: "The same",
    overlays: "Keys only, once it asks for them, with layer-shell v4 and a seat",
  },
  {
    platform: "Web",
    route: (
      <>
        A hidden <code>&lt;textarea&gt;</code>; no <code>EditContext</code>
      </>
    ),
    fields: (
      <>
        Typing and composition as textarea edits applied to the field, not <code>Ime</code> events, with the candidate
        window at the page's top-left corner, not the caret. On iOS 27 the soft keyboard closes right after a tap
        focuses a field ({issue(165)}).
      </>
    ),
    editors: (
      <>
        Keys only; no <code>Ime</code> events
      </>
    ),
    overlays: "No overlays",
  },
  {
    platform: "iOS",
    route: "None",
    fields: (
      <>
        No text input: <code>embed::LayerHost</code> routes no keys
      </>
    ),
    editors: "No",
    overlays: "No overlays",
  },
]

const methodRefs: CodeRef[] = [
  { path: "crates/rune/src/window/events.rs", note: "the only WindowEvent::Ime route" },
  { path: "crates/rune/src/window/platform_sync.rs", note: "input method on, candidate window at the caret" },
  { path: "crates/rune/src/runtime/input/app_ime.rs", note: "editors the app draws (App::on_ime)" },
  { path: "crates/rune/src/overlay/any_host.rs", note: "overlays: keys on request, no input method" },
  { path: "crates/rune/src/platform/layer_shell.rs", note: "Wayland overlays: keys from the seat" },
  { path: "crates/rune/src/platform/web/bridge.js", note: "the hidden textarea" },
  { path: "crates/rune/src/platform/web/input.rs", note: "textarea messages to Runtime::web_edit" },
  { path: "crates/rune/src/runtime/web/runtime.rs", note: "web_edit: edits the focused TextEdit directly" },
  { path: "crates/rune/src/platform/embed.rs", note: "LayerHost, for iOS" },
]

type Platform = "macOS" | "Windows" | "Linux" | "Web" | "iOS"
const platforms: Platform[] = ["macOS", "Windows", "Linux", "Web", "iOS"]

const none: Record<Platform, ReactNode> = { macOS: "No", Windows: "No", Linux: "No", Web: "No", iOS: "No" }
const typed: Record<Platform, ReactNode> = {
  macOS: "Input method",
  Windows: "Input method",
  Linux: "Input method",
  Web: "Textarea",
  iOS: "No",
}

/** Text services by platform, for a text field in a window. */
const services: { service: string; cells: Record<Platform, ReactNode> }[] = [
  {
    service: "Spell check",
    cells: {
      macOS: (
        <>
          With <code>El::spellcheck</code>: <code>NSSpellChecker</code>
        </>
      ),
      Windows: (
        <>
          With <code>El::spellcheck</code>: <code>ISpellChecker</code>
        </>
      ),
      Linux: "No",
      Web: "No",
      iOS: "No",
    },
  },
  { service: "Dictation", cells: typed },
  { service: "Emoji picker", cells: typed },
  { service: "Autofill", cells: none },
  { service: "Look Up", cells: { ...none, macOS: "No: winit's view gives the system no text" } },
  { service: "Writing Tools", cells: { ...none, macOS: "No: winit's view gives the system no text" } },
  {
    service: "Edit menu",
    cells: {
      macOS: "The field's menu, and the menu bar's Edit menu",
      Windows: "The field's menu",
      Linux: "No: no native popup menus",
      Web: "No: no native popup menus",
      iOS: "No: no native popup menus",
    },
  },
  { service: "Text drag and drop", cells: none },
]

const serviceRefs: CodeRef[] = [
  { path: "crates/rune/src/spell.rs", note: "the checker on each platform" },
  { path: "crates/rune/src/runtime/input/field_menu.rs", note: "a field's menu" },
  { path: "crates/rune/src/popup_menu/mod.rs", note: "native popup menus: macOS and Windows" },
  { path: "crates/rune/src/native_menu/standard.rs", note: "the macOS Edit menu" },
  { path: "crates/rune/src/runtime/selection/click.rs", note: "a press in a field selects" },
  { path: "crates/rune/src/drag_out/mod.rs", note: "drags out of a window, macOS only" },
  { path: "crates/rune/src/platform/web/bridge.js", note: "the textarea and the password input" },
]

export function TextInputView() {
  return (
    <>
      <PageHead eyebrow={<span>Text input</span>} title="Text input">
        <p>
          A Rune text field (<code>text_input</code>, or rune-kit's <code>TextInput</code> and <code>TextArea</code>)
          draws its own text. On the desktop, plain typing reaches it as keys, and what an input method composes, such
          as CJK conversion, arrives from winit 0.30 as <code>Ime</code> events, which go to the focused field or to
          an editor the app draws (<code>App::on_ime</code>). Dead keys, the emoji picker and dictation can arrive
          either way, depending on winit's backend: on Windows a dead key's character comes as a key's text. On the
          web, a focused field's typing and compositions go into a hidden textarea, and Rune applies the textarea's
          edits to the field's <code>TextEdit</code> directly. No <code>Ime</code> events come from them, so{" "}
          <code>App::on_ime</code> hears nothing there.
        </p>
        <p>
          The two tables say what each platform gets today. Each cell is written by hand from the code listed under its
          table, not from a run on that platform.
        </p>
      </PageHead>

      <section className="doc-section">
        <SectionHead id="input-methods" title="Input methods">
          <p>
            On the desktop, while a focused field, or a caret the app marks with <code>El::ime_cursor</code>, has the
            keyboard, the window turns its input method on and places the candidate window at the caret. An editor the
            app draws hears the events with <code>App::on_ime</code>. A focused secure field keeps the input method
            off.
          </p>
        </SectionHead>
        <div className="table-wrap">
          <table className="decisions matrix">
            <thead>
              <tr>
                <th scope="col">Platform</th>
                <th scope="col">Route</th>
                <th scope="col">Text fields</th>
                <th scope="col">Editors the app draws</th>
                <th scope="col">Overlays</th>
              </tr>
            </thead>
            <tbody>
              {methods.map(m => (
                <tr key={m.platform}>
                  <th scope="row">{m.platform}</th>
                  <td data-label="Route">{m.route}</td>
                  <td data-label="Text fields">{m.fields}</td>
                  <td data-label="Editors the app draws">{m.editors}</td>
                  <td data-label="Overlays">{m.overlays}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="prose prose-after">
          <ul>
            <li>
              On the desktop the route is winit's. IMM32, XIM and <code>text-input-v3</code> are winit 0.30's backends,
              and Rune has no platform text-input code of its own, so Windows gets no TSF text store.
            </li>
            <li>
              Only a window routes <code>WindowEvent::Ime</code> into the runtime. An overlay on macOS, or on Wayland
              with layer-shell v4, takes keys once it asks for them (<code>KeyPolicy::OnDemand</code>) and never turns
              an input method on, so plain typing reaches its field and a composition doesn't. Windows and X11
              overlays take no keys.
            </li>
            <li>
              On the web the textarea holds up to 512 UTF-16 units on each side of the selection, and a composition
              holds it still until commit (<a href={`${REPO}/blob/dev/WEB.md`}>WEB.md</a>). Its edits and compositions
              go to the focused field's <code>TextEdit</code> (<code>Runtime::web_edit</code>), not through{" "}
              <code>Ime</code> events. The textarea stays at the page's top-left corner, so the candidate window opens
              there. It follows a focused field only, so an editor the app draws gets keys from the canvas, but no
              composition and no <code>Ime</code> events.
            </li>
            <li>
              An iOS app embeds Rune with <code>embed::LayerHost</code>, which draws into the host view's Metal layer
              and routes no keys and no input method. There is no <code>UITextInput</code> adapter.
            </li>
          </ul>
        </div>
        <div className="prose-after">
          <CodeRefs refs={methodRefs} />
        </div>
      </section>

      <section className="doc-section">
        <SectionHead id="services" title="Text services">
          <p>
            What a text field in a window offers besides typing. <strong>Input method</strong> means Rune has no code
            for the service: text it sends through the input method, or as keys, lands in the field like any typing.
            Whether each service sends it to a Rune field hasn't been checked on each platform.{" "}
            <strong>Textarea</strong>: what the service types into the web's hidden textarea lands in the focused field
            as an edit.
          </p>
        </SectionHead>
        <div className="table-wrap">
          <table className="decisions matrix">
            <thead>
              <tr>
                <th scope="col">Service</th>
                {platforms.map(p => (
                  <th key={p} scope="col">
                    {p}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {services.map(s => (
                <tr key={s.service}>
                  <th scope="row">{s.service}</th>
                  {platforms.map(p => (
                    <td key={p} data-label={p}>
                      {s.cells[p]}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="prose prose-after">
          <ul>
            <li>
              <code>El::spellcheck</code> (rune-kit's <code>TextInput::spellcheck</code> and{" "}
              <code>TextArea::spellcheck</code>) marks a misspelled word once the caret has left it, or a second
              after typing in it stops, and the field's menu offers its suggestions and Learn Spelling. On Windows,
              when the app sets <code>El::on_spellcheck</code> and <code>El::on_spellcheck_languages</code>, the menu
              also has a Spellcheck Languages submenu that chooses the dictionaries. Rune has no checker on Linux, the
              web or iOS.
            </li>
            <li>
              Right-click (control-click on macOS), the Menu key or Shift+F10 opens a field's menu: Cut, Copy and
              Paste, and Select All off macOS. It is a native popup menu, which Rune has on macOS and Windows only;
              elsewhere it doesn't open. On macOS, <code>App::standard_menus</code> adds an Edit menu (Undo, Redo, Cut,
              Copy, Paste, Select All) to the menu bar, and a regular app gets it without asking.
            </li>
            <li>
              Look Up and Writing Tools read a field's text from the system. winit's macOS view gives them none: it
              reports no selection and no text.
            </li>
            <li>
              No field tells the system what it holds, so nothing autofills it. On the web, the secure field's input
              has <code>autocomplete="off"</code>.
            </li>
            <li>
              A press in a field places the caret or selects, so its text can't be dragged, and windows take only file
              drops. <code>El::drag_out</code> drags text out of an element on macOS, but not out of a field.
            </li>
            <li>
              On the web, the browser's own replacements, such as predictive text, land in the field. The web smoke test
              checks them in headless Chromium, not with a phone's keyboard or dictation.
            </li>
          </ul>
        </div>
        <div className="prose-after">
          <CodeRefs refs={serviceRefs} />
        </div>
      </section>

      <Pager prev={["/fast-apps", "Fast apps"]} next={["/architecture", "Architecture"]} />
    </>
  )
}
