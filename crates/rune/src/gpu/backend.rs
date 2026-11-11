//! Which backend an Apple app draws with: Rune's native Metal backend (`metal-backend`, on by
//! default, and the default on macOS 13 and later), or wgpu's.

use std::sync::atomic::{AtomicU8, Ordering};

/// The GPU backend an app draws with on macOS (`App::gpu_backend`) and iOS (`prewarm_gpu_on`, before
/// the first `embed::LayerHost`). Elsewhere wgpu picks one.
///
/// A Mac app that chooses neither draws with `Metal` when Rune's `metal-backend` feature is on (it
/// is by default), and with `Wgpu` when it's off; an iOS app draws with `Wgpu`. `Default` stays `Wgpu`; passing it to
/// `App::gpu_backend` opts out of the native backend. The native backend needs macOS 13 or iOS 16:
/// before them, `Metal` draws with wgpu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuBackend {
    /// wgpu's Metal backend.
    #[default]
    Wgpu,
    /// Rune's native Metal backend, with Rune's `metal-backend` feature; without it, wgpu.
    Metal,
}

const UNSET: u8 = 0;
const WGPU: u8 = 1;
const METAL: u8 = 2;

/// What the app chose: nothing yet, `Wgpu` or `Metal`.
#[cfg_attr(not(all(target_vendor = "apple", feature = "metal-backend")), allow(dead_code))]
static CHOSEN: AtomicU8 = AtomicU8::new(UNSET);

/// The backend an app that chooses none draws with: the native one on macOS with the feature.
const DEFAULT: GpuBackend = if cfg!(all(target_os = "macos", feature = "metal-backend")) { GpuBackend::Metal } else { GpuBackend::Wgpu };

/// Makes GPUs from now on with `backend`, process-wide. A `prewarm_gpu` running on another
/// backend starts over on this one.
#[cfg_attr(not(target_vendor = "apple"), allow(unused_variables))]
pub(crate) fn choose(backend: GpuBackend) {
    #[cfg(all(target_vendor = "apple", not(feature = "metal-backend")))]
    if backend == GpuBackend::Metal {
        eprintln!("rune: GpuBackend::Metal needs Rune's `metal-backend` feature; drawing with wgpu");
    }
    #[cfg(all(target_vendor = "apple", feature = "metal-backend"))]
    {
        let before = current();
        CHOSEN.store(match backend { GpuBackend::Wgpu => WGPU, GpuBackend::Metal => METAL }, Ordering::Relaxed);
        #[cfg(target_os = "macos")]
        if current() != before {
            crate::render::restart_prewarm();
        }
        #[cfg(not(target_os = "macos"))]
        let _ = before;
    }
}

fn chosen() -> Option<GpuBackend> {
    match CHOSEN.load(Ordering::Relaxed) {
        WGPU => Some(GpuBackend::Wgpu),
        METAL => Some(GpuBackend::Metal),
        _ => None,
    }
}

fn env() -> Option<String> {
    std::env::var("RUNE_GPU_BACKEND").ok()
}

/// The first OS version the native backend runs on: macOS 13 and iOS 16, which have MSL 3.0, the
/// version it writes shaders in and its precompiled libraries are built for.
const NATIVE_FLOOR: u32 = if cfg!(target_os = "ios") { 16 } else { 13 };

/// The OS's major version (macOS 13 is 13), or 0 where it can't be read.
#[cfg(target_vendor = "apple")]
pub(crate) fn os_major() -> u32 {
    static MAJOR: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *MAJOR.get_or_init(|| {
        let mut buf = [0u8; 32];
        let mut len = buf.len();
        // SAFETY: sysctlbyname writes at most `len` bytes into `buf` and the length it wrote into `len`.
        let ok = unsafe { libc::sysctlbyname(c"kern.osproductversion".as_ptr(), buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) } == 0;
        let version = if ok { String::from_utf8_lossy(&buf[..len.saturating_sub(1)]).into_owned() } else { String::new() };
        version.split('.').next().and_then(|m| m.trim().parse().ok()).unwrap_or(0)
    })
}

#[cfg(not(target_vendor = "apple"))]
fn os_major() -> u32 { 0 }

/// How a log names the OS whose major version is `os`.
fn os_name(os: u32) -> String {
    let name = if cfg!(target_os = "ios") { "iOS" } else { "macOS" };
    if os == 0 { format!("a {name} whose version Rune can't read") } else { format!("{name} {os}") }
}

#[cfg_attr(not(all(target_vendor = "apple", feature = "metal-backend")), allow(dead_code))]
/// The backend GPUs are made with: `RUNE_GPU_BACKEND` (`wgpu` or `metal`) over the app's
/// choice, over the default, and wgpu's on an OS the native backend doesn't run on. Asked for
/// there, it says so once.
pub(crate) fn current() -> GpuBackend {
    let (env, chosen, os) = (env(), chosen(), os_major());
    let backend = resolve(env.as_deref(), chosen, os);
    if backend != resolve(env.as_deref(), chosen, u32::MAX) && !defaulted(env.as_deref(), chosen) {
        static SAID: std::sync::Once = std::sync::Once::new();
        SAID.call_once(|| eprintln!("rune: Rune's Metal backend needs {} or later, and this is {}; drawing with wgpu", os_name(NATIVE_FLOOR), os_name(os)));
    }
    backend
}

/// What `RUNE_GPU_LOG` calls the backend `adapter` is on.
pub(crate) fn label(adapter: &wgpu::Adapter) -> &'static str {
    #[cfg(all(target_vendor = "apple", feature = "metal-backend"))]
    if crate::gpu::metal::owns_adapter(adapter) {
        return "rune backend";
    }
    let _ = adapter;
    "wgpu"
}

/// Whether `device` is the native Metal backend's.
pub(crate) fn native(device: &wgpu::Device) -> bool {
    #[cfg(all(target_vendor = "apple", feature = "metal-backend"))]
    return crate::gpu::metal::raw_device(device).is_some();
    #[cfg(not(all(target_vendor = "apple", feature = "metal-backend")))]
    {
        let _ = device;
        false
    }
}

/// Features a device on `adapter` gets whether or not a compositor asks: all the native backend
/// offers, which cost it nothing, so a prewarmed device serves any compositor it can draw for.
pub(crate) fn free_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    #[cfg(all(target_vendor = "apple", feature = "metal-backend"))]
    if crate::gpu::metal::owns_adapter(adapter) {
        return crate::gpu::metal::FEATURES;
    }
    let _ = adapter;
    wgpu::Features::empty()
}

/// The features the last compositor that fell back to wgpu wanted and the native backend lacks.
#[cfg_attr(not(all(target_vendor = "apple", feature = "metal-backend")), allow(dead_code))]
static MISSING: std::sync::Mutex<wgpu::Features> = std::sync::Mutex::new(wgpu::Features::empty());

/// Whether a GPU for a compositor that wants `wanted` draws with wgpu instead of the native
/// backend: only when the native backend is the default, neither the app nor the environment
/// asked for it, and it lacks one of `wanted`'s features. Says so once.
#[cfg_attr(not(all(target_vendor = "apple", feature = "metal-backend")), allow(unused_variables))]
pub(crate) fn wgpu_for(wanted: wgpu::Features) -> bool {
    #[cfg(all(target_vendor = "apple", feature = "metal-backend"))]
    {
        let missing = wanted - crate::gpu::metal::FEATURES;
        if !missing.is_empty() && DEFAULT == GpuBackend::Metal && defaulted(env().as_deref(), chosen()) {
            *MISSING.lock().unwrap_or_else(|e| e.into_inner()) = missing;
            static SAID: std::sync::Once = std::sync::Once::new();
            SAID.call_once(|| eprintln!("rune: the compositor wants {missing}, which Rune's Metal backend lacks; drawing with wgpu"));
            return true;
        }
    }
    false
}

/// Which backend draws on `device`, on which GPU, and why, for an app's log: "Rune's Metal
/// backend on Apple M4 (the default)", or "wgpu Metal on Apple M4 (a compositor wants
/// POLYGON_MODE_LINE, which Rune's Metal backend lacks)". A window's compositor gets its device in
/// `compose::Frame::device`. `RUNE_GPU_LOG=1` prints it for each window and overlay GPU Rune makes.
pub fn describe(device: &wgpu::Device) -> String {
    let info = device.adapter_info();
    let backend = if native(device) { "Rune's Metal backend".to_string() } else { format!("wgpu {:?}", info.backend) };
    let why = reason(native(device));
    if why.is_empty() { format!("{backend} on {}", info.name) } else { format!("{backend} on {} ({why})", info.name) }
}

/// Why the backend is the native one (`native`) or wgpu's; empty where wgpu is the only one.
fn reason(native: bool) -> String {
    #[cfg(all(target_vendor = "apple", feature = "metal-backend"))]
    {
        let missing = *MISSING.lock().unwrap_or_else(|e| e.into_inner());
        return why(native, env().as_deref(), chosen(), missing, os_major());
    }
    #[cfg(all(target_vendor = "apple", not(feature = "metal-backend")))]
    return if native { String::new() } else { "Rune's metal-backend feature is off".into() };
    #[cfg(not(target_vendor = "apple"))]
    {
        let _ = native;
        String::new()
    }
}

/// `reason` for a backend chosen from `env`, the app's choice, the features a compositor wanted
/// and the native backend lacks, and the OS's major version.
#[cfg_attr(not(all(target_vendor = "apple", feature = "metal-backend")), allow(dead_code))]
fn why(native: bool, env: Option<&str>, chosen: Option<GpuBackend>, missing: wgpu::Features, os: u32) -> String {
    match (native, env, chosen) {
        (true, Some("metal"), _) => "RUNE_GPU_BACKEND=metal".into(),
        (true, _, Some(GpuBackend::Metal)) => "App::gpu_backend chose it".into(),
        (true, ..) => "the default".into(),
        // As `resolve` decides: the OS floor over whatever asked for the native backend, then
        // the environment over the app.
        (false, ..) if resolve(env, chosen, os) != resolve(env, chosen, u32::MAX) =>
            format!("Rune's Metal backend needs {} or later, and this is {}", os_name(NATIVE_FLOOR), os_name(os)),
        (false, Some("wgpu"), _) => "RUNE_GPU_BACKEND=wgpu".into(),
        (false, env, Some(GpuBackend::Wgpu)) if !matches!(env, Some("metal" | "wgpu")) => "App::gpu_backend chose it".into(),
        (false, ..) if DEFAULT == GpuBackend::Wgpu => "the default here".into(),
        (false, ..) if !missing.is_empty() => format!("a compositor wants {missing}, which Rune's Metal backend lacks"),
        (false, ..) => String::new(),
    }
}

/// Whether a GPU on `device` serves a compositor that wants `wanted`: false for a native
/// backend device when `wgpu_for(wanted)`.
pub(crate) fn serves(device: &wgpu::Device, wanted: wgpu::Features) -> bool {
    #[cfg(all(target_vendor = "apple", feature = "metal-backend"))]
    if crate::gpu::metal::raw_device(device).is_some() {
        return !wgpu_for(wanted);
    }
    let _ = (device, wanted);
    true
}

/// The backend `env` (`RUNE_GPU_BACKEND`) asks for, over the app's choice, over the default:
/// wgpu's on an OS whose major version `os` the native backend doesn't run on.
#[cfg_attr(not(all(target_vendor = "apple", feature = "metal-backend")), allow(dead_code))]
fn resolve(env: Option<&str>, chosen: Option<GpuBackend>, os: u32) -> GpuBackend {
    let backend = match env {
        Some("metal") => GpuBackend::Metal,
        Some("wgpu") => GpuBackend::Wgpu,
        _ => chosen.unwrap_or(DEFAULT),
    };
    if os < NATIVE_FLOOR { GpuBackend::Wgpu } else { backend }
}

/// Whether the backend is the default one, which neither the environment nor the app chose.
#[cfg_attr(not(all(target_vendor = "apple", feature = "metal-backend")), allow(dead_code))]
fn defaulted(env: Option<&str>, chosen: Option<GpuBackend>) -> bool {
    !matches!(env, Some("metal" | "wgpu")) && chosen.is_none()
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
