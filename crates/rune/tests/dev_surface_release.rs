//! A build without debug assertions keeps the process's arguments for the app unless it opts in
//! (`rune::dev_tooling` or `RUNE_DEV=1`). This builds `examples/dev_surface.rs` in release, which
//! takes minutes from a cold target directory, so it's ignored by default:
//! `cargo test -p rune --test dev_surface_release -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Builds the fixture in release and returns its executable.
fn fixture() -> PathBuf {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .args(["build", "--release", "-p", "rune", "--example", "dev_surface", "--message-format=json-render-diagnostics"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stderr(Stdio::inherit())
        .output()
        .unwrap();
    assert!(out.status.success(), "the fixture's release build failed");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["target"]["name"] == "dev_surface")
        .find_map(|message| message["executable"].as_str().map(PathBuf::from))
        .expect("cargo names the fixture's executable")
}

/// Runs the fixture, killing it after a minute: a run that opened a window would never end.
fn run(exe: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(exe);
    command.args(args).env_remove("RUNE_DEV").env_remove("DEV_SURFACE_OPT_IN").env_remove("DEV_SURFACE_WINDOWS");
    for (key, value) in env { command.env(key, value); }
    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(60) { let _ = child.kill(); }
        std::thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

/// What the fixture printed: the arguments Rune left the app.
fn extras(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
#[ignore = "builds a release binary: cargo test -p rune --test dev_surface_release -- --ignored"]
fn a_release_binary_reads_rune_flags_only_when_it_opts_in() {
    let exe = fixture();
    let dir = std::env::temp_dir().join(format!("rune-dev-surface-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let snap = dir.join("snap.png");
    let path = snap.to_str().unwrap();

    // No opt-in: `--snap` is the app's, as are a malformed Rune flag and `--help`, and nothing exits.
    let all = ["--snap", path, "--size", "large", "--help", "--frames"];
    let out = run(&exe, &[], &all);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(extras(&out), format!("{all:?}"));
    assert!(!snap.exists(), "nothing rendered");

    // `RUNE_DEV=1`, then `rune::dev_tooling` with `App::run` and with `run_windows`: `--snap`
    // renders, as in a debug build.
    let windows = [("DEV_SURFACE_OPT_IN", "1"), ("DEV_SURFACE_WINDOWS", "1")];
    for env in [&[("RUNE_DEV", "1")][..], &[("DEV_SURFACE_OPT_IN", "1")], &windows] {
        let out = run(&exe, env, &["--snap", path]);
        assert!(out.status.success() && snap.exists(), "{env:?}: {out:?}");
        std::fs::remove_file(&snap).unwrap();
    }

    // Opted in, Rune's flags behave as in a debug build: a malformed one exits with code 2.
    for env in [&[("RUNE_DEV", "1")][..], &windows] {
        let out = run(&exe, env, &["--size", "large", "doc.txt"]);
        assert_eq!(out.status.code(), Some(2), "{env:?}: {out:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("--size is WxH"), "{out:?}");
    }
    std::fs::remove_dir_all(&dir).ok();
}
