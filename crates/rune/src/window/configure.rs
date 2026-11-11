//! Configuring a window's or overlay's surface, and saying so when it can't.

use crate::render::Gpu;

/// `(width, height)` held to what `device` makes a surface at: 1 to its
/// `max_texture_dimension_2d` pixels a side. Windows has reported a window resized as it appears
/// with a negative client height, wrapped to 65527.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) fn drawable_size(device: &wgpu::Device, (width, height): (u32, u32)) -> (u32, u32) {
    let max = device.limits().max_texture_dimension_2d.max(1);
    (width.clamp(1, max), height.clamp(1, max))
}

/// What a configure can fail with: a configuration the surface refuses, a driver's internal
/// error, and a swapchain it has no memory for (`ConfigureSurfaceError::Device(OutOfMemory)`).
const SCOPES: [wgpu::ErrorFilter; 3] = [wgpu::ErrorFilter::Validation, wgpu::ErrorFilter::Internal, wgpu::ErrorFilter::OutOfMemory];

/// Configures `surface`, returning wgpu's error instead of panicking in its default handler,
/// so a surface the driver refuses (an overlay's XWayland window on NVIDIA, hex#979) or runs
/// out of memory for costs only its owner. The message names the configuration for logs. Rune
/// configures a surface only here (`configure_tests` holds it to that).
///
/// A lost device fails here even when wgpu reports nothing: it drops the `DeviceLost` that
/// `Surface::configure` returns (wgpu-30.0.1 `handle_error_inner`, "will be surfaced via
/// callback") without recording the surface's error sink, so the surface has no presentation
/// and the next `get_current_texture` panics ("Surface is not configured for presentation").
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn configure(surface: &wgpu::Surface, gpu: &Gpu, config: &wgpu::SurfaceConfiguration) -> Result<(), String> {
    if gpu.is_lost() { return Err(lost_device(config)); }
    #[cfg(test)]
    if refusals::take() { return Err(format!("refused by a test ({}x{})", config.width, config.height)); }
    let [validation, internal, memory] = SCOPES.map(|filter| gpu.device.push_error_scope(filter));
    surface.configure(&gpu.device, config);
    // `Device::destroy` only marks the device invalid: its lost callback runs when the device
    // next maintains itself, at the soonest a poll. Without this, a device destroyed before it
    // reported itself configures nothing while `is_lost` still reads false, and the drawable
    // its surface is asked for panics.
    gpu.device.poll(wgpu::PollType::Poll).ok();
    let error = pollster::block_on(memory.pop()).or(pollster::block_on(internal.pop())).or(pollster::block_on(validation.pop()));
    if gpu.is_lost() { return Err(lost_device(config)); }
    error.map_or(Ok(()), |error| Err(refusal(&error, config)))
}

/// A configuration that never reached the surface because its device is gone.
#[cfg(not(target_arch = "wasm32"))]
fn lost_device(config: &wgpu::SurfaceConfiguration) -> String {
    format!("the GPU device was lost ({}x{}, {:?}, {:?}, {:?})", config.width, config.height, config.format, config.alpha_mode, config.present_mode)
}

/// `configure` on the web, answered now or later. On WebGPU the JS device's own error scopes
/// catch a refusal (`platform::web::health::Scopes`): wgpu's conversion of a
/// `GPUInternalError` panics. The browser answers later (`None`), and `answer` hears it then;
/// the canvas draws meanwhile. WebGL2 runs through wgpu-core, whose scopes have answered by the
/// time this returns (`Some`).
#[cfg(target_arch = "wasm32")]
pub(crate) fn configure_then(surface: &wgpu::Surface, gpu: &Gpu, config: &wgpu::SurfaceConfiguration,
    answer: impl FnOnce(Result<(), String>) + 'static) -> Option<Result<(), String>> {
    use std::{future::Future, task::{Context, Poll, Waker}};
    if let Some(scopes) = crate::platform::web::health::Scopes::push(&gpu.device) {
        surface.configure(&gpu.device, config);
        let named = config.clone();
        scopes.pop(move |error| answer(error.map_or(Ok(()), |cause| Err(described(&cause, &named)))));
        return None;
    }
    let [validation, internal, memory] = SCOPES.map(|filter| gpu.device.push_error_scope(filter));
    surface.configure(&gpu.device, config);
    let (memory, internal, validation) = (memory.pop(), internal.pop(), validation.pop());
    let mut popped = Box::pin(async move {
        let (memory, internal) = (memory.await, internal.await);
        memory.or(internal).or(validation.await)
    });
    let result = |error: Option<wgpu::Error>, config: &wgpu::SurfaceConfiguration| error.map_or(Ok(()), |error| Err(refusal(&error, config)));
    match popped.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(error) => Some(result(error, config)),
        Poll::Pending => {
            let named = config.clone();
            wasm_bindgen_futures::spawn_local(async move { answer(result(popped.await, &named)) });
            None
        }
    }
}

/// `configure_then`, logging a refusal the browser reports late. Only the web host configures
/// a surface on the web; overlays and windows that call this don't run there.
#[cfg(target_arch = "wasm32")]
pub(crate) fn configure(surface: &wgpu::Surface, gpu: &Gpu, config: &wgpu::SurfaceConfiguration) -> Result<(), String> {
    let log = |result: Result<(), String>| if let Err(error) = result {
        web_sys::console::error_1(&format!("rune: a surface refused its configuration: {error}").into());
    };
    configure_then(surface, gpu, config, log).unwrap_or(Ok(()))
}

/// wgpu's `error` for a refused `config`, on one line.
fn refusal(error: &wgpu::Error, config: &wgpu::SurfaceConfiguration) -> String {
    let text = error.to_string();
    described(&text.lines().map(str::trim).filter(|line| !line.is_empty() && *line != "Caused by:").collect::<Vec<_>>().join(": "), config)
}

/// `cause`, naming the configuration it refused.
fn described(cause: &str, config: &wgpu::SurfaceConfiguration) -> String {
    format!("{cause} ({}x{}, {:?}, {:?}, {:?})", config.width, config.height, config.format, config.alpha_mode, config.present_mode)
}

/// Refusals a test asks `configure` for on its thread, in place of asking wgpu.
#[cfg(test)]
pub(crate) mod refusals {
    std::thread_local! { static LEFT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }

    /// Refuses the next `count` configures on this thread.
    pub(crate) fn refuse(count: u32) { LEFT.set(count); }

    pub(super) fn take() -> bool {
        let left = LEFT.get();
        if left > 0 { LEFT.set(left - 1); }
        left > 0
    }
}
