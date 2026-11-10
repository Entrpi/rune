//! Rune's dev surface: its flags in the process's arguments (`--snap`, `--script`, `--trace`, ...)
//! and the interaction timing a loopback `OTEL_EXPORTER_OTLP_ENDPOINT` turns on. A debug build
//! always has it. A build without debug assertions has it only after [`dev_tooling`] or with
//! `RUNE_DEV=1`, so a shipped app's arguments reach the app untouched, and a collector set up for
//! another tool doesn't start sampling it. `RUNE_DEV` is Rune's own: nothing sets it ambiently, as
//! tools set `OTEL_*`.

use std::sync::atomic::{AtomicBool, Ordering};

static OPTED_IN: AtomicBool = AtomicBool::new(false);

/// Gives a build without debug assertions Rune's dev surface, as a debug build has it: Rune reads
/// its flags (`--snap`, `--script`, `--trace`, ...) from the process's arguments, and a loopback
/// `OTEL_EXPORTER_OTLP_ENDPOINT` turns interaction timing on. Without it, or `RUNE_DEV=1`, such a
/// build hands every argument to `extra_args` and ignores the endpoint. Call it first in `main`,
/// before `run`, `run_windows`, `extra_args` or `is_headless` read the arguments.
pub fn dev_tooling() {
    OPTED_IN.store(true, Ordering::Relaxed);
}

/// Whether this process has Rune's dev surface.
pub(crate) fn on() -> bool {
    in_env(&|k| std::env::var(k).ok(), cfg!(debug_assertions), opted_in())
}

pub(crate) fn opted_in() -> bool {
    OPTED_IN.load(Ordering::Relaxed)
}

/// For the environment `var` reads, a build with or without debug assertions, and whether the app
/// called [`dev_tooling`]: a debug build, `RUNE_DEV=1`, or the call.
pub(crate) fn in_env(var: &dyn Fn(&str) -> Option<String>, debug: bool, opted_in: bool) -> bool {
    debug || opted_in || var("RUNE_DEV").as_deref().map(str::trim) == Some("1")
}

#[cfg(test)]
#[path = "dev_surface_tests.rs"]
mod tests;
