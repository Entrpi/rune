use super::*;
use std::time::Duration;

const RED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#f00"/></svg>"##;

/// An 8 px SVG drawing nothing but an `<image>` from `href`, over its whole canvas.
fn with_image(href: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8"><image width="8" height="8" xlink:href="{href}"/></svg>"#)
}

fn red(rgba: &[u8]) -> bool {
    rgba.chunks_exact(4).any(|p| p[0] > 200 && p[3] > 200)
}

/// `data:` with every byte percent-encoded.
fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime},{}", bytes.iter().map(|b| format!("%{b:02X}")).collect::<String>())
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rune-svg-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn an_image_href_draws_a_data_image_but_reads_a_local_file_only_when_trusted() {
    let (_, _, rgba) = svg_rgba(with_image(&data_url("image/svg+xml", RED.as_bytes())).as_bytes(), 8, false).unwrap();
    assert!(red(&rgba), "a data: SVG image draws");
    let dir = scratch("file-href");
    let file = dir.join("inner.svg");
    std::fs::write(&file, RED).unwrap();
    let outer = with_image(file.to_str().unwrap());
    let (_, _, rgba) = svg_rgba(outer.as_bytes(), 8, false).unwrap();
    assert!(!red(&rgba), "an untrusted SVG draws nothing from a local file");
    let (_, _, rgba) = svg_rgba(outer.as_bytes(), 8, true).unwrap();
    assert!(red(&rgba), "a trusted SVG draws the file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A FIFO with no writer blocks whoever opens it to read: the decode must never try, trusted
/// or not. On a failure the test opens the FIFO to write, so the stuck decode can finish.
#[cfg(unix)]
#[test]
fn a_fifo_href_blocks_no_thread() {
    let dir = scratch("fifo-href");
    let fifo = dir.join("pipe.svg");
    let path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0, "mkfifo");
    for files in [false, true] {
        let outer = with_image(fifo.to_str().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || { let _ = tx.send(svg_rgba(outer.as_bytes(), 8, files).map(|_| ())); });
        let done = rx.recv_timeout(Duration::from_secs(10));
        if done.is_err() {
            let _ = std::fs::OpenOptions::new().write(true).custom_flags(libc::O_NONBLOCK).open(&fifo);
        }
        assert_eq!(done, Ok(Ok(())), "files: {files}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;

#[test]
fn an_svgz_bomb_stops_inflating_at_its_cap() {
    use std::io::Write;
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">"#).unwrap();
    let spaces = vec![b' '; 1 << 20];
    for _ in 0..=(MAX_INFLATED >> 20) { gz.write_all(&spaces).unwrap(); }
    gz.write_all(b"</svg>").unwrap();
    let bomb = gz.finish().unwrap();
    assert!(bomb.len() < 1 << 20, "{} bytes of gzip", bomb.len());
    let e = svg_rgba(&bomb, 8, false).unwrap_err();
    assert!(e.0.contains("inflates past 64 MiB"), "{e}");
}

/// Ten copies of the level below, six levels deep: 1.6 KB that usvg would expand to a million
/// nodes, and five levels to 222,222, which it would build and draw.
fn fan_out(levels: usize) -> String {
    let mut s = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8"><defs><rect id="l0" width="1" height="1"/>"#);
    for l in 1..=levels {
        s += &format!(r#"<g id="l{l}">"#);
        for _ in 0..10 { s += &format!(r##"<use xlink:href="#l{}"/>"##, l - 1); }
        s += "</g>";
    }
    s + &format!(r##"</defs><use xlink:href="#l{levels}"/></svg>"##)
}

#[test]
fn a_use_fan_out_is_refused_by_the_node_budget() {
    // Five levels expand to 222,222 nodes inside usvg; the post-parse node budget refuses them.
    assert!(run(fan_out(5)).unwrap_err().0.contains("more than 100000 nodes"));
    // Three levels (1,111 nodes) draw.
    assert!(run(fan_out(3)).is_ok());
}


fn run(svg: String) -> Result<(), ImageError> {
    std::thread::Builder::new().stack_size(2 << 20).spawn(move || svg_rgba(svg.as_bytes(), 8, false).map(|_| ())).unwrap().join().unwrap()
}

/// An 8 px SVG with `defs` and then `body`.
fn svg(defs: &str, body: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8"><defs>{defs}</defs>{body}</svg>"#)
}

/// roxmltree parses, usvg converts and resvg draws a level of nesting in a Rust stack frame or
/// more: a thousand nested `<svg>` overflow a decode thread's 2 MiB stack, which aborts.
#[test]
fn deep_nesting_is_refused_on_a_decode_threads_stack() {
    let nest = |depth: usize, tag: &str| format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">{}<rect width="8" height="8"/>{}</svg>"#, format!("<{tag}>").repeat(depth), format!("</{tag}>").repeat(depth));
    for tag in ["svg", "g"] {
        let e = run(nest(1000, tag)).unwrap_err();
        assert!(e.0.contains("nests deeper than 64 elements"), "{tag}: {e}");
        assert!(run(nest(MAX_DEPTH - 2, tag)).is_ok(), "{tag}");
    }
    // Closing tags in a comment close nothing, and markup an entity brings counts too.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">"#;
    let commented = format!("{svg}{}<!--{}-->{}<rect/>{}</svg>", "<g>".repeat(40), "</g>".repeat(40), "<g>".repeat(40), "</g>".repeat(80));
    assert!(run(commented).unwrap_err().0.contains("nests deeper"));
    let deep = |name: &str, inner: &str| format!(r#"<!ENTITY {name} "{}{inner}{}">"#, "<g>".repeat(40), "</g>".repeat(40));
    let entities = format!("<!DOCTYPE svg [{}{}]>{svg}&e1;</svg>", deep("e2", ""), deep("e1", "&e2;"));
    assert!(run(entities).unwrap_err().0.contains("nests deeper"));
    // Entities nothing reaches count nothing, even in a chain: `alias` holds `&deep;` but the
    // body references neither.
    let unused = format!(r#"<!DOCTYPE svg [<!ENTITY deep "{}{}"><!ENTITY alias "&deep;">]>{svg}<rect width="8" height="8"/></svg>"#, "<g>".repeat(70), "</g>".repeat(70));
    assert!(run(unused).is_ok());
    // But an alias the body does reach is followed to its deep entity.
    let reached = format!(r#"<!DOCTYPE svg [<!ENTITY deep "{}{}"><!ENTITY alias "&deep;">]>{svg}&alias;</svg>"#, "<g>".repeat(70), "</g>".repeat(70));
    assert!(run(reached).unwrap_err().0.contains("nests deeper"));
    // A nested SVG image draws inside its parent's levels; both at their limit fit the stack.
    let inner = nest(MAX_DEPTH - 2, "svg");
    let outer = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8">{}<image width="8" height="8" xlink:href="{}"/>{}</svg>"#, "<svg>".repeat(MAX_DEPTH - 3), data_url("image/svg+xml", inner.as_bytes()), "</svg>".repeat(MAX_DEPTH - 3));
    assert!(run(outer).is_ok());
}

/// A PNG's signature and header claiming `w`×`h`, which usvg sizes an image by.
fn png_header(w: u32, h: u32) -> Vec<u8> {
    let mut ihdr = b"IHDR".to_vec();
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    let mut crc = flate2::Crc::new();
    crc.update(&ihdr);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(13u32.to_be_bytes());
    png.extend(ihdr);
    png.extend(crc.sum().to_be_bytes());
    png
}

#[test]
fn an_oversized_data_image_is_refused() {
    let e = svg_rgba(with_image(&data_url("image/png", &png_header(100_000, 100_000))).as_bytes(), 8, false).unwrap_err();
    assert!(e.0.contains("images and patterns are too large"), "{e}");
    // Its pixels never wrap past the budget.
    let e = svg_rgba(with_image(&data_url("image/png", &png_header(u32::MAX, u32::MAX))).as_bytes(), 8, false).unwrap_err();
    assert!(e.0.contains("images and patterns are too large"), "{e}");
    assert!(svg_rgba(with_image(&data_url("image/png", &png_header(64, 64))).as_bytes(), 8, false).is_ok());
    // A nested SVG draws on a canvas of its own the size of its parent's: five at 4096 px are
    // past the budget of four.
    let image = format!(r#"<image width="8" height="8" xlink:href="{}"/>"#, data_url("image/svg+xml", RED.as_bytes()));
    let five = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8">{}</svg>"#, image.repeat(5));
    assert!(svg_rgba(five.as_bytes(), 64, false).is_ok());
    assert!(svg_rgba(five.as_bytes(), MAX_SIDE, false).unwrap_err().0.contains("images and patterns are too large"));
}

/// resvg draws a pattern's tile at its own size before filling with it: a 20,000 px tile in an
/// 8 px SVG is a 1.6 GB pixmap.
#[test]
fn a_pattern_tile_counts_against_the_pixel_budget() {
    let tiled = |side: u32| svg(&format!(r#"<pattern id="p" patternUnits="userSpaceOnUse" width="{side}" height="{side}"><rect width="1" height="1"/></pattern>"#), r#"<rect width="8" height="8" fill="url(#p)"/>"#);
    assert!(svg_rgba(tiled(20_000).as_bytes(), 8, false).unwrap_err().0.contains("images and patterns are too large"));
    assert!(svg_rgba(tiled(4).as_bytes(), 8, false).is_ok());
}

/// usvg follows a clip path's clip path, a mask's mask and the like into the next, and catches
/// only cycles of one or two: a ring of three or more recurses until the stack overflows. The
/// scan reads each reference as usvg does (svgtypes for `url()`, simplecss for stylesheets), so
/// the ring is found however it is spelled.
#[test]
fn every_raster_is_clamped_to_4096_pixels_a_side() {
    let wide = br##"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="1"><rect width="1000" height="1" fill="#f00"/></svg>"##;
    let (w, h, _) = svg_rgba(wide, 20_000, false).unwrap();
    assert_eq!((w, h), (MAX_SIDE, 4));
    // `url_set` asks for a source's listed width.
    let dir = scratch("url-set");
    let file = dir.join("wide.svg");
    std::fs::write(&file, wide).unwrap();
    let cache = super::super::ImageCache::new(std::sync::Arc::new(|| {}));
    let image = cache.url_set(&[(file.to_str().unwrap(), 20_000)], 100.0, 1.0);
    assert_eq!(image.wait(), Ok([MAX_SIDE, 4]));
    let _ = std::fs::remove_dir_all(&dir);
}
