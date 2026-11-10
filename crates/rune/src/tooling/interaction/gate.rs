//! Whether interaction timing runs, read once from the environment, and where it sends.
//!
//! On when `RUNE_PERF=1`, or when `OTEL_EXPORTER_OTLP_ENDPOINT` names a loopback address (a
//! developer's local collector) in a process with Rune's dev surface (`dev_surface`: a debug
//! build, `RUNE_DEV=1` or `rune::dev_tooling`), so a shipped build ignores a collector set up for
//! another tool. `RUNE_PERF=0` turns it off over either. A headless run turns on only for
//! `RUNE_PERF=1`, so agents' snapshots and tests don't send. Every check is one load of a cached
//! byte.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// Where `RUNE_PERF=1` sends when no loopback endpoint is set: a local OTLP/HTTP collector.
pub(crate) const DEFAULT_ENDPOINT: &str = "127.0.0.1:27686";

const UNKNOWN: u8 = 0;
const OFF: u8 = 1;
const ON: u8 = 2;

static STATE: AtomicU8 = AtomicU8::new(UNKNOWN);
static ENDPOINT: OnceLock<Option<SocketAddr>> = OnceLock::new();

#[inline(always)]
pub(crate) fn on() -> bool {
    #[cfg(test)]
    return FORCED.get();
    #[cfg(not(test))]
    match STATE.load(Ordering::Relaxed) {
        ON => true,
        OFF => false,
        _ => decide(false),
    }
}

/// A headless run: on only for an explicit `RUNE_PERF=1`.
pub(crate) fn headless() { decide(true); }

#[cold]
fn decide(headless: bool) -> bool {
    let var = |k: &str| std::env::var(k).ok();
    let opted_in = crate::tooling::dev_surface::opted_in();
    let on = !cfg!(test) && !cfg!(target_arch = "wasm32") && wanted_in(&var, cfg!(debug_assertions), opted_in, headless);
    if on { let _ = ENDPOINT.set(endpoint(var("OTEL_EXPORTER_OTLP_ENDPOINT").as_deref())); }
    STATE.store(if on { ON } else { OFF }, Ordering::Relaxed);
    on
}

/// [`wanted`] for the environment `var` reads, a build with or without debug assertions, and
/// whether the app called `rune::dev_tooling`: the endpoint counts only with Rune's dev surface.
pub(crate) fn wanted_in(var: &dyn Fn(&str) -> Option<String>, debug: bool, opted_in: bool, headless: bool) -> bool {
    let otel = var("OTEL_EXPORTER_OTLP_ENDPOINT").filter(|_| crate::tooling::dev_surface::in_env(var, debug, opted_in));
    wanted(var("RUNE_PERF").as_deref(), otel.as_deref(), headless)
}

/// Whether the settings turn timing on.
pub(crate) fn wanted(perf: Option<&str>, otel: Option<&str>, headless: bool) -> bool {
    match perf.map(str::trim) {
        Some("0") => false,
        Some("1") => true,
        _ => !headless && otel.and_then(loopback).is_some(),
    }
}

/// Where spans go: the loopback endpoint set, else [`DEFAULT_ENDPOINT`].
pub(crate) fn endpoint(otel: Option<&str>) -> Option<SocketAddr> {
    otel.and_then(loopback).or_else(|| DEFAULT_ENDPOINT.parse().ok())
}

/// `http://host:port[/path]` or `host:port` on loopback; anything else is `None`.
pub(crate) fn loopback(setting: &str) -> Option<SocketAddr> {
    let rest = setting.trim();
    let rest = rest.strip_prefix("http://").unwrap_or(rest);
    let host = rest.split('/').next()?.replace("localhost", "127.0.0.1");
    host.parse::<SocketAddr>().ok().filter(|a| a.ip().is_loopback())
}

/// The collector, once timing is on.
pub(crate) fn collector() -> Option<SocketAddr> { ENDPOINT.get().copied().flatten() }

#[cfg(test)]
thread_local! {
    static FORCED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Turns timing on for this test thread, sending nowhere.
#[cfg(test)]
pub(crate) fn force_on() { FORCED.set(true); }
