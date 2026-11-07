#![cfg(feature = "markdown")]
//! The tail of a [`Markdown`] still streaming draws as what it's about to become: an open
//! emphasis never shows its markers, and a `| …` line never lays out as a paragraph before it's
//! a table.

use std::cell::RefCell;

use rune::motion_gate::{Run, check};
use rune::{Cx, El, Env, Runtime, col};
use rune_kit::{Kit, Markdown};

fn streaming(src: &String, _: &mut Cx<String>) -> El {
    col().w_full().child(Markdown::new(src.clone()).streaming(true).build(&Kit::neutral(true)))
}

fn texts(rt: &Runtime<String>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).filter(|t| !t.is_empty()).collect()
}

/// Streams `deltas` into a fresh runtime, calling `each` after every one lands.
fn stream(deltas: &[&str], mut each: impl FnMut(&str, &Runtime<String>)) {
    let mut rt = Runtime::new(String::new(), streaming, (480.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    for d in deltas {
        rt.update(|s, _| s.push_str(d));
        rt.frame(1.0 / 60.0);
        let src = rt.state.clone();
        each(&src, &rt);
    }
}

#[test]
fn an_open_bold_never_shows_its_stars() {
    let deltas = ["…and *", "*", "Sett", "ings › Appear", "ance", "*", "*", "…"];
    let mut last = String::new();
    stream(&deltas, |src, rt| {
        let shown = texts(rt).join("\n");
        assert!(!shown.contains('*'), "{src:?} shows {shown:?}");
        assert!(shown.starts_with(&last), "{src:?}: {last:?} became {shown:?}");
        last = shown;
    });
    assert_eq!(last, "…and Settings › Appearance…");
}

#[test]
fn a_header_line_draws_nothing_until_it_is_a_table() {
    let deltas = ["| File", " |", " Change |\n", "|---|---|\n", "| a.rs | x |"];
    let mut width = None;
    stream(&deltas, |src, rt| {
        for e in rt.elements() {
            if e.text.as_deref() == Some("File") {
                assert!(e.path.contains("th"), "{src:?} draws `File` outside a table: {}", e.path);
            }
        }
        let table = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("b0")).map(|e| e.rect[2]);
        if let (Some(was), Some(now)) = (width, table) {
            assert_eq!(was, now, "{src:?} resized the table");
        }
        width = width.or(table);
    });
    assert!(width.is_some_and(|w| w < 300.0), "the table sizes to its cells, not the column: {width:?}");
}

/// Typed a character a frame, neither document swaps a word or jumps.
#[test]
fn a_stream_types_without_swaps_or_jumps() {
    for doc in ["Then open **Settings › Appearance** and pick *Dark*, or run `theme dark`.", "| File | Change |\n|---|---|\n| a.rs | x |\n"] {
        let mut r = Run::new(Runtime::new(String::new(), streaming, (480.0, 800.0), Env::new(true, 1.0)), 60.0);
        r.step();
        for ch in doc.chars() {
            r.act("type", |rt| rt.update(|s, _| s.push(ch)));
        }
        r.play(1.0);
        let found: Vec<String> = check(&r.rec).into_iter().filter(|f| f.kind == "text-swap" || f.kind.contains("jump")).map(|f| f.to_string()).collect();
        assert!(found.is_empty(), "{doc:?}:\n{}", found.join("\n"));
    }
}

thread_local! {
    /// Every source an image wire has been asked for, in order.
    static ASKED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn with_images(src: &String, streaming: bool) -> El {
    let md = Markdown::new(src.clone()).streaming(streaming).images(|s| {
        ASKED.with(|a| a.borrow_mut().push(s.to_owned()));
        None
    });
    col().w_full().child(md.build(&Kit::neutral(true)))
}

/// An image still streaming in shows its alt text, and an image with no source shows its alt
/// text, without the app being asked for a source: it is only ever asked for a whole one.
#[test]
fn an_image_wire_is_only_asked_for_a_whole_source() {
    let doc = "see ![a chart](https://ex.com/a.png \"Title\") and more";
    ASKED.with(|a| a.borrow_mut().clear());
    let mut rt = Runtime::new(String::new(), |s, _| with_images(s, true), (480.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    for ch in doc.chars() {
        rt.update(|s, _| s.push(ch));
        rt.frame(1.0 / 60.0);
        let src = rt.state.clone();
        let asked = ASKED.with(|a| a.borrow().clone());
        assert!(asked.iter().all(|s| s == "https://ex.com/a.png"), "{src:?} asked for {asked:?}");
        // Until its `)` arrives, nothing is asked, and its alt text so far shows (a space it
        // ends with waits for the next word).
        if !src.contains(')') {
            assert!(asked.is_empty(), "{src:?} asked for {asked:?}");
            if let Some(alt) = src.strip_prefix("see ![").map(|a| a.split(']').next().unwrap_or(a).trim_end()).filter(|a| !a.is_empty()) {
                assert!(texts(&rt).iter().any(|t| t == alt), "{src:?} doesn't show its alt {alt:?}: {:?}", texts(&rt));
            }
        }
    }
    let asked = ASKED.with(|a| a.borrow().clone());
    assert!(!asked.is_empty(), "the whole image is asked for its picture");
    assert_eq!(texts(&rt), ["see", "a chart", "and more"]);

    for doc in ["![alt]()", "see ![alt]() and more"] {
        ASKED.with(|a| a.borrow_mut().clear());
        let mut rt = Runtime::new(doc.to_owned(), |s, _| with_images(s, false), (480.0, 800.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let asked = ASKED.with(|a| a.borrow().clone());
        assert!(asked.is_empty(), "{doc:?} asked for {asked:?}");
        assert!(texts(&rt).iter().any(|t| t == "alt"), "{doc:?} doesn't show its alt: {:?}", texts(&rt));
    }
}

/// An image inside an image's alt text is part of that text, so its source is never asked for,
/// while it streams in or once it is whole: only the outer image's is, once its `)` arrives.
#[test]
fn an_image_in_an_images_alt_text_is_never_asked_for() {
    let doc = "see ![outer ![inner](in.png) tail](outer.png) and more";
    ASKED.with(|a| a.borrow_mut().clear());
    let mut rt = Runtime::new(String::new(), |s, _| with_images(s, true), (480.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    for ch in doc.chars() {
        rt.update(|s, _| s.push(ch));
        rt.frame(1.0 / 60.0);
        let src = rt.state.clone();
        let asked = ASKED.with(|a| a.borrow().clone());
        assert!(asked.iter().all(|s| s == "outer.png"), "{src:?} asked for {asked:?}");
        if !src.contains("](outer.png)") {
            assert!(asked.is_empty(), "{src:?} asked for {asked:?}");
        }
    }
    assert!(!ASKED.with(|a| a.borrow().is_empty()), "the whole image is asked for its picture");
    assert_eq!(texts(&rt), ["see", "outer inner tail", "and more"]);

    ASKED.with(|a| a.borrow_mut().clear());
    let mut rt = Runtime::new(doc.to_owned(), |s, _| with_images(s, false), (480.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let asked = ASKED.with(|a| a.borrow().clone());
    assert!(!asked.is_empty() && asked.iter().all(|s| s == "outer.png"), "{doc:?} asked for {asked:?}");
    assert_eq!(texts(&rt), ["see", "outer inner tail", "and more"]);
}
