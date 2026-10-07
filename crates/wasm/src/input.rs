//! Keyboard, mouse and paste encoding through `termy_core`'s native encoders,
//! so web and desktop Termy send identical bytes.

use termy_core::terminal_engine::{Engine, MouseEncoding, MouseTracking};
use termy_core::{
    TerminalKeyEventKind, TerminalKeyboardMode, TerminalMouseButton, TerminalMouseEventKind,
    TerminalMouseMode, TerminalMouseModifiers, TerminalMousePosition, TermyKeystroke,
    TermyModifiers, encode_mouse_report, keystroke_to_input_with_options,
};

const CTRL: u32 = 1;
const ALT: u32 = 1 << 1;
const SHIFT: u32 = 1 << 2;
const META: u32 = 1 << 3;

pub fn keyboard_mode(engine: &Engine) -> TerminalKeyboardMode {
    let modes = engine.modes();
    let bits = modes.kitty_keyboard;
    TerminalKeyboardMode::from_flags(
        modes.application_cursor,
        bits & 1 != 0,
        bits & 2 != 0,
        bits & 4 != 0,
        bits & 8 != 0,
        bits & 16 != 0,
    )
}

pub fn mouse_mode(engine: &Engine) -> TerminalMouseMode {
    let modes = engine.modes();
    TerminalMouseMode {
        enabled: modes.mouse_tracking != MouseTracking::None,
        report_click: matches!(
            modes.mouse_tracking,
            MouseTracking::Click | MouseTracking::Press
        ),
        report_drag: modes.mouse_tracking == MouseTracking::Drag,
        report_motion: modes.mouse_tracking == MouseTracking::Motion,
        sgr_encoding: matches!(
            modes.mouse_encoding,
            MouseEncoding::Sgr | MouseEncoding::SgrPixels
        ),
        utf8_encoding: modes.mouse_encoding == MouseEncoding::Utf8,
    }
}

pub fn encode_key(
    engine: &Engine,
    key: &str,
    text: Option<String>,
    modifiers: u32,
    kind: u32,
    option_as_alt: bool,
) -> Option<Vec<u8>> {
    let keystroke = TermyKeystroke {
        modifiers: TermyModifiers {
            control: modifiers & CTRL != 0,
            alt: modifiers & ALT != 0,
            shift: modifiers & SHIFT != 0,
            platform: modifiers & META != 0,
            function: false,
        },
        key: key.to_owned(),
        key_char: text,
    };
    let kind = match kind {
        1 => TerminalKeyEventKind::Repeat,
        2 => TerminalKeyEventKind::Release,
        _ => TerminalKeyEventKind::Press,
    };
    keystroke_to_input_with_options(
        &keystroke,
        kind,
        keyboard_mode(engine),
        false,
        option_as_alt,
    )
}

pub fn encode_mouse(
    engine: &Engine,
    kind: u32,
    button: u32,
    col: u32,
    row: u32,
    modifiers: u32,
) -> Option<Vec<u8>> {
    let button = match button {
        1 => TerminalMouseButton::Middle,
        2 => TerminalMouseButton::Right,
        _ => TerminalMouseButton::Left,
    };
    let event = match kind {
        0 => TerminalMouseEventKind::Press(button),
        1 => TerminalMouseEventKind::Release(button),
        2 => TerminalMouseEventKind::Drag(button),
        3 => TerminalMouseEventKind::Move,
        4 => TerminalMouseEventKind::WheelUp,
        5 => TerminalMouseEventKind::WheelDown,
        6 => TerminalMouseEventKind::WheelLeft,
        7 => TerminalMouseEventKind::WheelRight,
        _ => return None,
    };
    encode_mouse_report(
        mouse_mode(engine),
        event,
        TerminalMousePosition {
            col: col as usize,
            row: row as usize,
        },
        TerminalMouseModifiers {
            shift: modifiers & SHIFT != 0,
            alt: modifiers & ALT != 0,
            control: modifiers & CTRL != 0,
        },
    )
}

pub fn encode_paste(bracketed: bool, text: &str) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
    if !bracketed {
        return normalized.into_bytes();
    }
    // A pasted end marker would let clipboard content escape the bracket.
    let body = normalized.replace("\x1b[201~", "");
    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"\x1b[200~");
    out.extend_from_slice(body.as_bytes());
    out.extend_from_slice(b"\x1b[201~");
    out
}
