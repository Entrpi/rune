//! `LayerHost` on a CAMetalLayer with no window: a compositor's marks, and a surface that
//! refuses its configuration.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use objc2_video::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};

use super::LayerHost;
use crate::compose::{Compositor, Frame};
use crate::render::KIND_EFFECT;
use crate::tooling::AppSpec;
use crate::{Cx, El, col, effect};

struct MarkProbe(Arc<AtomicU32>);

impl Compositor<()> for MarkProbe {
    fn render(&mut self, f: &mut Frame<'_, ()>) {
        for (idx, inst) in f.rt.instances.iter().enumerate() {
            if inst.params[1] == KIND_EFFECT {
                self.0.store(f.rt.marks.get(idx).copied().unwrap_or(0), Ordering::Relaxed);
            }
        }
        f.draw_ui();
    }
}

fn layer() -> Retained<AnyObject> { unsafe { msg_send![AnyClass::get(c"CAMetalLayer").expect("QuartzCore"), new] } }

#[test]
fn layer_host_runs_compositor_with_tracked_marks() {
    let layer = layer();
    let seen = Arc::new(AtomicU32::new(0));
    let view = |_: &(), _: &mut Cx<()>| -> El { col().size(120.0, 80.0).child(effect(42).size(60.0, 20.0)) };
    let spec = AppSpec::new("embed-test", (120.0, 80.0), || (), view);
    let host = unsafe {
        LayerHost::from_core_animation_layer(
            Retained::as_ptr(&layer) as *mut std::ffi::c_void,
            &spec,
            (),
            (120.0, 80.0),
            2.0,
            true,
            Arc::new(|| {}),
        )
    };
    let Ok(mut host) = host else { return };
    host = host.with_compositor(MarkProbe(seen.clone()));
    let out = host.frame();
    assert!(out.drew);
    assert_eq!(seen.load(Ordering::Relaxed), 42);
}

/// A layer whose surface refuses its configuration is still a host: it draws nothing, rather
/// than panicking on a drawable, and asks to be woken for its retry; a resize or the retry
/// draws again. One that keeps refusing stops asking, and stops animating, after its last
/// attempt, until a resize; suspended, it holds its retry until it shows again.
#[test]
fn layer_host_waits_out_a_refused_configuration() {
    use crate::window::{REFUSED_ATTEMPTS, REFUSED_RETRY, refusals};
    if let Err(error) = super::shared_gpu() { return eprintln!("skipped: {error}") }
    let layer = layer();
    let spec = AppSpec::new("embed-refused", (120.0, 80.0), || (), |_: &(), _: &mut Cx<()>| col().size(120.0, 80.0));
    refusals::refuse(1);
    let host = unsafe {
        LayerHost::from_core_animation_layer(Retained::as_ptr(&layer) as *mut std::ffi::c_void, &spec, (), (120.0, 80.0), 2.0, true, Arc::new(|| {}))
    };
    let mut host = host.expect("a refused first configuration still makes a host");
    assert!(!host.configured.drawable() && host.configured.retry().is_some(), "it waits to try again");
    // Its retry may come due while the renderer is made: refuse that one too.
    refusals::refuse(1);
    let out = host.frame();
    refusals::refuse(0);
    assert!(!out.drew && !out.animating, "{out:?}");
    assert!(out.wake_after_secs.is_some_and(|s| s > 0.0 && s <= 3.0 * REFUSED_RETRY.as_secs_f64()), "{out:?}");
    host.resize((120.0, 80.0), 2.0);
    assert!(host.frame().drew, "the same size tries again");

    refusals::refuse(1);
    host.resize((100.0, 60.0), 2.0);
    assert!(!host.configured.drawable(), "a refused resize gives no drawable");
    std::thread::sleep(REFUSED_RETRY);
    assert!(host.frame().drew, "the retry takes");

    // Suspended, a refused layer keeps no retry; showing again starts it over.
    refusals::refuse(1);
    host.resize((110.0, 70.0), 2.0);
    assert!(host.configured.retry().is_some());
    host.resize((0.0, 0.0), 2.0);
    assert_eq!(host.configured.retry(), None, "a suspended layer holds its retry");
    host.resize((110.0, 70.0), 2.0);
    assert!(host.frame().drew, "showing it again configures at once");

    refusals::refuse(REFUSED_ATTEMPTS);
    host.resize((120.0, 80.0), 2.0);
    let mut frames = 0;
    let out = loop {
        let out = host.frame();
        frames += 1;
        let Some(wait) = out.wake_after_secs.filter(|_| frames < 20) else { break out };
        std::thread::sleep(std::time::Duration::from_secs_f64(wait));
    };
    assert!(!out.drew && !out.animating && out.wake_after_secs.is_none(), "{out:?}");
    assert!(host.configured.gave_up() && frames <= REFUSED_ATTEMPTS, "a frame per retry at most, then none: {frames}");
    assert!(!host.frame().animating, "it stays quiet after giving up");
    host.resize((120.0, 80.0), 2.0);
    assert!(host.frame().drew, "a resize tries again");
}
