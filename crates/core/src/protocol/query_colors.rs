use crate::TerminalColor;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalQueryColors {
    pub ansi: [TerminalColor; 16],
    pub foreground: TerminalColor,
    pub background: TerminalColor,
    // Retained as part of the query fallback contract, but OSC 12 replies still
    // only come from an explicit live cursor override in the terminal state.
    pub cursor: Option<TerminalColor>,
}

impl Default for TerminalQueryColors {
    fn default() -> Self {
        Self {
            ansi: [
                TerminalColor {
                    r: 0x00,
                    g: 0x00,
                    b: 0x00,
                },
                TerminalColor {
                    r: 0xcd,
                    g: 0x00,
                    b: 0x00,
                },
                TerminalColor {
                    r: 0x00,
                    g: 0xcd,
                    b: 0x00,
                },
                TerminalColor {
                    r: 0xcd,
                    g: 0xcd,
                    b: 0x00,
                },
                TerminalColor {
                    r: 0x00,
                    g: 0x00,
                    b: 0xee,
                },
                TerminalColor {
                    r: 0xcd,
                    g: 0x00,
                    b: 0xcd,
                },
                TerminalColor {
                    r: 0x00,
                    g: 0xcd,
                    b: 0xcd,
                },
                TerminalColor {
                    r: 0xe5,
                    g: 0xe5,
                    b: 0xe5,
                },
                TerminalColor {
                    r: 0x7f,
                    g: 0x7f,
                    b: 0x7f,
                },
                TerminalColor {
                    r: 0xff,
                    g: 0x00,
                    b: 0x00,
                },
                TerminalColor {
                    r: 0x00,
                    g: 0xff,
                    b: 0x00,
                },
                TerminalColor {
                    r: 0xff,
                    g: 0xff,
                    b: 0x00,
                },
                TerminalColor {
                    r: 0x5c,
                    g: 0x5c,
                    b: 0xff,
                },
                TerminalColor {
                    r: 0xff,
                    g: 0x00,
                    b: 0xff,
                },
                TerminalColor {
                    r: 0x00,
                    g: 0xff,
                    b: 0xff,
                },
                TerminalColor {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                },
            ],
            foreground: TerminalColor {
                r: 0xe5,
                g: 0xe5,
                b: 0xe5,
            },
            background: TerminalColor {
                r: 0x1e,
                g: 0x1e,
                b: 0x1e,
            },
            cursor: None,
        }
    }
}

impl TerminalQueryColors {
    pub(crate) fn indexed_color(self, idx: u8) -> TerminalColor {
        match idx {
            0..=15 => self.ansi[idx as usize],
            16..=231 => {
                let idx = idx - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                let to_component = |value: u8| if value == 0 { 0 } else { 55 + (value * 40) };
                TerminalColor {
                    r: to_component(r),
                    g: to_component(g),
                    b: to_component(b),
                }
            }
            232..=255 => {
                let gray = 8 + ((idx - 232) * 10);
                TerminalColor {
                    r: gray,
                    g: gray,
                    b: gray,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalQueryColors;
    use crate::{
        TerminalColor,
        terminal_engine::{Engine, Options, Size},
    };

    fn reply(colors: TerminalQueryColors, input: &[u8]) -> Vec<u8> {
        let mut engine = Engine::new(Size { cols: 32, rows: 4 }, Options::default());
        engine.set_query_colors(colors);
        engine.feed(input);
        let mut replies = Vec::new();
        engine.drain_replies(&mut replies);
        replies
    }

    #[test]
    fn indexed_colors_fall_back_to_generated_palette() {
        let colors = TerminalQueryColors::default();
        assert_eq!(colors.indexed_color(16), TerminalColor { r: 0, g: 0, b: 0 });
        assert_eq!(
            colors.indexed_color(231),
            TerminalColor {
                r: 255,
                g: 255,
                b: 255
            }
        );
        assert_eq!(
            colors.indexed_color(232),
            TerminalColor { r: 8, g: 8, b: 8 }
        );
        assert_eq!(
            colors.indexed_color(255),
            TerminalColor {
                r: 238,
                g: 238,
                b: 238
            }
        );
        assert_eq!(
            reply(colors, b"\x1b]4;232;?\x07"),
            b"\x1b]4;232;rgb:0808/0808/0808\x1b\\"
        );
    }

    #[test]
    fn configured_ansi_colors_supply_protocol_fallbacks() {
        let mut colors = TerminalQueryColors::default();
        colors.ansi[4] = TerminalColor { r: 1, g: 2, b: 3 };
        assert_eq!(
            reply(colors, b"\x1b]4;4;?\x07"),
            b"\x1b]4;4;rgb:0101/0202/0303\x1b\\"
        );
    }

    #[test]
    fn live_foreground_override_wins_over_fallback() {
        let defaults = TerminalQueryColors {
            foreground: TerminalColor {
                r: 0xaa,
                g: 0xbb,
                b: 0xcc,
            },
            ..TerminalQueryColors::default()
        };
        assert_eq!(
            reply(defaults, b"\x1b]10;#123456\x07\x1b]10;?\x07"),
            b"\x1b]10;rgb:1212/3434/5656\x1b\\"
        );
    }

    #[test]
    fn reset_foreground_reverts_to_fallback() {
        let defaults = TerminalQueryColors {
            foreground: TerminalColor {
                r: 0x44,
                g: 0x55,
                b: 0x66,
            },
            ..TerminalQueryColors::default()
        };
        assert_eq!(
            reply(defaults, b"\x1b]10;#123456\x07\x1b]110\x07\x1b]10;?\x07"),
            b"\x1b]10;rgb:4444/5555/6666\x1b\\"
        );
    }

    #[test]
    fn cursor_queries_require_live_override() {
        let defaults = TerminalQueryColors {
            cursor: Some(TerminalColor {
                r: 0xab,
                g: 0xcd,
                b: 0xef,
            }),
            ..TerminalQueryColors::default()
        };
        assert!(reply(defaults, b"\x1b]12;?\x07").is_empty());
        assert_eq!(
            reply(defaults, b"\x1b]12;#102030\x07\x1b]12;?\x07"),
            b"\x1b]12;rgb:1010/2020/3030\x1b\\"
        );
    }

    #[test]
    fn unsupported_palette_index_has_no_reply() {
        assert!(reply(TerminalQueryColors::default(), b"\x1b]4;256;?\x07").is_empty());
    }
}
