use rune::{Color, Cx, El, Env, Runtime, TextEdit, col, el, text};
use rune::render::{Gpu, Instance, KIND_LAYER, KIND_LAYER_END, OFFSCREEN_FORMAT, Offscreen, Renderer};
use rune_kit::*;

#[derive(Default)]
struct Demo { case: usize, enabled: bool, combo: ComboboxState }

/// What fades in each case of `view`: the path each disabled control's faded element ends at,
/// and Desktop's `opacity` for it (0.4 Contrast, 0.45 segments, else 0.5).
const FADES: [&[(&str, f32)]; 15] = [
    &[("case/#0/stack/control", 0.5)],
    &[("case/#0/stack/control", 0.5)],
    &[("case/select-trigger", 0.5)],
    &[("case/#0/segment0", 0.45), ("case/#0/segment1", 0.45)],
    &[("case/#0", 0.5)],
    &[("case/#0", 0.5)],
    &[("case/#0", 0.5)],
    &[("case/#0", 0.5)],
    &[("case/#0", 0.4)],
    &[("case/#0", 0.5)],
    &[("case/#0", 0.4)],
    &[("case/#0/control", 0.5)],
    &[("case/#0/control", 0.5)],
    &[("rows/menu-item0", 0.5)],
    &[("rows/combo-option-disabled", 0.5)],
];

fn view(s: &Demo, cx: &mut Cx<Demo>) -> El {
    let k = Kit::neutral(cx.system_dark());
    let off = !s.enabled;
    let mut layers = Vec::new();
    let edit = TextEdit::new("Disabled value");
    let button = |variant| Button::new("Button").icon(k.glyph.copy).variant(variant).disabled(off).build(&k);
    let segment = if off { State::Disabled } else { State::Rest };
    let body = match s.case {
        0 => RadioItem::new("Radio", false).disabled(off).build(&k),
        1 => RadioItem::new("Radio", true).disabled(off).build(&k),
        2 => Select::new("select", &["One", "Two"]).current(Some("One")).disabled(off).build(&k, &mut layers),
        3 => SegmentedControl::new(&["One", "Two"], Some(0)).width(None)
            .segment_state(0, segment).segment_state(1, segment).build(&k),
        4 => TextInput::new(&edit).leading_icon(k.glyph.copy).copy_button().disabled(off).build(&k),
        5 => TextArea::new(&edit).disabled(off).build(&k),
        6 => TextField::new("Field", &edit).error("Error").copyable().disabled(off).build(&k),
        7 => button(ButtonVariant::Neutral),
        8 => button(ButtonVariant::Contrast),
        9 => button(ButtonVariant::Outline),
        10 => IconButton::new(k.glyph.copy).variant(IconButtonVariant::Contrast).disabled(off).build(&k),
        11 => Switch::new(false).label("Switch").disabled(off).build(&k),
        12 => Switch::new(true).label("Switch").disabled(off).build(&k),
        13 => Menu::new("menu").trigger(text("Menu")).open(true).min_width(220.0)
            .item(MenuItem::new("Item").disabled(off).icon(k.glyph.copy).shortcut("A").badge("New").checked(true)
                .lead(el().size(16.0, 16.0).fill(Color::WHITE).child(el().size(8.0, 8.0).fill(Color::BLACK))))
            .build(&k, &mut layers),
        14 => {
            // The list's active row is its first enabled option, so "Other" leads and the
            // option under test is drawn alike either way but for the fade.
            let mut option = ComboboxOption::new("disabled", "Disabled option");
            option.disabled = off;
            Combobox::new("combo", &s.combo, &[ComboboxOption::new("other", "Other"), option], |s: &mut Demo| &mut s.combo)
                .current(Some("disabled")).build(&k, &mut layers)
        }
        _ => unreachable!(),
    };
    // On the page at its own size, so a menu or list opens below its trigger, in the window.
    root(col().key("case").fill(k.c.bg_base).padding(20.0).items_start().child(body), layers)
}

const SIZE: (u32, u32) = (320, 240);
const SCALE: f32 = 2.0;

fn settled(case: usize, enabled: bool, dark: bool) -> Runtime<Demo> {
    let mut s = Demo { case, enabled, ..Default::default() };
    s.combo.open = true;
    let mut rt = Runtime::new(s, view, (SIZE.0 as f32, SIZE.1 as f32), Env { system_dark: dark, scale: SCALE });
    rt.set_srgb_blend(true);
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(0.0);
    rt
}

/// The layers below full opacity in `instances`, as (device-pixel bounds, opacity).
fn faded_layers(instances: &[Instance]) -> Vec<([f32; 4], f32)> {
    instances.iter().filter(|i| i.params[1] == KIND_LAYER && i.fill[3] < 1.0).map(|i| (i.rect, i.fill[3])).collect()
}

/// `instances` without its faded layers and all they hold: what lies behind them.
fn behind_faded_layers(instances: &[Instance]) -> Vec<Instance> {
    // Whether each open layer fades.
    let mut open: Vec<bool> = Vec::new();
    instances.iter().filter(|i| {
        let was_inside = open.contains(&true);
        match i.params[1] {
            k if k == KIND_LAYER => open.push(i.fill[3] < 1.0),
            k if k == KIND_LAYER_END => { open.pop(); }
            _ => {}
        }
        !was_inside && !open.contains(&true)
    }).copied().collect()
}

/// `instances` drawn by `rt`'s atlas over its clear colour, as sRGB RGBA8 device pixels.
fn draw(gpu: &Gpu, renderer: &mut Renderer, rt: &mut Runtime<Demo>, instances: &[Instance]) -> Vec<u8> {
    let (w, h) = ((SIZE.0 as f32 * SCALE) as u32, (SIZE.1 as f32 * SCALE) as u32);
    let target = Offscreen::with_format(&gpu.device, w, h, OFFSCREEN_FORMAT.remove_srgb_suffix());
    renderer.prepare(&gpu.device, &gpu.queue, &mut rt.text.atlas, instances, (w, h));
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    renderer.draw(&mut encoder, &target.view, rt.clear_color());
    target.copy(&mut encoder);
    renderer.submit(&gpu.queue, encoder);
    target.read(&gpu.device)
}

/// A disabled control fades as one layer at Desktop's opacity, as CSS `opacity` does: each of
/// its pixels is the enabled control's pixel at that opacity over what lies behind it, so its
/// label shows its fill as it is rather than faded through it, and an enabled control doesn't
/// fade at all.
#[test]
fn disabled_controls_fade_as_one_layer() {
    let gpu = Gpu::headless();
    let w = (SIZE.0 as f32 * SCALE) as usize;
    for dark in [false, true] {
        let mut renderer = Renderer::new(&gpu.device, OFFSCREEN_FORMAT.remove_srgb_suffix());
        renderer.set_srgb_blend(true);
        for (case, fades) in FADES.iter().enumerate() {
            let mut off = settled(case, false, dark);
            let mut on = settled(case, true, dark);
            assert_eq!(faded_layers(&on.instances), [], "case {case} dark={dark}: an enabled control fades");
            let layers = faded_layers(&off.instances);
            assert_eq!(layers.len(), fades.len(), "case {case} dark={dark}: one layer for each disabled control: {layers:?}");
            let (all, behind, enabled) = (off.instances.clone(), behind_faded_layers(&off.instances), on.instances.clone());
            let disabled_px = draw(&gpu, &mut renderer, &mut off, &all);
            let behind_px = draw(&gpu, &mut renderer, &mut off, &behind);
            let enabled_px = draw(&gpu, &mut renderer, &mut on, &enabled);
            let elements = off.elements();
            for &(path, alpha) in *fades {
                let found: Vec<_> = elements.iter().filter(|e| e.path.ends_with(path)).collect();
                assert_eq!(found.len(), 1, "case {case}: one element at {path}");
                let r = found[0].rect.map(|v| v * SCALE);
                if case == 13 { assert_eq!(found[0].role, "menuitemcheckbox"); }
                if case == 14 { assert_eq!(found[0].role, "option"); }
                assert!(layers.iter().any(|&(b, a)| (a - alpha).abs() < 1e-4 && (0..4).all(|j| (b[j] - r[j]).abs() <= 1.0)),
                    "case {case} dark={dark}: {path} at {r:?} fades as one layer at {alpha}: {layers:?}");
                let (mut worst, mut dimmed) = ((0.0f32, [0usize; 2]), 0.0f32);
                for y in r[1].ceil() as usize..(r[1] + r[3]).floor() as usize {
                    for x in r[0].ceil() as usize..(r[0] + r[2]).floor() as usize {
                        for ch in 0..3 {
                            let at = (y * w + x) * 4 + ch;
                            let (got, lit, under) = (disabled_px[at] as f32, enabled_px[at] as f32, behind_px[at] as f32);
                            let miss = (got - (alpha * lit + (1.0 - alpha) * under)).abs();
                            if miss > worst.0 { worst = (miss, [x, y]); }
                            dimmed = dimmed.max((got - lit).abs());
                        }
                    }
                }
                assert!(worst.0 <= 2.0, "case {case} dark={dark}: {path} at pixel {:?} is {:.1} off the enabled control at {alpha} over what lies behind it", worst.1, worst.0);
                assert!(dimmed >= 8.0, "case {case} dark={dark}: {path} draws as when enabled");
            }
        }
    }
}

/// A disabled contrast button on the page, as the browser draws `opacity: 0.4` over it: 40% of
/// the fill under its sheen, and none of its shadows through it.
#[test]
fn a_disabled_contrast_button_fades_as_the_browser_does() {
    use rune::color::srgb_encode;
    use rune::render::{Gpu, OFFSCREEN_FORMAT, Offscreen, Renderer};
    fn view(_: &(), cx: &mut Cx<()>) -> El {
        let k = rune_theme_opencode::kit(cx.system_dark());
        let b = Button::new("Unlock").variant(ButtonVariant::Contrast).control_size(ControlSize::Large).disabled(true).build(k);
        col().w_full().h_full().fill(k.c.bg_base).padding(10.0).child(b.w(200.0))
    }
    let (w, h) = (220u32, 48u32);
    let gpu = Gpu::headless();
    for dark in [false, true] {
        let mut rt = Runtime::new((), view, (w as f32, h as f32), Env { system_dark: dark, scale: 1.0 });
        rt.set_srgb_blend(true);
        rt.frame(0.0);
        rt.settle_all();
        rt.frame(0.0);
        let format = OFFSCREEN_FORMAT.remove_srgb_suffix();
        let mut renderer = Renderer::new(&gpu.device, format);
        renderer.set_srgb_blend(true);
        let target = Offscreen::with_format(&gpu.device, w, h, format);
        renderer.prepare(&gpu.device, &gpu.queue, &mut rt.text.atlas, &rt.instances, (w, h));
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        renderer.draw(&mut encoder, &target.view, rt.clear_color());
        target.copy(&mut encoder);
        renderer.submit(&gpu.queue, encoder);
        let px = target.read(&gpu.device);
        let k = rune::theme::with_blend(true, || rune_theme_opencode::kit(dark));
        let byte = |c: f32| (srgb_encode(c) * 255.0).round();
        let (fill, page) = (byte(k.c.bg_contrast.r), byte(k.c.bg_base.r));
        let mix = |fg: f32, bg: f32, a: f32| a * fg + (1.0 - a) * bg;
        // Inside the left padding, clear of the label, from the top of the button down.
        for y in [16u32, 24, 32] {
            let t = (y as f32 + 0.5 - 10.0) / 28.0;
            let want = mix(mix(255.0, fill, 0.2 * (1.0 - t)), page, 0.4);
            let got = px[((y * w + 15) * 4) as usize] as f32;
            assert!((got - want).abs() <= 1.5, "dark {dark}, row {y}: {got} vs the browser's {want:.1}");
        }
    }
}
