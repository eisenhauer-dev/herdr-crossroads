use crate::input::{Arrow, Key, Keys};
use crate::keymap::{Action, Context, Keymap};
use crate::test_support::ui;

pub(crate) fn decode(bytes: &[u8]) -> Vec<Key> {
    let mut keys = Keys::new();
    bytes.iter().filter_map(|b| keys.feed(*b)).collect()
}

#[test]
fn sgr_mouse_press_is_an_event_not_search_text() {
    assert_eq!(decode(b"\x1b[<0;7;5M").len(), 1);
}

#[test]
fn windows_mouse_input_policy_preserves_vt_bytes_and_disables_quick_edit() {
    use crate::terminal::windows_input::input_mode;
    for saved in [0, 0xffff, 0x17, 0x200] {
        let disabled = input_mode(saved, false);
        assert_eq!(
            disabled & 0x57,
            0,
            "no cooked/echo/Ctrl-C/legacy mouse/Quick Edit"
        );
        assert_eq!(disabled & 0x280, 0x280, "VT input and extended modes");
        let enabled = input_mode(saved, true);
        assert_eq!(
            enabled & 0x210,
            0x210,
            "ConPTY mouse delivery plus VT byte input, not legacy-only records"
        );
        assert_eq!(
            enabled & 0x47,
            0,
            "cooked/echo/Ctrl-C/Quick Edit remain off"
        );
        assert_eq!(
            enabled & !0x2d7,
            saved & !0x2d7,
            "unrelated console flags preserved"
        );
    }
}

#[test]
fn oversized_mouse_report_is_consumed_through_its_terminator() {
    let mut report = b"\x1b[<0;".to_vec();
    report.extend(vec![b'9'; 256]);
    report.extend_from_slice(b";5Mx");
    assert_eq!(decode(&report), [Key::Char('x')]);
}

#[test]
fn mouse_reports_decode_fragmented_coordinates_release_and_wheel_without_key_leaks() {
    use crate::input::{Mouse, MouseButton};
    for (bytes, button) in [
        (b"\x1b[<0;65535;1M".as_slice(), MouseButton::Left),
        (b"\x1b[<0;65535;1m", MouseButton::Release),
        (b"\x1b[<64;65535;1M", MouseButton::WheelUp),
        (b"\x1b[<65;65535;1M", MouseButton::WheelDown),
    ] {
        let mut decoder = Keys::new();
        for byte in &bytes[..bytes.len() - 1] {
            assert_eq!(decoder.feed(*byte), None);
        }
        assert_eq!(
            decoder.feed(*bytes.last().unwrap()),
            Some(Key::Mouse(Mouse {
                button,
                x: 65534,
                y: 0
            }))
        );
        assert_eq!(decoder.feed(b'x'), Some(Key::Char('x')));
    }
    for report in [
        "<0;0;1M",
        "<0;1;0M",
        "<0;65536;1M",
        "<0;+1;1M",
        "<a;1;1M",
        "<0;;1M",
        "<0;1;1;1M",
        "<4;1;1M",
        "<8;1;1M",
        "<16;1;1M",
        "<1;1;1M",
        "<2;1;1M",
        "<32;1;1M",
        "<66;1;1M",
        "<65;1;1m",
    ] {
        assert_eq!(
            decode(format!("\x1b[{report}x").as_bytes()),
            [Key::Char('x')],
            "{report}"
        );
    }
    assert_eq!(
        decode(b"\x1b[<0;12\x1b[?1u\x1b[A\x1b[106u"),
        [Key::Up, Key::Char('j')]
    );
}

#[test]
fn raw_ht_cr_aliases_remain_physical_byte_limits_not_configured_chords() {
    assert_eq!(decode(b"\t\r"), [Key::Tab, Key::Enter]);
    let map = Keymap::default();
    assert_eq!(
        map.action(Context::Select, decode(b"\t")[0]),
        Some(Action::NextWorkspace)
    );
    assert_eq!(
        map.action(Context::Select, decode(b"\r")[0]),
        Some(Action::FocusPane)
    );
    assert_eq!(map.action(Context::Select, Key::Ctrl('i')), None);
    assert_eq!(map.action(Context::Select, Key::Ctrl('m')), None);
}

#[test]
fn classic_and_kitty_keep_control_identities_and_arrows() {
    assert_eq!(
        decode(b"\n\x08\r\x7f\t"),
        [
            Key::Ctrl('j'),
            Key::Ctrl('h'),
            Key::Enter,
            Key::Backspace,
            Key::Tab
        ]
    );
    assert_eq!(
        decode(b"\x1bj\x1bOA\x1b[B\x1b[1;5D\x1b[1;3C"),
        [
            Key::Alt('j'),
            Key::Up,
            Key::Down,
            Key::CtrlArrow(Arrow::Left),
            Key::AltArrow(Arrow::Right)
        ]
    );
    assert_eq!(
        decode(b"\x1b[105;5u\x1b[109;5u\x1b[13u\x1b[9u\x1b[106;5u\x1b[104;5u"),
        [
            Key::Ctrl('i'),
            Key::Ctrl('m'),
            Key::Enter,
            Key::Tab,
            Key::Ctrl('j'),
            Key::Ctrl('h')
        ]
    );
    assert_eq!(
        decode(b"\x1b[9;2u\x1b[106;2u\x1b[57352;3u"),
        [Key::BackTab, Key::Char('J'), Key::AltArrow(Arrow::Up)]
    );
    assert!(decode(b"\x1b[106;7u\x1b[106;5:3u\x1b[27;3u\x1b[1;6A").is_empty());
    let mut keys = Keys::new();
    for byte in b"\x1b[?1u" {
        keys.feed(*byte);
    }
    assert!(keys.kitty_response);
    keys.feed(27);
    assert_eq!(keys.flush(), Some(Key::Esc));
}

#[test]
fn repeated_escape_emits_the_first_escape_and_restarts_the_deadline() {
    use std::time::{Duration, Instant};
    let mut keys = Keys::new();
    assert_eq!(keys.feed(27), None);
    keys.escape_started = Some(Instant::now() - Duration::from_millis(50));
    let second_press = Instant::now();
    assert_eq!(keys.feed(27), Some(Key::Esc));
    assert_eq!(keys.escape, 1);
    assert!(keys.escape_started.unwrap() >= second_press);
    assert_eq!(keys.flush(), Some(Key::Esc));
    assert_eq!(keys.flush(), None);
}

#[test]
fn repeated_escape_keeps_csi_and_ss3_sequences_out_of_search_text() {
    use crate::selection::Mode;
    for (bytes, expected) in [
        (b"\x1b\x1b[A".as_slice(), vec![Key::Esc, Key::Up]),
        (b"\x1b\x1bOA", vec![Key::Esc, Key::Up]),
        (b"\x1b\x1b\x1b[A", vec![Key::Esc, Key::Esc, Key::Up]),
        (b"\x1b\x1b[1;5A", vec![Key::Esc, Key::CtrlArrow(Arrow::Up)]),
        (b"\x1b\x1b[15~", vec![Key::Esc]),
        (b"\x1b\x1bj", vec![Key::Esc, Key::Alt('j')]),
        (b"\x1b\x1b[?1u", vec![Key::Esc]),
    ] {
        assert_eq!(decode(bytes), expected, "{bytes:?}");
    }
    for help in [false, true] {
        let mut state = ui();
        if help {
            state.key(Key::Char('?'));
        }
        state.key(Key::Char('f'));
        state.key(Key::Char('x'));
        for key in decode(b"\x1b\x1b[A") {
            state.key(key);
        }
        if help {
            assert_eq!(state.mode, Mode::Help);
            assert!(!state.help.searching);
            assert_eq!(state.help.query, "x");
        } else {
            assert_eq!(state.mode, Mode::Select);
            assert_eq!(state.query, "x");
        }
        assert!(!state.closed);
    }
}

#[test]
fn malformed_alt_utf8_does_not_modify_the_following_plain_key() {
    assert_eq!(decode(b"\x1b\xffx"), [Key::Char('x')]);
    assert_eq!(decode("é".as_bytes()), [Key::Char('é')]);
    assert_eq!(decode("\x1béx".as_bytes()), [Key::Char('x')]);
}
