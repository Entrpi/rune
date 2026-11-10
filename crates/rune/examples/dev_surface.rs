//! The app `tests/dev_surface_release.rs` builds in release. It prints the arguments Rune left
//! it and exits, unless Rune read flags that ask for a headless run, which it then renders with
//! `App::run`, or with `run_windows` under `DEV_SURFACE_WINDOWS=1`. `DEV_SURFACE_OPT_IN=1` makes
//! it call `rune::dev_tooling()` first.
use rune::{App, AppSpec, Color, Cx, El, WindowSpec, el};

fn view(_: &(), _: &mut Cx<()>) -> El {
    el().size(120.0, 80.0).fill(Color { r: 0.4, g: 0.65, b: 1.0, a: 1.0 })
}

fn spec() -> AppSpec<()> {
    AppSpec::new("Dev surface", (120.0, 80.0), || (), view)
}

fn main() {
    if std::env::var_os("DEV_SURFACE_OPT_IN").is_some() { rune::dev_tooling(); }
    if !rune::is_headless() {
        println!("{:?}", rune::extra_args());
        return;
    }
    if std::env::var_os("DEV_SURFACE_WINDOWS").is_some() {
        rune::run_windows(|_| [WindowSpec::new("main", spec())]);
    } else {
        App::new(spec()).run();
    }
}
