//! Images larger than the GPU's largest texture: placed smaller, never made into a texture the
//! device refuses.

use super::*;

/// Top half red, bottom half green, `w` × `h`.
fn halves(w: u32, h: u32) -> Vec<u8> {
    (0..h).flat_map(|y| std::iter::repeat_n(if y < h / 2 { [255, 0, 0, 255] } else { [0, 255, 0, 255] }, w as usize)).flatten().collect()
}

/// A host that draws a 4096 × 4096 image once, on a device whose textures are at most 2048
/// pixels a side, sees it shrunk on that frame, with no validation error: the atlas places it
/// to fit, though nothing told it and the renderer made for that device was made on another
/// thread.
#[test]
fn an_image_larger_than_the_gpus_textures_draws_shrunk_without_a_validation_error() {
    use crate::{Cx, El, Env, Runtime, el};
    use crate::render::{OFFSCREEN_FORMAT, Offscreen, Renderer};
    fn view(s: &Pixels, _: &mut Cx<Pixels>) -> El { el().size(8.0, 8.0).pixels(s) }
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::PRIMARY;
    let instance = wgpu::Instance::new(descriptor);
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else { return eprintln!("skipped: no wgpu adapter") };
    let required_limits = wgpu::Limits { max_texture_dimension_2d: 2048, ..adapter.limits() };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { required_limits, ..Default::default() }))
        .expect("a device limited to 2048 px textures");
    let image = Pixels::new();
    image.set(4096, 4096, halves(4096, 4096));
    let mut rt = Runtime::new(image, view, (8.0, 8.0), Env::new(true, 1.0));
    std::thread::scope(|s| { s.spawn(|| drop(Renderer::new(&device, OFFSCREEN_FORMAT))); });
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    rt.frame(0.0);
    assert!(rt.images.pages.iter().all(|page| page.width.max(page.height) <= 2048), "placed to fit before this thread made a renderer");
    let mut renderer = Renderer::new(&device, OFFSCREEN_FORMAT);
    let target = Offscreen::new(&device, 8, 8);
    renderer.upload_images(&device, &queue, &mut rt.images);
    renderer.prepare(&device, &queue, &mut rt.text.atlas, &rt.instances, (8, 8));
    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.draw(&mut encoder, &target.view, [0.0, 0.0, 1.0, 1.0]);
    target.copy(&mut encoder);
    queue.submit([encoder.finish()]);
    let out = target.read(&device);
    if let Some(error) = pollster::block_on(scope.pop()) { panic!("validation error: {error}"); }
    let pixel = |x: usize, y: usize| <[u8; 4]>::try_from(&out[(y * 8 + x) * 4..(y * 8 + x + 1) * 4]).unwrap();
    assert_eq!((pixel(4, 1), pixel(4, 6)), ([255, 0, 0, 255], [0, 255, 0, 255]), "the image draws, shrunk, on its first frame");
    assert!(rt.images.pages.iter().all(|page| page.width.max(page.height) <= 2048));
}

/// Told the limit, an atlas places a streamed image larger than it halved as its mips would
/// be, in linear light, and a decoded one at its largest mip that fits, counting each copy;
/// one that fits is untouched.
#[test]
fn an_atlas_places_images_no_larger_than_its_limit() {
    let mut atlas = ImageAtlas::default();
    atlas.set_max_side(2048);
    let pixel = |atlas: &ImageAtlas, placed: Placed| {
        let page = &atlas.pages[placed.page];
        let [x, y, ..] = placed.rect.map(|v| v as usize);
        <[u8; 4]>::try_from(&page.data[(y * page.width as usize + x) * 4..][..4]).unwrap()
    };
    let streamed = Pixels::new();
    streamed.set(4096, 4096, halves(4096, 4096));
    let placed = atlas.place(&streamed, [8.0, 8.0]).expect("placed shrunk, not dropped");
    assert_eq!(placed.rect, [0, 0, 2048, 2048]);
    let page = &atlas.pages[placed.page];
    assert_eq!((page.width, page.height), (2048, 2048));
    let at = |x: usize, y: usize| &page.data[(y * 2048 + x) * 4..][..4];
    assert_eq!((at(5, 5), at(5, 2000)), (&[255, 0, 0, 255][..], &[0, 255, 0, 255][..]));
    assert_eq!(atlas.too_large, 1);
    atlas.place(&streamed, [8.0, 8.0]);
    assert_eq!(atlas.too_large, 1, "an unchanged frame isn't copied again");

    // Black beside white halves to the grey of half their light, as a mip does, not to the
    // average of their bytes (128).
    let stripes = Pixels::new();
    stripes.set(4096, 1, [[0, 0, 0, 255], [255, 255, 255, 255]].repeat(2048).concat());
    let placed = atlas.place(&stripes, [8.0, 1.0]).unwrap();
    assert!(placed.premultiplied && placed.rect[2..] == [2048, 1]);
    let grey = pixel(&atlas, placed);
    assert!(grey[..3].iter().all(|&c| c.abs_diff(188) <= 1) && grey[3] == 255, "{grey:?}");

    // Transparent white beside opaque blue: no grey bleeds in, and the pixel is the one the
    // mip chain makes of the same pair.
    let pair = [[255, 255, 255, 0], [0, 0, 255, 255]].concat();
    let edge = Pixels::new();
    edge.set(4096, 1, pair.repeat(2048));
    let shrunk = atlas.place(&edge, [8.0, 1.0]).unwrap();
    let cache = crate::image::ImageCache::new(Arc::new(|| {}));
    let mip = cache.rgba(2, 1, pair.clone());
    mip.wait().unwrap();
    let mip = atlas.place(mip.pixels(), [1.0, 1.0]).unwrap();
    assert_eq!(mip.rect[2..], [1, 1]);
    assert_eq!(pixel(&atlas, shrunk), pixel(&atlas, mip));
    assert_eq!(atlas.too_large, 3);

    let decoded = cache.rgba(4096, 4, halves(4096, 4));
    decoded.wait().unwrap();
    let placed = atlas.place(decoded.pixels(), [4096.0, 4.0]).unwrap();
    assert_eq!(placed.rect[2..], [2048, 2], "its mip 1, though the drawn size wants mip 0");
    assert_eq!(atlas.too_large, 4);

    let small = Pixels::new();
    small.set(2048, 2, halves(2048, 2));
    assert_eq!(atlas.place(&small, [2048.0, 2.0]).unwrap().rect[2..], [2048, 2]);
    assert_eq!(atlas.too_large, 4, "an image that fits isn't counted");
}
