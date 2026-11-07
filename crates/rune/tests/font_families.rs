use std::hash::{Hash, Hasher};
use rune::{Family, OpticalSize, TextStyle, text::{Span, TextSystem}};

const INTER: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");
const PLEX: &[u8] = include_bytes!("../../../themes/opencode/assets/fonts/IBMPlexMono-Regular.ttf");
const DISPLAY: TextStyle = TextStyle::new(13.0, 400.0).family(Family::Named("IBM Plex Mono"));

// Exactly TextStyle's public fields. Deliberately exhaustive: alignment is an element builder,
// not a style field. Consumers build styles with `TextStyle::new` and `..` (checked for `italic`).
const _: TextStyle = TextStyle {
    size: 13.0,
    weight: 400.0,
    family: Family::Sans,
    line_height: 0.0,
    tracking: 0.0,
    variations: "",
    features: "",
    optical_size: OpticalSize::Auto,
    italic: false,
};

#[test]
fn registered_family_is_independent_of_slots_and_cache_entries() {
    // Plex is registered but occupies neither slot, just like a display wordmark family.
    let mut system = TextSystem::new(&[INTER, PLEX], ("Inter", "Inter"));
    let default = TextStyle::new(13.0, 400.0);
    let sans = system.layout_rich("Hello 0123", &[], default, 2.0, None);
    let named = system.layout_rich("Hello 0123", &[], DISPLAY, 2.0, None);
    assert!(!std::rc::Rc::ptr_eq(&sans, &named));
    assert_ne!(sans.full_width(), named.full_width());
    for run in named.lines().flat_map(|l| l.runs()) {
        assert_eq!(run.font().data.as_ref(), PLEX);
    }
    let again = system.layout_rich("Hello 0123", &[], DISPLAY, 2.0, None);
    assert!(std::rc::Rc::ptr_eq(&named, &again));
    let other = system.layout_rich("Hello 0123", &[], default.family(Family::Named("Inter")), 2.0, None);
    assert!(!std::rc::Rc::ptr_eq(&named, &other), "the family name must participate in the cache key");
    for style in [DISPLAY.mono(), DISPLAY.family(Family::Sans)] {
        let layout = system.layout_rich("Hello 0123", &[], style, 2.0, None);
        for run in layout.lines().flat_map(|l| l.runs()) {
            assert_eq!(run.font().data.as_ref(), INTER);
        }
    }
    let spans = [Span { mono: true, ..Span::new(6, 10) }];
    let rich = system.layout_rich("Hello 0123", &spans, DISPLAY, 2.0, None);
    assert!(rich.lines().flat_map(|l| l.runs()).any(|r| r.font().data.as_ref() == PLEX));
    assert!(rich.lines().flat_map(|l| l.runs()).any(|r| r.font().data.as_ref() == INTER),
        "mono spans still select the mono slot");
}

/// The glyph rects and atlas pixels "Hello 0123 ffi" draws in `family` at `scale`, from a fresh
/// system whose slots are distinct (Inter sans, Plex mono), so a sans fallback and a mono one
/// draw differently.
fn drawn(family: Family, scale: f32) -> (Vec<([f32; 4], [f32; 2])>, Vec<u8>) {
    let mut system = TextSystem::new(&[INTER, PLEX], ("Inter", "IBM Plex Mono"));
    let mut quads = Vec::new();
    system.glyphs("Hello 0123 ffi", TextStyle::new(13.0, 400.0).family(family), scale, [0.0, 0.0], &mut quads);
    (quads.iter().map(|q| (q.rect, q.uv)).collect(), std::mem::take(&mut system.atlas.data))
}

#[test]
fn missing_named_family_falls_back_to_sans_pixels() {
    // Distinct slots prove missing families choose sans, not mono. A name with a comma is a CSS
    // list tried in order, so a list of missing names draws sans too, and a list that goes on
    // to a registered family draws that family.
    for scale in [1.0, 2.0] {
        let sans = drawn(Family::Sans, scale);
        for missing in ["Rune Missing Family", "Rune Missing Family, Rune Other Missing Family"] {
            let named = drawn(Family::Named(missing), scale);
            assert_eq!(named.0, sans.0, "{missing:?} must draw sans glyphs at {scale}×");
            assert!(named.1 == sans.1, "{missing:?} must draw sans atlas pixels at {scale}×");
        }
        let plex = drawn(Family::Named("IBM Plex Mono"), scale);
        assert_ne!(plex.0, sans.0);
        let listed = drawn(Family::Named("Rune Missing Family, IBM Plex Mono"), scale);
        assert_eq!(listed.0, plex.0, "a list skips its missing family for the next one at {scale}×");
        assert!(listed.1 == plex.1, "a list skips its missing family for the next one at {scale}×");
    }
}

#[test]
fn default_faces_keep_batch_c_pixels() {
    // Capture the existing sans/mono paths, including mono rich spans, at both densities.
    for scale in [1.0, 2.0] {
        let mut system = TextSystem::new(&[INTER, PLEX], ("Inter", "IBM Plex Mono"));
        system.set_font_smoothing(rune::FontSmoothing::On);
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for style in [TextStyle::new(13.0, 400.0), TextStyle::new(13.0, 400.0).mono()] {
            let spans = [Span { mono: true, ..Span::new(6, 10) }];
            let layout = system.layout_rich("Hello 0123 ffi", &spans, style, scale, None);
            layout.full_width().to_bits().hash(&mut hash);
            let mut quads = Vec::new();
            system.glyphs_for_layout(&layout, scale, [0.0, 0.0], false, &mut quads);
            for q in quads {
                q.rect.map(f32::to_bits).hash(&mut hash);
                q.uv.map(f32::to_bits).hash(&mut hash);
            }
        }
        system.atlas.data.hash(&mut hash);
        // Re-baselined for `FontSmoothing::On` (CoreGraphics pipeline) once Calibrated was removed.
        let expected = if scale == 1.0 { 17045220814369067823 } else { 10933675232581112092 };
        assert_eq!(hash.finish(), expected, "sans/mono pixels changed at {scale}×");
    }
}
