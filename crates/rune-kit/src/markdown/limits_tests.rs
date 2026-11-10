use super::*;
use std::time::{Duration, Instant};

/// How long a debug build may take over each adversarial source: its parse, and a frame of it
/// whole and one still streaming in. A release build is far faster.
const BOUND: Duration = Duration::from_secs(2);

/// Quotes and lists open around the innermost block.
fn depth(blocks: &[Block]) -> usize {
    blocks.iter().map(|b| match b {
        Block::Quote(inner) => 1 + depth(inner),
        Block::List { items, .. } => 1 + items.iter().map(|i| depth(i)).max().unwrap_or(0),
        _ => 0,
    }).max().unwrap_or(0)
}

/// Every paragraph's and heading's text, in document order.
fn texts(blocks: &[Block], out: &mut Vec<String>) {
    for b in blocks {
        match b {
            Block::Para(i) | Block::Heading(_, i) => out.push(i.text.clone()),
            Block::Quote(inner) => texts(inner, out),
            Block::List { items, .. } => items.iter().for_each(|i| texts(i, out)),
            _ => {}
        }
    }
}

fn view(src: &String, _: &mut rune::Cx<String>) -> El {
    col().w_full().child(Markdown::new(src.clone()).selectable(true).quote_frame(|e, _| e).build(&Kit::neutral(true)))
}

fn streamed(src: &String, _: &mut rune::Cx<String>) -> El {
    col().w_full().child(Markdown::new(src.clone()).streaming(true).selectable(true).build(&Kit::neutral(true)))
}

/// The parse of `src`, after drawing a frame of it whole and one of it still streaming in, all
/// within [`BOUND`].
fn drawn(src: &str) -> Rc<Vec<Block>> {
    let start = Instant::now();
    let blocks = parse_cached(src);
    for v in [view as fn(&String, &mut rune::Cx<String>) -> El, streamed] {
        let mut rt = rune::Runtime::new(src.to_owned(), v, (480.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
    }
    assert!(start.elapsed() < BOUND, "{} bytes took {:?}", src.len(), start.elapsed());
    blocks
}

#[test]
fn a_hundred_thousand_nested_quotes_draw_thirty_two_deep_with_their_text() {
    let blocks = drawn(&format!("{} deep", ">".repeat(100_000)));
    assert_eq!(depth(&blocks), MAX_DEPTH);
    let mut t = Vec::new();
    texts(&blocks, &mut t);
    assert_eq!(t, ["deep"]);
    // Quotes past the cap with blocks of their own keep each, in order.
    let src = (1..=40).map(|n| format!("{} q{n}\n", ">".repeat(n))).collect::<String>();
    let blocks = parse(&src);
    assert_eq!(depth(&blocks), MAX_DEPTH);
    let mut t = Vec::new();
    texts(&blocks, &mut t);
    assert_eq!(t.join(" "), (1..=40).map(|n| format!("q{n}")).collect::<Vec<_>>().join(" "));
}

#[test]
fn a_hundred_thousand_nested_list_markers_draw_thirty_two_deep_with_their_text() {
    for src in [format!("{}x", "- ".repeat(100_000)), format!("{}x", "1. ".repeat(100_000)), format!("{}x", "> - ".repeat(50_000))] {
        let blocks = drawn(&src);
        assert_eq!(depth(&blocks), MAX_DEPTH, "{}", &src[..8]);
        let mut t = Vec::new();
        texts(&blocks, &mut t);
        assert_eq!(t, ["x"], "{}", &src[..8]);
    }
    // Items past the cap stay paragraphs of their own, a task keeps its box as text, and each
    // item the renderer meets has its task entry.
    let mut src = String::new();
    for n in 0..36 {
        src += &format!("{}- [x] t{n}\n", "  ".repeat(n));
    }
    let (blocks, _, tasks, _, _) = doc_cached(&src);
    let mut t = Vec::new();
    texts(&blocks, &mut t);
    assert_eq!(t[..MAX_DEPTH], (0..MAX_DEPTH).map(|n| format!("t{n}")).collect::<Vec<_>>()[..]);
    assert_eq!(t[MAX_DEPTH..], ["[x] t32", "[x] t33", "[x] t34", "[x] t35"]);
    assert_eq!(tasks.len(), MAX_DEPTH);
    assert!(tasks.iter().all(|t| matches!(t, Some((true, _)))));
}

/// Nine-digit numbers widen each level's copied indent by 11 columns: 32 levels pass a `u8`.
#[test]
fn deep_lists_with_wide_numbers_copy_without_overflow() {
    let src = format!("> {}x", "999999999. ".repeat(40));
    let blocks = drawn(&src);
    assert_eq!(depth(&blocks), MAX_DEPTH);
}

#[test]
fn a_ten_thousand_column_table_keeps_its_first_128() {
    let row = |c: &str| (0..10_000).map(|i| format!("|{c}{i}")).collect::<String>() + "|\n";
    let src = format!("{}{}{}", row("h"), "|-".repeat(10_000) + "|\n", row("c"));
    let blocks = drawn(&src);
    let [Block::Table(t)] = &blocks[..] else { panic!("{:?}", &blocks[..1]) };
    assert_eq!((t.align.len(), t.head.len(), t.rows[0].len()), (MAX_COLUMNS, MAX_COLUMNS, MAX_COLUMNS));
    assert_eq!((t.head[MAX_COLUMNS - 1].text.as_str(), t.rows[0][MAX_COLUMNS - 1].text.as_str()), ("h127", "c127"));
    // A header line alone, drawn as the table it is about to become, keeps the cap too.
    let blocks = drawn(row("h").trim_end());
    let [Block::Table(t)] = &blocks[..] else { panic!("{:?}", &blocks[..1]) };
    assert_eq!((t.align.len(), t.head.len()), (MAX_COLUMNS, MAX_COLUMNS));
}

#[test]
fn a_megabyte_of_stars_draws() {
    let blocks = drawn(&"*".repeat(1 << 20));
    assert_eq!(*blocks, [Block::Rule]);
}

#[test]
fn past_the_block_cap_the_rest_is_one_paragraph_as_written() {
    let src = "a\n\n".repeat(MAX_BLOCKS + 10) + "- b\n- *c*\n";
    let blocks = parse_cached(&src);
    assert_eq!(blocks.len(), MAX_BLOCKS + 1);
    let Some(Block::Para(rest)) = blocks.last() else { panic!() };
    assert_eq!(rest.text, "a\n\n".repeat(10) + "- b\n- *c*");
    assert_eq!(spent(&blocks), MAX_BLOCKS + 1);
    // Items count too (an item and its text are two): the cap falls inside the list, which
    // keeps the items before it.
    let src = "- i\n".repeat(MAX_BLOCKS);
    let blocks = parse_cached(&src);
    let [Block::List { items, .. }, Block::Para(rest)] = &blocks[..] else { panic!() };
    assert_eq!(items.len(), MAX_BLOCKS / 2);
    assert_eq!(rest.text.matches("- i").count(), MAX_BLOCKS - items.len());
}

/// An image standing alone in a paragraph lifts into a block of its own, so one paragraph of
/// images builds a block per image: past the cap the rest stay in the line as alt text.
#[test]
fn images_in_one_paragraph_count_against_the_block_cap() {
    let src = "![x](a) ".repeat(MAX_BLOCKS + 1000);
    let blocks = parse(&src);
    assert_eq!(blocks.len(), MAX_BLOCKS + 1);
    assert!(blocks[..MAX_BLOCKS].iter().all(|b| matches!(b, Block::Image { .. })));
    let Some(Block::Para(rest)) = blocks.last() else { panic!() };
    assert_eq!(rest.text.split(' ').filter(|w| *w == "x").count(), 1000);
}

/// A stream grown from its last block matches a whole parse on both sides of the block cap.
#[test]
fn a_stream_past_the_block_cap_parses_like_a_whole_one() {
    let mut src = "a\n\n".repeat(MAX_BLOCKS - 3);
    for more in ["b\n\n", "c\n\n", "- d\n", "- e\n", "f\n\n", "g"] {
        src += more;
        let (blocks, sources, tasks, starts, maps) = doc_cached(&src);
        let (want, want_sources, want_tasks, want_cuts, want_starts, want_maps) = parse_doc(&src, true);
        assert_eq!(*blocks, want, "after {more:?}");
        assert_eq!((&*sources, &*tasks, &*starts, &*maps), (&want_sources[..], &want_tasks[..], &want_starts[..], &want_maps[..]), "after {more:?}");
        let cuts = CACHE.with_borrow(|c| c.docs.peek(&*src).unwrap().cuts.clone());
        assert_eq!(&*cuts, &want_cuts[..], "after {more:?}");
    }
}

