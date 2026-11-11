use super::{DEFAULT, GpuBackend, defaulted, resolve, why};

/// A current macOS, where the native backend runs.
const OS: u32 = 26;

#[test]
fn the_environment_overrides_the_app() {
    assert_eq!(resolve(None, Some(GpuBackend::Metal), OS), GpuBackend::Metal);
    assert_eq!(resolve(Some("wgpu"), Some(GpuBackend::Metal), OS), GpuBackend::Wgpu);
    assert_eq!(resolve(Some("metal"), Some(GpuBackend::Wgpu), OS), GpuBackend::Metal);
    assert_eq!(resolve(Some(""), Some(GpuBackend::Metal), OS), GpuBackend::Metal);
}

#[test]
fn an_app_that_chooses_wgpu_keeps_it_over_the_default() {
    assert_eq!(resolve(None, Some(GpuBackend::Wgpu), OS), GpuBackend::Wgpu);
    assert_eq!(resolve(None, None, OS), DEFAULT);
    assert_eq!(resolve(Some("bogus"), None, OS), DEFAULT);
    assert!(defaulted(None, None));
    assert!(!defaulted(None, Some(GpuBackend::Metal)));
    assert!(!defaulted(Some("metal"), None));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_draws_with_the_native_backend_by_default_with_the_feature() {
    let expected = if cfg!(feature = "metal-backend") { GpuBackend::Metal } else { GpuBackend::Wgpu };
    assert_eq!(DEFAULT, expected);
}

/// The native backend needs MSL 3.0, so macOS 13: before it, the default, the app's choice
/// and `RUNE_GPU_BACKEND=metal` all draw with wgpu, and `describe_gpu` says why. A version
/// Rune can't read counts as too old.
#[cfg(not(target_os = "ios"))]
#[test]
fn the_native_backend_draws_from_macos_13() {
    use GpuBackend::{Metal, Wgpu};
    let by_default = |want| if DEFAULT == Metal { want } else { Wgpu };
    for (os, want) in [(11, Wgpu), (12, Wgpu), (13, Metal), (26, Metal), (0, Wgpu)] {
        assert_eq!(resolve(None, None, os), by_default(want), "the default on macOS {os}");
        assert_eq!(resolve(None, Some(Metal), os), want, "App::gpu_backend(Metal) on macOS {os}");
        assert_eq!(resolve(Some("metal"), None, os), want, "RUNE_GPU_BACKEND=metal on macOS {os}");
        assert_eq!(resolve(Some("wgpu"), Some(Metal), os), Wgpu);
    }
    let none = wgpu::Features::empty();
    assert_eq!(why(false, Some("metal"), None, none, 12), "Rune's Metal backend needs macOS 13 or later, and this is macOS 12");
    assert_eq!(why(false, None, Some(Metal), none, 11), "Rune's Metal backend needs macOS 13 or later, and this is macOS 11");
    assert_eq!(why(false, Some("wgpu"), None, none, 12), "RUNE_GPU_BACKEND=wgpu", "the environment's choice is the reason");
    assert_eq!(why(false, Some("metal"), Some(Wgpu), none, 12), "Rune's Metal backend needs macOS 13 or later, and this is macOS 12",
        "the environment overrides the app, and the OS floor overrides both");
    assert_eq!(why(false, None, Some(Wgpu), none, 12), "App::gpu_backend chose it", "wgpu asked for is no fallback");
    if DEFAULT == Metal {
        assert_eq!(why(false, None, None, none, 12), "Rune's Metal backend needs macOS 13 or later, and this is macOS 12");
        assert_eq!(why(true, None, None, none, 13), "the default");
    }
}
