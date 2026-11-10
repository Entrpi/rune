//! A secure field fed as the browser's password sink feeds it: keystrokes, fills and pastes.
use super::*;

struct Login {
    pw: crate::TextEdit,
    key: crate::TextEdit,
    changes: usize,
}

fn login(s: &Login, _: &mut Cx<Login>) -> El {
    crate::col()
        .child(crate::text_input(&s.pw, crate::tok::type_::BODY).key("pw").single_line().secure().w(200.0)
            .on_edit(|s: &mut Login| &mut s.pw).on_input_change(|s: &mut Login, _| s.changes += 1))
        .child(crate::text_input(&s.key, crate::tok::type_::BODY).key("key").secure().w(200.0)
            .on_edit(|s: &mut Login| &mut s.key))
}

/// A runtime with the field keyed `key` focused, and the field's path.
fn focused(key: &str) -> (Runtime<Login>, String) {
    let state = Login { pw: crate::TextEdit::new("").secure(), key: crate::TextEdit::new("").secure(), changes: 0 };
    let mut rt = Runtime::new(state, login, (300.0, 200.0), Env::default());
    rt.trace_channels = Some(Default::default());
    rt.frame(0.0);
    let path = rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
    rt.set_focus(Some(path.clone()), true);
    rt.frame(0.0);
    assert!(rt.has_secure_focus());
    (rt, path)
}

#[test]
fn a_fill_replaces_what_was_typed_as_one_edit() {
    let (mut rt, path) = focused("pw");
    for c in "abcdefgh".chars() { rt.web_secure(&path, c.to_string()); rt.frame(0.0); }
    assert_eq!((rt.state.pw.text.as_str(), rt.state.changes), ("abcdefgh", 8));
    rt.web_secure_replace(&path, "0123456789abc".into());
    rt.frame(0.0);
    assert_eq!(rt.state.pw.text, "0123456789abc", "13 filled over 8 typed leaves the 13");
    assert_eq!((rt.state.changes, rt.state.pw.range()), (9, (13, 13)), "one edit, the caret after it");
    rt.web_secure(&path, "!".into());
    rt.frame(0.0);
    assert_eq!(rt.state.pw.text, "0123456789abc!", "a keystroke after a fill inserts at the caret");
    // A fill meant for a field that has since lost focus is dropped.
    rt.web_secure_replace("elsewhere", "other".into());
    rt.frame(0.0);
    assert_eq!((rt.state.pw.text.as_str(), rt.state.changes), ("0123456789abc!", 10));
}

#[test]
fn filling_a_filled_field_leaves_only_the_second_value() {
    for key in ["pw", "key"] {
        let (mut rt, path) = focused(key);
        rt.web_secure(&path, "typed".into());
        rt.frame(0.0);
        rt.web_secure_replace(&path, "first fill".into());
        rt.frame(0.0);
        rt.web_secure_replace(&path, "second".into());
        rt.frame(0.0);
        let text = if key == "pw" { &rt.state.pw.text } else { &rt.state.key.text };
        assert_eq!(text, "second", "{key}");
    }
}

#[test]
fn an_empty_fill_clears_the_field() {
    let (mut rt, path) = focused("pw");
    rt.web_secure(&path, "typed".into());
    rt.frame(0.0);
    rt.web_secure_replace(&path, String::new());
    rt.frame(0.0);
    assert_eq!((rt.state.pw.text.as_str(), rt.state.changes), ("", 2));
}

#[test]
fn a_paste_into_a_secure_field_replaces_only_its_selection() {
    let (mut rt, path) = focused("pw");
    rt.web_secure(&path, "abcdefgh".into());
    rt.frame(0.0);
    rt.update(|s, _| s.pw.set_selection(2, 5));
    rt.frame(0.0);
    rt.paste("XY");
    rt.frame(0.0);
    assert_eq!(rt.state.pw.text, "abXYfgh");
}

#[test]
fn a_filled_password_reaches_only_the_model() {
    let (mut rt, path) = focused("pw");
    rt.web_secure_replace(&path, "0123456789abc".into());
    rt.frame(0.0);
    assert_eq!(rt.state.pw.text, "0123456789abc");
    let mut mirror = Mirror::default();
    rt.web_context(&mut mirror);
    assert!(mirror.secure && mirror.key == path && mirror.text.is_empty(), "the DOM sink is sent no text");
    let mut page = crate::runtime::web_a11y::A11yMirror::default();
    let page = rt.web_a11y(&mut page).map(str::to_owned).unwrap_or_default();
    let places = [
        ("event log and trace", rt.events.join("\n")),
        ("page mirror", page),
        ("accessibility tree", rt.accessibility_json().to_string()),
        ("copy", rt.copy_text().unwrap_or_default()),
        ("elements", format!("{:?}", rt.elements().iter().map(|e| e.text.clone()).collect::<Vec<_>>())),
        ("Debug", format!("{:?}", rt.state.pw)),
    ];
    for (place, s) in places { assert!(!s.contains("456789"), "{place}: {s}"); }
}
