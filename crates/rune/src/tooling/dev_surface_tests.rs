use super::in_env;
use crate::tooling::Args;
use crate::tooling::interaction::gate::wanted_in;

/// An environment holding just `vars`.
fn env(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k| vars.iter().find(|v| v.0 == k).map(|v| v.1.to_owned())
}

#[test]
fn an_optimised_build_has_the_dev_surface_only_when_asked() {
    // (debug build, environment, dev_tooling called) → Rune reads its flags.
    let cases: [(bool, &'static [(&str, &str)], bool, bool); 9] = [
        (true, &[], false, true),
        (true, &[("RUNE_DEV", "0")], false, true),
        (false, &[], false, false),
        (false, &[("RUNE_DEV", "1")], false, true),
        (false, &[("RUNE_DEV", " 1\n")], false, true),
        (false, &[("RUNE_DEV", "0")], false, false),
        (false, &[("RUNE_DEV", "true")], false, false),
        (false, &[], true, true),
        (false, &[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4318")], false, false),
    ];
    for (debug, vars, opted_in, on) in cases {
        assert_eq!(in_env(&env(vars), debug, opted_in), on, "debug {debug}, {vars:?}, dev_tooling {opted_in}");
    }
}

#[test]
fn an_ambient_collector_starts_timing_only_with_the_dev_surface() {
    const LOCAL: (&str, &str) = ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4318");
    // `gate::decide` passes the real environment, `cfg!(debug_assertions)` and `dev_tooling`'s flag.
    let timed = |vars: &'static [(&'static str, &'static str)], debug, opted_in| wanted_in(&env(vars), debug, opted_in, false);
    assert!(timed(&[LOCAL], true, false), "a debug build honours a developer's collector, as before");
    assert!(!timed(&[LOCAL], false, false), "a shipped build ignores one set up for another tool");
    assert!(timed(&[LOCAL, ("RUNE_DEV", "1")], false, false), "RUNE_DEV=1");
    assert!(timed(&[LOCAL], false, true), "rune::dev_tooling");
    assert!(timed(&[("RUNE_PERF", "1")], false, false), "RUNE_PERF=1 asks for it in any build");
    assert!(!timed(&[LOCAL, ("RUNE_PERF", "0"), ("RUNE_DEV", "1")], false, true), "RUNE_PERF=0 still wins");
    assert!(!timed(&[], true, true), "nothing asked for it");
    assert!(!wanted_in(&env(&[LOCAL]), true, false, true), "a headless run still needs RUNE_PERF=1");
}

#[test]
fn without_the_dev_surface_every_argument_is_the_apps() {
    let all: Vec<String> = ["--snap", "a.png", "--size", "large", "--script", "--help", "-h", "--hot", "file.txt"].map(String::from).into();
    let (args, extra) = Args::from_process(all.clone(), false);
    assert!(!args.headless());
    assert!(args.snap.is_none() && args.size.is_none() && args.script.is_none());
    assert_eq!(extra, all, "untouched and in order, `--help` included");
    let (args, extra) = Args::from_process(["--snap", "a.png", "file.txt"].map(String::from).into(), true);
    assert!(args.headless() && args.snap.is_some());
    assert_eq!(extra, ["file.txt"]);
}
