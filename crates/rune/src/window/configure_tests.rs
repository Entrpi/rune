//! Configuring a surface: only through `configure`, which reports a refusal instead of
//! panicking, and what a host does with a surface that refused (`Configured`).

use super::*;
use crate::machine::Effects;

/// Methods named `configure` that aren't `wgpu::Surface`'s, by their receiver.
const NOT_SURFACES: [&str; 1] = ["observer"];

/// Rune calls `wgpu::Surface::configure` only in `window::configure`: a raw call takes a
/// refusal to wgpu's default error handler, which panics and ends the app. Every method call
/// named `configure` in crates/rune/src counts, whatever its arguments, unless its receiver is
/// one of `NOT_SURFACES`.
#[test]
fn surfaces_are_configured_only_through_the_checked_helper() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let needle = [".config", "ure("].concat();
    let (mut raw, mut paths) = (Vec::new(), vec![root.clone()]);
    while let Some(path) = paths.pop() {
        if path.is_dir() {
            paths.extend(std::fs::read_dir(&path).unwrap().map(|entry| entry.unwrap().path()));
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "rs") || path.ends_with("window/configure.rs") { continue; }
        let source = std::fs::read_to_string(&path).unwrap();
        for (at, _) in source.match_indices(&needle) {
            let before = &source[..at];
            let receiver = &before[before.rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(0, |i| i + 1)..];
            if !NOT_SURFACES.contains(&receiver) {
                raw.push(format!("{}:{} ({receiver})", path.strip_prefix(&root).unwrap().display(), before.lines().count()));
            }
        }
    }
    raw.sort();
    assert!(raw.is_empty(), "Surface::configure called outside window::configure: {raw:?}");
}

/// A surface that keeps failing tries again by itself on a doubling wait, says so as the run
/// begins and as it stops trying, then waits for a resize. Refused configures and lost
/// drawables count toward the same run; a configure that takes doesn't end it, a drawn frame does.
#[test]
fn a_failing_surface_retries_on_a_backoff_and_reports_twice() {
    use SurfaceEffect::{Configure, Report, WakeAt};
    let mut configured = Configured::default();
    assert!(configured.drawable() && configured.retry().is_none());
    let mut now = Instant::now();
    let refuse = |c: &mut Configured, now| { let a = c.begin(); c.answer(a, &Err("no".into()), now) };
    let take = |c: &mut Configured| { let a = c.begin(); c.answer(a, &Ok(()), Instant::now()) };

    assert_eq!(refuse(&mut configured, now), Effects::two(Report(Refused::First), WakeAt(now + REFUSED_RETRY)));
    assert!(!configured.drawable());
    let due = now + REFUSED_RETRY;
    assert_eq!(configured.step(SurfaceEvent::Tick { now: due - Duration::from_millis(1) }), Effects::one(WakeAt(due)));
    assert_eq!(configured.step(SurfaceEvent::Tick { now: due }), Effects::one(Configure), "a due retry configures");
    // The retry takes, but its drawable is lost: the same run goes on.
    now = due;
    assert_eq!(take(&mut configured), Effects::none());
    assert!(configured.drawable());
    assert_eq!(configured.step(SurfaceEvent::Lost { now }), Effects::one(WakeAt(now + REFUSED_RETRY * 2)), "the second failure waits twice as long");
    now += REFUSED_RETRY * 2;
    assert_eq!(refuse(&mut configured, now), Effects::one(WakeAt(now + REFUSED_RETRY * 4)));
    now += REFUSED_RETRY * 4;
    assert_eq!(refuse(&mut configured, now), Effects::one(WakeAt(now + REFUSED_RETRY * 8)));
    now += REFUSED_RETRY * 8;
    assert_eq!(refuse(&mut configured, now), Effects::one(Report(Refused::Last)), "the fifth stops trying");
    assert!(configured.gave_up() && configured.retry().is_none() && !configured.drawable());
    assert_eq!(configured.step(SurfaceEvent::Tick { now: now + Duration::from_secs(60) }), Effects::none());
    assert_eq!(refuse(&mut configured, now), Effects::none(), "a resize still tries, quietly");
    assert_eq!(take(&mut configured), Effects::none());
    assert!(configured.drawable());
    assert_eq!(configured.step(SurfaceEvent::Lost { now }), Effects::none(), "no frame drew since: the run goes on, given up");
    assert_eq!(take(&mut configured), Effects::none());
    configured.step(SurfaceEvent::Drew);
    assert_eq!(refuse(&mut configured, now), Effects::two(Report(Refused::First), WakeAt(now + REFUSED_RETRY)), "a drawn frame ended the run");
    assert!(REFUSED_ATTEMPTS == 5 && Refused::Last.say("a window", None).starts_with("a window's GPU surface failed 5 times"));
}

/// A configure's answer carries its attempt: an answer to an earlier configure, once a newer
/// one started, changes nothing, so a stale refusal never takes down a newer configuration.
/// The surface draws while the browser checks it.
#[test]
fn a_stale_answer_changes_nothing() {
    let (mut configured, now) = (Configured::default(), Instant::now());
    let first = configured.begin();
    assert!(configured.drawable(), "it draws while the browser checks it");
    let second = configured.begin();
    assert_eq!(configured.answer(first, &Err("late".into()), now), Effects::none());
    assert!(configured.drawable());
    assert_eq!(configured.answer(second, &Ok(()), now), Effects::none());
    assert_eq!(configured.state, SurfaceState::Took { failures: 0 });
    assert_eq!(configured.answer(second, &Err("twice".into()), now), Effects::none(), "an answered attempt is answered");
    let third = configured.begin();
    configured.step(SurfaceEvent::Drew);
    assert_eq!(configured.answer(third, &Err("late".into()), now).into_iter().count(), 2, "a refusal after drawing begins a run");
    assert!(!configured.drawable());
}

/// A host that draws nothing for now holds its retry, and arms it again when it draws.
#[test]
fn a_suspended_host_holds_its_retry() {
    let (mut configured, now) = (Configured::default(), Instant::now());
    let attempt = configured.begin();
    configured.answer(attempt, &Err("no".into()), now);
    assert!(configured.retry().is_some());
    assert_eq!(configured.step(SurfaceEvent::Suspend), Effects::none());
    assert_eq!(configured.retry(), None, "no timer while suspended");
    assert_eq!(configured.step(SurfaceEvent::Tick { now: now + Duration::from_secs(1) }), Effects::none());
    let later = now + Duration::from_secs(1);
    assert_eq!(configured.step(SurfaceEvent::Resume { now: later }), Effects::one(SurfaceEffect::WakeAt(later + REFUSED_RETRY)));
    assert_eq!(configured.retry(), Some(later + REFUSED_RETRY));
}

/// On a real Metal layer, on wgpu's backend: a configuration the surface refuses is an error,
/// not a panic in wgpu's default handler, and a later one that takes draws. A height Windows
/// reported wrapped below zero (65527) is held to the device's largest texture.
#[cfg(target_os = "macos")]
#[test]
fn a_refused_configuration_is_an_error_and_a_later_one_draws() {
    use objc2_video::{msg_send, rc::Retained, runtime::{AnyClass, AnyObject}};
    let layer: Retained<AnyObject> = unsafe { msg_send![AnyClass::get(c"CAMetalLayer").expect("QuartzCore"), new] };
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::METAL;
    let instance = wgpu::Instance::new(descriptor);
    let target = wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(Retained::as_ptr(&layer) as *mut std::ffi::c_void);
    let surface = unsafe { instance.create_surface_unsafe(target) }.expect("a Metal layer surface");
    let options = wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() };
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&options)) else { return eprintln!("skipped: no Metal adapter") };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).expect("a Metal device");
    let gpu = Gpu::from_device(instance, adapter, device, queue);
    let max = gpu.device.limits().max_texture_dimension_2d;
    assert_eq!(drawable_size(&gpu.device, (1156, 65527)), (1156, max));
    let (mut config, _) = surface_config(&surface, &gpu.adapter, drawable_size(&gpu.device, (64, 65527)), false).expect("a layer configuration");
    configure(&surface, &gpu, &config).expect("a size held to the device's limit takes");

    let mut refused = Configured::default();
    let bad = wgpu::SurfaceConfiguration { format: wgpu::TextureFormat::Rgba8Uint, height: 64, ..config.clone() };
    let attempt = refused.begin();
    let error = configure(&surface, &gpu, &bad).expect_err("a layer has no Rgba8Uint drawables");
    assert!(error.contains("Rgba8Uint"), "{error}");
    refused.answer(attempt, &Err(error), Instant::now());
    assert!(!refused.drawable(), "and gives no drawable until one takes");

    config.height = 64;
    let attempt = refused.begin();
    let result = configure(&surface, &gpu, &config);
    refused.answer(attempt, &result, Instant::now());
    assert!(result.is_ok() && refused.drawable());
    let mut meter = SurfaceMeter::new("refused", Arc::default());
    let frame = acquire_checked(&surface, &gpu, &config, true, 1, &mut meter, false).expect("a drawable once a configure took");
    let view = frame.texture.create_view(&Default::default());
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view, depth_slice: None, resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
        })],
        ..Default::default()
    });
    gpu.queue.submit([encoder.finish()]);
    gpu.queue.present(frame);
    refused.step(SurfaceEvent::Drew);
}
