# rune-register

Lists a module adds itself to from its own file, for apps that keep lists of stories, scenes or
knob groups: a new entry is one line beside its code, and no central list needs editing. It is
opt-in. An app depends on it directly, pinned to a `dev` rev as it pins `rune`. It needs nothing
but `linkme`, and nothing in this workspace uses it: not `rune`, `rune-kit` or the examples.

| Item | What it does |
| --- | --- |
| `slice!(pub LIST: T)` | Declares a list of `Part<T>`s. |
| `register!(LIST: T = [item, …])` | Adds this module's items to `LIST`, from any module. |
| `collect(&LIST) -> Vec<T>` | Every part's items: parts ordered by module path, each part's items in its own order. Parts from different modules come in path order whatever the link order; parts registered in the same module keep the order the linker gave them. |
| `merge(own, more, name, join)` | `own` with `more` folded in by name. `own`'s last item (the list's footer) is set aside; each item of `more` joins the first item of its name among the rest of `own` and the `more` items already added (through `join`), or is added after them. The footer goes back last, unchanged, so an item named like it is added as new. |

```rust
use rune_register::{collect, slice};

#[derive(Clone, Copy)]
pub struct Scene(&'static str);

slice!(pub SCENES: Scene);

mod chat {
    rune_register::register!(super::SCENES: super::Scene = [super::Scene("chat-1")]);
}

fn main() {
    let names: Vec<&str> = collect(&SCENES).iter().map(|s| s.0).collect();
    assert_eq!(names, ["chat-1"]);
}
```

Registration uses [linkme](https://docs.rs/linkme), which collects statics in a linker section.
That works on Linux, macOS, iOS, tvOS, Android, Windows, FreeBSD and a few others. On `wasm32`
every list is empty: `register!` still type-checks its items but adds them nowhere, so a web
build must make its lists another way. Any other target linkme lacks fails to build with
linkme's error.

A part is only collected if its crate is linked in, so a crate that only registers must be named
somewhere, for example `use my_crate as _;`.
