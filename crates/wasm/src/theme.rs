use termy_core::theme_core::{Rgb8, ThemeColors, normalize_theme_id};
use termy_core::themes;

/// Themes compiled into `termy_core`. The global registry only holds providers
/// a host registers, so the web build lists its bundled themes explicitly.
type BuiltinTheme = (&'static str, fn() -> ThemeColors);

const BUILTIN_THEMES: &[BuiltinTheme] = &[
    ("termy", themes::termy),
    ("termy-light", themes::termy_light),
    ("tokyo-night", themes::tokyo_night),
    ("catppuccin-mocha", themes::catppuccin_mocha),
    ("dracula", themes::dracula),
    ("gruvbox-dark", themes::gruvbox_dark),
    ("nord", themes::nord),
    ("solarized-dark", themes::solarized_dark),
    ("one-dark", themes::one_dark),
    ("monokai", themes::monokai),
    ("material-dark", themes::material_dark),
    ("palenight", themes::palenight),
    ("tomorrow-night", themes::tomorrow_night),
    ("oceanic-next", themes::oceanic_next),
];

pub fn theme_ids() -> Vec<String> {
    BUILTIN_THEMES
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .collect()
}

/// `[fg, bg, cursor, ansi0..15]` as `0xRRGGBB`.
pub fn theme_colors(id: &str) -> Option<Vec<u32>> {
    let id = normalize_theme_id(id);
    BUILTIN_THEMES
        .iter()
        .find(|(builtin, _)| *builtin == id)
        .map(|(_, theme)| flatten(&theme()))
}

fn flatten(theme: &ThemeColors) -> Vec<u32> {
    let hex =
        |color: Rgb8| (u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b);
    [theme.foreground, theme.background, theme.cursor]
        .into_iter()
        .chain(theme.ansi)
        .map(hex)
        .collect()
}
