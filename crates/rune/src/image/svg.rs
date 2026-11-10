//! SVG and SVGZ to RGBA, bounded for an SVG from anywhere (a web response, a chat message, a
//! model's reply). An `<image>` draws only a `data:` image unless the cache trusts its SVGs'
//! files ([`ImageCache::svg_file_images`](super::ImageCache::svg_file_images)), and then only
//! a regular file. SVGZ inflates through a cap. A raw nesting-depth pre-pass refuses a document
//! that would overflow roxmltree's recursive parse. After usvg builds the tree, its nodes and
//! the pixels its images and patterns would draw are counted, and refused past a budget, and a
//! raster is at most [`MAX_SIDE`] a side.
//!
//! What this does not stop: a reference cycle or loop among clip paths, masks, filters, gradients
//! or patterns is resolved inside usvg 0.45 before any of these bounds run. usvg's own cycle
//! check catches only one- and two-element cases, so a longer ring overflows the decode thread's
//! stack (which aborts the app) and a gradient href loop hangs it. That is a usvg bug, fixed
//! upstream by linebender/resvg#1225 (an iterative cycle search over the resolved tree) and
//! #1223 (a configurable parse depth); Rune gets the fix by upgrading resvg, not by parsing the
//! document itself.
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

use resvg::usvg::roxmltree::{Document, ParsingOptions};
use resvg::usvg::{ImageHrefResolver, ImageKind, Node, Options, Paint, Transform, Tree, filter};
use resvg::{tiny_skia, usvg};

use super::ImageError;

/// The largest side of an SVG's raster, whatever size it is asked for.
const MAX_SIDE: u32 = 4096;
/// Encoded SVG or SVGZ bytes.
const MAX_BYTES: usize = 32 << 20;
/// Markup an SVGZ, and the SVGZ images nested in it, inflate to.
const MAX_INFLATED: usize = 64 << 20;
/// Nodes in the tree usvg builds, `<use>` copies and every instance included. roxmltree also
/// caps its own XML nodes here, so a `<use>` fan-out usvg would expand past this builds nothing.
const MAX_NODES: usize = 100_000;
/// Elements nested inside one another. roxmltree parses a level in a Rust stack frame or more,
/// on a decode thread's 2 MiB stack; a nested SVG image draws inside its parent's levels, so the
/// two together stay within 128.
const MAX_DEPTH: usize = 64;
/// Pixels an SVG's images cost to draw: a raster's own, and a nested SVG's whole canvas, which
/// resvg draws it on. Four squares of [`MAX_SIDE`].
const MAX_IMAGE_PIXELS: u64 = 4 * MAX_SIDE as u64 * MAX_SIDE as u64;

/// An SVG scaled to fit `px` square (at most [`MAX_SIDE`]), keeping its aspect ratio, as
/// straight-alpha sRGB RGBA8. `px == 0` rasterizes at twice the drawing's own size, at most
/// [`MAX_SIDE`]. `files` lets an `<image>` read a local file.
pub(crate) fn svg_rgba(bytes: &[u8], px: u32, files: bool) -> Result<(u32, u32, Vec<u8>), ImageError> {
    if bytes.len() > MAX_BYTES { return Err(ImageError("SVG is too large".into())); }
    let budget = Arc::new(Budget { inflated: MAX_INFLATED.into(), refused: Mutex::default() });
    let opt = Options { image_href_resolver: resolver(files, budget.clone()), ..Options::default() };
    let tree = parse(bytes, &opt, &budget).map_err(|(_, e)| e)?;
    if let Some(e) = budget.refused.lock().unwrap_or_else(|e| e.into_inner()).take() { return Err(e); }
    let size = tree.size();
    let px = if px == 0 { (size.width().max(size.height()) * 2.0).ceil().clamp(1.0, MAX_SIDE as f32) as u32 } else { px.min(MAX_SIDE) };
    let k = px as f32 / size.width().max(size.height());
    let (w, h) = (((size.width() * k).round() as u32).clamp(1, px), ((size.height() * k).round() as u32).clamp(1, px));
    weigh(&tree, k, u64::from(w) * u64::from(h))?;
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or_else(|| ImageError("empty SVG".into()))?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(k, k), &mut pixmap.as_mut());
    Ok((w, h, pixmap.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect()))
}

/// What one SVG and the SVGs nested in it may still spend. A nested image parses inside usvg's
/// parse of its parent, through the href resolver, so a refusal there (its SVGZ inflate cap or
/// its nesting-depth pre-pass) is kept here.
struct Budget {
    inflated: AtomicUsize,
    refused: Mutex<Option<ImageError>>,
}

/// Takes `n` from what is `left`, or leaves it and returns false.
fn spend(left: &AtomicUsize, n: usize) -> bool {
    left.try_update(Relaxed, Relaxed, |l| l.checked_sub(n)).is_ok()
}

/// `data` (SVG markup, or SVGZ) as usvg's tree, refused before usvg builds it when it would
/// inflate, expand or nest past the budget. An error says whether it was past a budget (true)
/// or not an SVG at all.
fn parse(data: &[u8], opt: &Options, budget: &Budget) -> Result<Tree, (bool, ImageError)> {
    let over = |why: String| (true, ImageError(why));
    let inflated;
    let data = if data.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        let left = budget.inflated.load(Relaxed) as u64;
        flate2::read::GzDecoder::new(data).take(left + 1).read_to_end(&mut out).map_err(|_| (false, ImageError("SVGZ is not valid gzip".into())))?;
        if !spend(&budget.inflated, out.len()) { return Err(over(format!("SVGZ inflates past {} MiB", MAX_INFLATED >> 20))); }
        inflated = out;
        &inflated[..]
    } else { data };
    let text = std::str::from_utf8(data).map_err(|_| (false, ImageError("SVG is not UTF-8".into())))?;
    // roxmltree parses an element's content in a call of its own: a deep document must fail
    // before it parses.
    if markup_depth(text) > MAX_DEPTH { return Err(over(format!("SVG nests deeper than {MAX_DEPTH} elements"))); }
    let doc = Document::parse_with_options(text, ParsingOptions { allow_dtd: true, nodes_limit: MAX_NODES as u32 })
        .map_err(|e| (matches!(e, usvg::roxmltree::Error::NodesLimitReached), ImageError(format!("SVG: {e}"))))?;
    Tree::from_xmltree(&doc, opt).map_err(|e| (false, ImageError(format!("SVG: {e}"))))
}

/// How deep `text`'s elements nest, read from the markup alone, never less than they do, so a
/// document that would overflow roxmltree's recursive parse fails before it parses. Comments,
/// CDATA and processing instructions hold no elements. roxmltree expands an entity reference ten
/// deep, so the deepest markup among the entities the body can reach (directly, or through
/// another reachable entity) counts ten times; an entity nothing reaches counts nothing.
fn markup_depth(text: &str) -> usize {
    let (body, entities) = split_dtd(text);
    let deepest = nesting(body);
    // Entities reachable from the body, transitively.
    let mut reached: Vec<bool> = vec![false; entities.len()];
    let index: std::collections::HashMap<&str, usize> = entities.iter().enumerate().map(|(k, (n, _))| (*n, k)).collect();
    let mut queue: Vec<usize> = refs(body).filter_map(|n| index.get(n).copied()).collect();
    while let Some(k) = queue.pop() {
        if std::mem::replace(&mut reached[k], true) { continue; }
        queue.extend(refs(entities[k].1).filter_map(|n| index.get(n).copied()));
    }
    let via = entities.iter().zip(&reached).filter(|&(_, &r)| r).map(|((_, v), _)| nesting(v)).max().unwrap_or(0);
    deepest + 10 * via
}

/// The names an entity reference (`&name;`) uses in `text`, skipping declarations: a `<` before
/// the next `;` is markup, not a reference.
fn refs(text: &str) -> impl Iterator<Item = &str> {
    text.split('&').skip(1).filter_map(|s| {
        let end = s.find(|c| c == ';' || c == '<')?;
        (s.as_bytes()[end] == b';' && !s[..end].is_empty()).then(|| &s[..end])
    })
}

/// The deepest element nesting in `markup`, read in one pass, comments and CDATA and PIs skipped.
fn nesting(markup: &str) -> usize {
    let b = markup.as_bytes();
    let past = |from: usize, end: &str| markup[from..].find(end).map_or(b.len(), |j| from + j + end.len());
    let (mut depth, mut deepest, mut i) = (0usize, 0usize, 0);
    while let Some(j) = markup[i..].find('<') {
        let at = i + j;
        let rest = &markup[at..];
        i = if rest.starts_with("<!--") { past(at + 4, "-->") }
        else if rest.starts_with("<![CDATA[") { past(at, "]]>") }
        else if rest.starts_with("<?") { past(at, "?>") }
        else if rest.starts_with("</") { depth = depth.saturating_sub(1); past(at, ">") }
        else if rest.starts_with("<!") { past(at, ">") }
        else {
            depth += 1;
            deepest = deepest.max(depth);
            let end = tag_end(b, at + 1);
            if b[..end].ends_with(b"/>") { depth -= 1; }
            end
        };
    }
    deepest
}

/// `text` split into (the body outside any DTD, the entities the DTD declares as name and
/// literal value), in one pass. A DTD's internal subset is `<!DOCTYPE … [ … ]>` before the root.
fn split_dtd(text: &str) -> (&str, Vec<(&str, &str)>) {
    let b = text.as_bytes();
    let mut entities = Vec::new();
    let mut i = 0;
    // Find the DOCTYPE, past any leading comments, PIs and whitespace.
    loop {
        let Some(j) = text[i..].find('<') else { return (text, entities) };
        let at = i + j;
        let rest = &text[at..];
        if rest.starts_with("<!--") { i = text[at + 4..].find("-->").map_or(b.len(), |k| at + 4 + k + 3); continue; }
        if rest.starts_with("<?") { i = text[at..].find("?>").map_or(b.len(), |k| at + k + 2); continue; }
        if !rest.starts_with("<!DOCTYPE") { return (text, entities); }
        i = at;
        break;
    }
    // Without an internal subset the DTD ends at its own `>`.
    let Some(open) = text[i..].find(['[', '>']).filter(|&k| b[i + k] == b'[') else {
        return (&text[tag_end(b, i + 1)..], entities);
    };
    let mut k = i + open + 1;
    while k < b.len() && b[k] != b']' {
        let dr = &text[k..];
        k = if dr.starts_with("<!--") { text[k + 4..].find("-->").map_or(b.len(), |m| k + 4 + m + 3) }
        else if dr.starts_with("<!ENTITY") {
            let end = tag_end(b, k + 1);
            if let Some(ent) = entity(&text[k..end]) { entities.push(ent); }
            end
        } else if dr.starts_with('<') { tag_end(b, k + 1) }
        else { k + 1 };
    }
    // The body starts after the DTD's `]>`.
    (&text[tag_end(b, k)..], entities)
}

/// `<!ENTITY [%] name "value">` as (name, value), when its value is a literal.
fn entity(decl: &str) -> Option<(&str, &str)> {
    let rest = decl.strip_prefix("<!ENTITY")?.trim_start();
    let rest = rest.strip_prefix('%').map_or(rest, str::trim_start);
    let name_end = rest.find(char::is_whitespace)?;
    let after = rest[name_end..].trim_start();
    let quote = after.chars().next().filter(|&c| c == '"' || c == '\'')?;
    let value_end = after[1..].find(quote)?;
    Some((&rest[..name_end], &after[1..1 + value_end]))
}

/// From `i`, the index just past the first `>` outside quotes.
fn tag_end(b: &[u8], mut i: usize) -> usize {
    while i < b.len() {
        match b[i] {
            b'>' => return i + 1,
            q @ (b'"' | b'\'') => i = b[i + 1..].iter().position(|&c| c == q).map_or(b.len(), |j| i + 2 + j),
            _ => i += 1,
        }
    }
    b.len()
}

/// The pixels a `w`×`h` raster holds, rounded as resvg rounds them; never wraps.
fn area(w: f32, h: f32) -> u64 {
    (f64::from(w.max(0.0)).round() * f64::from(h.max(0.0)).round()) as u64
}

/// Fails when the tree usvg built (clip paths, masks, patterns, filter images and nested SVGs
/// included) has more than [`MAX_NODES`] nodes, or its images and patterns cost more than
/// [`MAX_IMAGE_PIXELS`] drawn at `k` device pixels per unit on a `canvas` of that many pixels:
/// a raster its own, a nested SVG the whole canvas (resvg draws it on one of its own), and each
/// pattern a fill the size of its tile under its transform.
fn weigh(tree: &Tree, k: f32, canvas: u64) -> Result<(), ImageError> {
    let mut groups = vec![(tree.root(), Transform::from_scale(k, k))];
    let (mut nodes, mut pixels) = (0usize, 0u64);
    while let Some((g, ts)) = groups.pop() {
        if nodes > MAX_NODES || pixels > MAX_IMAGE_PIXELS { break; }
        let ts = ts.pre_concat(g.transform());
        nodes += g.children().len();
        let mut clip = g.clip_path();
        while let Some(c) = clip { groups.push((c.root(), ts.pre_concat(c.transform()))); clip = c.clip_path(); }
        let mut mask = g.mask();
        while let Some(m) = mask { groups.push((m.root(), ts)); mask = m.mask(); }
        for p in g.filters().iter().flat_map(|f| f.primitives()) {
            if let filter::Kind::Image(i) = p.kind() { groups.push((i.root(), ts)); }
        }
        for n in g.children() {
            match n {
                Node::Group(g) => groups.push((g, ts)),
                Node::Path(p) => {
                    for paint in p.fill().map(|f| f.paint()).into_iter().chain(p.stroke().map(|s| s.paint())) {
                        let Paint::Pattern(pattern) = paint else { continue };
                        // resvg draws the tile at its scale on the canvas, then fills with it.
                        let (sx, sy) = ts.pre_concat(pattern.transform()).get_scale();
                        pixels = pixels.saturating_add(area(pattern.rect().width() * sx, pattern.rect().height() * sy));
                        groups.push((pattern.root(), Transform::from_scale(sx, sy)));
                    }
                }
                Node::Image(i) => match i.kind() {
                    ImageKind::SVG(t) => {
                        groups.push((t.root(), ts));
                        pixels = pixels.saturating_add(canvas);
                    }
                    _ => pixels = pixels.saturating_add(area(i.size().width(), i.size().height())),
                },
                Node::Text(t) => groups.push((t.flattened(), ts)),
            }
        }
    }
    if nodes > MAX_NODES { return Err(ImageError(format!("SVG has more than {MAX_NODES} nodes"))); }
    if pixels > MAX_IMAGE_PIXELS { return Err(ImageError("SVG's images and patterns are too large to draw".into())); }
    Ok(())
}

/// Where an `<image>` (or `<feImage>`) may draw from: `data:` images, and with `files` a
/// regular file by path, as usvg resolves one (absolute, or relative to the working directory).
fn resolver(files: bool, budget: Arc<Budget>) -> ImageHrefResolver<'static> {
    let in_data = budget.clone();
    ImageHrefResolver {
        resolve_data: Box::new(move |mime, data, _| match mime {
            "image/png" | "image/jpg" | "image/jpeg" | "image/gif" | "image/webp" => raster(data),
            "image/svg+xml" => nested(&data, &in_data),
            "text/plain" => raster(data.clone()).or_else(|| nested(&data, &in_data)),
            _ => None,
        }),
        resolve_string: Box::new(move |href, opt| {
            if !files { return None; }
            let path = opt.get_abs_path(std::path::Path::new(href));
            // A FIFO would block the read, and a device such as /dev/zero never ends it.
            std::fs::metadata(&path).ok().filter(|m| m.is_file() && m.len() <= MAX_BYTES as u64)?;
            let mut data = Vec::new();
            std::fs::File::open(&path).ok()?.take(MAX_BYTES as u64).read_to_end(&mut data).ok()?;
            let svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg") || e.eq_ignore_ascii_case("svgz"));
            if svg { nested(&data, &budget) } else { raster(Arc::new(data)) }
        }),
    }
}

/// PNG, JPEG, GIF or WebP, by their first bytes.
fn raster(data: Arc<Vec<u8>>) -> Option<ImageKind> {
    let d = &data[..];
    Some(if d.starts_with(b"\x89PNG\r\n\x1a\n") { ImageKind::PNG(data) }
    else if d.starts_with(&[0xff, 0xd8, 0xff]) { ImageKind::JPEG(data) }
    else if d.starts_with(b"GIF8") { ImageKind::GIF(data) }
    else if d.len() >= 12 && &d[..4] == b"RIFF" && &d[8..12] == b"WEBP" { ImageKind::WEBP(data) }
    else { return None })
}

/// An SVG image inside an SVG, within its parent's budget and with no images of its own, as
/// usvg's own resolver loads one. Past a budget fails the parent; a malformed one is skipped.
fn nested(data: &[u8], budget: &Budget) -> Option<ImageKind> {
    let none = ImageHrefResolver { resolve_data: Box::new(|_, _, _| None), resolve_string: Box::new(|_, _| None) };
    match parse(data, &Options { image_href_resolver: none, ..Options::default() }, budget) {
        Ok(tree) => Some(ImageKind::SVG(tree)),
        Err((over, e)) => {
            if over { budget.refused.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(e); }
            None
        }
    }
}

#[cfg(test)]
#[path = "svg_tests.rs"]
mod tests;
