//! The dump's action table against the actions the runtime adds: every action a node supports
//! has a name in the dump, and every name in the table shows up on a node of one scene.

use accesskit::Action as Ak;
use serde_json::Value;

use super::ACTIONS;
use crate::{Cx, El, Env, Link, Runtime, TextEdit, col, rich, row, text, text_input};

/// Every AccessKit action, in AccessKit's order.
const EVERY: [Ak; 22] = [
    Ak::Click, Ak::Focus, Ak::Blur, Ak::Collapse, Ak::Expand, Ak::CustomAction, Ak::Decrement, Ak::Increment,
    Ak::HideTooltip, Ak::ShowTooltip, Ak::ReplaceSelectedText, Ak::ScrollDown, Ak::ScrollLeft, Ak::ScrollRight,
    Ak::ScrollUp, Ak::ScrollIntoView, Ak::ScrollToPoint, Ak::SetScrollOffset, Ak::SetTextSelection,
    Ak::SetSequentialFocusNavigationStartingPoint, Ak::SetValue, Ak::ShowContextMenu,
];

/// No wildcard arm, so an action a newer AccessKit adds stops this match compiling. Add the
/// new action to `EVERY` as well as here: nothing checks the two against each other.
#[allow(dead_code)]
fn every_is_complete(a: Ak) {
    match a {
        Ak::Click | Ak::Focus | Ak::Blur | Ak::Collapse | Ak::Expand | Ak::CustomAction | Ak::Decrement | Ak::Increment
        | Ak::HideTooltip | Ak::ShowTooltip | Ak::ReplaceSelectedText | Ak::ScrollDown | Ak::ScrollLeft | Ak::ScrollRight
        | Ak::ScrollUp | Ak::ScrollIntoView | Ak::ScrollToPoint | Ak::SetScrollOffset | Ak::SetTextSelection
        | Ak::SetSequentialFocusNavigationStartingPoint | Ak::SetValue | Ak::ShowContextMenu => {}
    }
}

fn find<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    if v["key"] == key { return Some(v); }
    v["children"].as_array()?.iter().find_map(|c| find(c, key))
}

#[derive(Default)]
struct Every {
    edit: TextEdit,
}

/// One of each control the runtime adds actions to (`a11y/node.rs`, and links in
/// `a11y/text.rs`), each with the actions it reaches. A new `add_action` there needs a control
/// here that reaches it.
fn every(s: &Every, _: &mut Cx<Every>) -> El {
    col().w_full().gap(8.0)
        // Click, Focus, ShowContextMenu.
        .child(row().key("button").role("button").focusable(true).on_press(|_: &mut Every| {})
            .on_secondary_press(|_: &mut Every, _, _| {}).child(text("Save")))
        // Expand, and Collapse once open.
        .child(row().key("closed").role("button").expanded(false).on_press(|_: &mut Every| {}).child(text("More")))
        .child(row().key("open").role("button").expanded(true).on_press(|_: &mut Every| {}).child(text("Less")))
        // Increment, Decrement, and SetValue for its step.
        .child(crate::el().key("slider").role("slider").range_value(3.0, 0.0, 10.0).range_step(1.0).label("Volume")
            .size(100.0, 12.0).on_step(|_: &mut Every, _, _| {}))
        // Click, Focus, SetValue, ReplaceSelectedText, SetTextSelection.
        .child(text_input(&s.edit, crate::tok::type_::BODY).key("field").single_line().w(200.0)
            .on_edit(|s: &mut Every| &mut s.edit))
        // A link's Click, Focus and ShowContextMenu.
        .child(rich("Read the spec now.", vec![]).key("para").w(300.0)
            .link(Link::new(9..13, "https://example.com/spec"))
            .on_link(|_: &mut Every, _, _| {})
            .on_link_secondary(|_: &mut Every, _, _, _| {}))
        // ScrollUp, ScrollDown, ScrollLeft, ScrollRight, SetScrollOffset; ScrollIntoView inside.
        .child(col().key("scroller").size(200.0, 100.0).scroll_xy()
            .child(col().size(600.0, 400.0).child(text("Wide content").key("inside"))))
}

/// Every action a node of the scene supports has an entry in [`ACTIONS`], or the dump (`--tree`,
/// `Runtime::accessibility_json`, the Kit goldens) would leave it out; and every entry shows up on
/// some node, so the scene keeps reaching every action the table names.
#[test]
fn the_action_table_names_every_action_the_tree_offers() {
    let mut rt = Runtime::new(Every::default(), every, (400.0, 500.0), Env::new(true, 1.0));
    for _ in 0..4 { rt.frame(0.0); }
    let tree = rt.accessibility_tree();
    let named = |a: Ak| ACTIONS.iter().any(|(t, _)| *t == a);
    let mut missing = Vec::new();
    for (_, n) in &tree.nodes {
        for a in EVERY.into_iter().filter(|&a| n.supports_action(a) && !named(a)) {
            missing.push(format!("{a:?} on {:?} {:?}", n.role(), n.author_id().unwrap_or("")));
        }
    }
    assert!(missing.is_empty(), "actions the dump leaves out: {missing:?}");
    let unseen: Vec<&str> = ACTIONS.iter()
        .filter(|(a, _)| !tree.nodes.iter().any(|(_, n)| n.supports_action(*a)))
        .map(|(_, s)| *s).collect();
    assert!(unseen.is_empty(), "no node of the scene supports {unseen:?}; add a control that does");
    let json = rt.accessibility_json();
    let scroller = find(&json, "scroller").unwrap_or_else(|| panic!("no scroller in {json:#}"));
    let actions: Vec<&str> = scroller["actions"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
    assert!(actions.contains(&"scroll-left") && actions.contains(&"scroll-right"), "{actions:?}");
}
