//! What a host knows of its surface's configuration, as one machine. wgpu panics on a drawable
//! from a surface that refused its configuration, so a host asks for one only while the state
//! is `drawable`. Each failure, a refused configure or a drawable lost or outdated, counts
//! toward a run that only a drawn frame ends: a configure that takes doesn't. After a failure
//! the surface tries again by itself after `REFUSED_RETRY`, then twice as long each time, and
//! after `REFUSED_ATTEMPTS` in a row it waits for a resize. The run is reported as it begins
//! and as it gives up. A host that draws nothing for now (suspended, zero-sized) holds its
//! retry until it draws again.

use std::time::Duration;

use crate::machine::{Attempt, Effects, Seq};
use crate::time::Instant;

/// The first retry after a failure; later ones wait twice as long.
pub(crate) const REFUSED_RETRY: Duration = Duration::from_millis(50);
/// Failures in a row after which a surface stops trying by itself.
pub(crate) const REFUSED_ATTEMPTS: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceState {
    /// The last configure took; `failures` is the run no frame has ended yet.
    Took { failures: u32 },
    /// Configure `attempt` waits for the browser's answer, and draws meanwhile.
    Checking { attempt: Attempt, failures: u32 },
    /// It failed `failures` times in a row, and tries again at `due`.
    Waiting { failures: u32, due: Instant },
    /// It failed while its host draws nothing, and waits for it to draw again.
    Held { failures: u32 },
    /// It failed `REFUSED_ATTEMPTS` times in a row: only a resize tries again.
    GaveUp,
}

impl Default for SurfaceState {
    fn default() -> Self { SurfaceState::Took { failures: 0 } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceEvent {
    /// The host configures as `attempt`; `Took` or `Refused` answers it.
    Configure { attempt: Attempt },
    Took { attempt: Attempt },
    Refused { attempt: Attempt, now: Instant },
    /// The surface gave no drawable at `now`: lost, outdated, or refused while reconfiguring.
    Lost { now: Instant },
    /// A frame drew from it.
    Drew,
    /// The host would draw at `now`.
    Tick { now: Instant },
    /// The host draws nothing for now (suspended or zero-sized).
    Suspend,
    /// It draws again from `now`.
    Resume { now: Instant },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceEffect {
    /// Configure the surface again now.
    Configure,
    /// Look again at this time, for the retry.
    WakeAt(Instant),
    /// Say the run began, or that it stopped trying.
    Report(Refused),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refused {
    First,
    Last,
}

impl Refused {
    /// The report about `what`'s surface ("a window"), with the error that ended its last try
    /// when the host has one.
    pub(crate) fn say(self, what: &str, error: Option<&str>) -> String {
        let said = match self {
            Refused::First => format!("{what}'s GPU surface has no drawable (a refused configuration, or a lost one) and draws nothing until it gets one"),
            Refused::Last => format!("{what}'s GPU surface failed {REFUSED_ATTEMPTS} times in a row; it tries again when resized"),
        };
        match error { Some(error) => format!("{said}: {error}"), None => said }
    }
}

impl SurfaceState {
    /// Whether the host may ask the surface for a drawable.
    pub(crate) fn drawable(self) -> bool { matches!(self, SurfaceState::Took { .. } | SurfaceState::Checking { .. }) }

    /// When it tries again by itself.
    pub(crate) fn retry(self) -> Option<Instant> {
        match self { SurfaceState::Waiting { due, .. } => Some(due), _ => None }
    }

    fn failures(self) -> u32 {
        match self {
            SurfaceState::Took { failures } | SurfaceState::Checking { failures, .. }
            | SurfaceState::Waiting { failures, .. } | SurfaceState::Held { failures } => failures,
            SurfaceState::GaveUp => REFUSED_ATTEMPTS,
        }
    }
}

pub(crate) fn step(state: &mut SurfaceState, event: SurfaceEvent) -> Effects<SurfaceEffect> {
    use {SurfaceEffect as Fx, SurfaceEvent as E, SurfaceState as S};
    match (*state, event) {
        (s, E::Configure { attempt }) => {
            *state = S::Checking { attempt, failures: s.failures() };
            Effects::none()
        }
        (S::Checking { attempt, failures }, E::Took { attempt: a }) if a == attempt => {
            *state = S::Took { failures };
            Effects::none()
        }
        (S::Checking { attempt, failures }, E::Refused { attempt: a, now }) if a == attempt => fail(state, failures, now),
        (S::Took { failures } | S::Checking { failures, .. }, E::Lost { now }) => fail(state, failures, now),
        (S::Took { .. }, E::Drew) => {
            *state = S::Took { failures: 0 };
            Effects::none()
        }
        (S::Checking { attempt, .. }, E::Drew) => {
            *state = S::Checking { attempt, failures: 0 };
            Effects::none()
        }
        (S::Waiting { due, .. }, E::Tick { now }) => Effects::one(if now >= due { Fx::Configure } else { Fx::WakeAt(due) }),
        (S::Waiting { failures, .. }, E::Suspend) => {
            *state = S::Held { failures };
            Effects::none()
        }
        (S::Held { failures }, E::Resume { now }) => {
            let due = now + retry_after(failures);
            *state = S::Waiting { failures, due };
            Effects::one(Fx::WakeAt(due))
        }
        // An answer to an attempt the state no longer holds, or nothing to do.
        _ => Effects::none(),
    }
}

/// The run grows by one failure at `now`.
fn fail(state: &mut SurfaceState, failures: u32, now: Instant) -> Effects<SurfaceEffect> {
    let failures = failures + 1;
    let mut effects = Effects::none();
    match failures {
        1 => effects.push(SurfaceEffect::Report(Refused::First)),
        REFUSED_ATTEMPTS => effects.push(SurfaceEffect::Report(Refused::Last)),
        _ => {}
    }
    *state = if failures >= REFUSED_ATTEMPTS {
        SurfaceState::GaveUp
    } else {
        let due = now + retry_after(failures);
        effects.push(SurfaceEffect::WakeAt(due));
        SurfaceState::Waiting { failures, due }
    };
    effects
}

/// The wait after the `failures`th failure in a row.
fn retry_after(failures: u32) -> Duration { REFUSED_RETRY * 2u32.pow(failures.clamp(1, REFUSED_ATTEMPTS) - 1) }

/// A host's surface state, and the attempts it numbers its configures with.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Configured {
    pub(crate) state: SurfaceState,
    seq: Seq,
}

impl Configured {
    pub(crate) fn drawable(&self) -> bool { self.state.drawable() }
    pub(crate) fn retry(&self) -> Option<Instant> { self.state.retry() }
    #[cfg(test)]
    pub(crate) fn gave_up(&self) -> bool { self.state == SurfaceState::GaveUp }
    pub(crate) fn step(&mut self, event: SurfaceEvent) -> Effects<SurfaceEffect> { step(&mut self.state, event) }

    /// A configure begins: its attempt, which its answer carries back.
    pub(crate) fn begin(&mut self) -> Attempt {
        let attempt = self.seq.next();
        self.step(SurfaceEvent::Configure { attempt });
        attempt
    }

    /// `result` answers configure `attempt` at `now`.
    pub(crate) fn answer(&mut self, attempt: Attempt, result: &Result<(), String>, now: Instant) -> Effects<SurfaceEffect> {
        self.step(match result { Ok(()) => SurfaceEvent::Took { attempt }, Err(_) => SurfaceEvent::Refused { attempt, now } })
    }
}
