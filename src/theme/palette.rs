use ratatui::style::Color;

// Herdr v0.9.1: src/config/theme.rs THEME_NAMES and src/app/state.rs Palette.
pub(crate) const NAMES: [&str; 19] = [
    "Auto (Herdr)",
    "catppuccin",
    "catppuccin-latte",
    "terminal",
    "tokyo-night",
    "tokyo-night-day",
    "dracula",
    "nord",
    "gruvbox",
    "gruvbox-light",
    "one-dark",
    "one-light",
    "solarized",
    "solarized-light",
    "kanagawa",
    "kanagawa-lotus",
    "rose-pine",
    "rose-pine-dawn",
    "vesper",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Theme {
    pub(crate) background: Color,
    pub(crate) accent: Color,
    pub(crate) text: Color,
    pub(crate) dim: Color,
    pub(crate) selection: Color,
    pub(crate) working: Color,
    pub(crate) blocked: Color,
    pub(crate) done: Color,
    pub(crate) idle: Color,
    pub(crate) unknown: Color,
}

fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Theme {
    pub(crate) fn named(name: &str) -> Self {
        // Herdr v0.9.1 Palette::panel_bg (terminal retains the host default).
        let background = match name {
            "catppuccin-latte" => 0xeff1f5,
            "tokyo-night" => 0x1a1b26,
            "tokyo-night-day" => 0xe1e2e7,
            "dracula" => 0x282a36,
            "nord" => 0x2e3440,
            "gruvbox" => 0x282828,
            "gruvbox-light" => 0xfbf1c7,
            "one-dark" => 0x282c34,
            "one-light" => 0xfafafa,
            "solarized" => 0x002b36,
            "solarized-light" => 0xfdf6e3,
            "kanagawa" => 0x1f1f28,
            "kanagawa-lotus" => 0xf2ecbc,
            "rose-pine" => 0x191724,
            "rose-pine-dawn" => 0xfaf4ed,
            "vesper" => 0x1a1a1a,
            _ => 0x181825,
        };
        // accent, text, overlay0, selection_bg, yellow, red, teal, green.
        let values = match name {
            "catppuccin-latte" => [
                0x1e66f5, 0x4c4f69, 0x9ca0b0, 0xbdd0f5, 0xdf8e1d, 0xd20f39, 0x179299, 0x40a02b,
            ],
            "tokyo-night" => [
                0x7aa2f7, 0xc0caf5, 0x565f89, 0x2d3650, 0xe0af68, 0xf7768e, 0x7dcfff, 0x9ece6a,
            ],
            "tokyo-night-day" => [
                0x2e7de9, 0x3760bf, 0x8990b3, 0xb6cae7, 0x8c6c3e, 0xf52a65, 0x118c74, 0x587539,
            ],
            "dracula" => [
                0xbd93f9, 0xf8f8f2, 0x6272a4, 0x463f5d, 0xf1fa8c, 0xff5555, 0x8be9fd, 0x50fa7b,
            ],
            "nord" => [
                0x88c0d0, 0xeceff4, 0x4c566a, 0x40505d, 0xebcb8b, 0xbf616a, 0x8fbcbb, 0xa3be8c,
            ],
            "gruvbox" => [
                0xd79921, 0xebdbb2, 0x928374, 0x4b3f27, 0xfabd2f, 0xfb4934, 0x8ec07c, 0xb8bb26,
            ],
            "gruvbox-light" => [
                0x076678, 0x3c3836, 0x928374, 0xebdbb2, 0xb57614, 0x9d0006, 0x427b58, 0x79740e,
            ],
            "one-dark" => [
                0x61afef, 0xabb2bf, 0x5c6370, 0x334659, 0xe5c07b, 0xe06c75, 0x56b6c2, 0x98c379,
            ],
            "one-light" => [
                0x4078f2, 0x383a42, 0xa0a1a7, 0xcddbf8, 0xc18401, 0xe45649, 0x0184bc, 0x50a14f,
            ],
            "solarized" => [
                0x268bd2, 0x93a1a1, 0x586e75, 0x083e55, 0xb58900, 0xdc322f, 0x2aa198, 0x859900,
            ],
            "solarized-light" => [
                0x268bd2, 0x657b83, 0x93a1a1, 0xc9dcdf, 0xb58900, 0xdc322f, 0x2aa198, 0x859900,
            ],
            "kanagawa" => [
                0x7e9cd8, 0xdcd7ba, 0x727169, 0x32384b, 0xc0a36e, 0xc34043, 0x7fb4ca, 0x76946a,
            ],
            "kanagawa-lotus" => [
                0x4d699b, 0x545464, 0xa09cac, 0xdcd5ac, 0x77713f, 0xc84053, 0x4e8ca2, 0x6f894e,
            ],
            "rose-pine" => [
                0xc4a7e7, 0xe0def4, 0x6e6a86, 0x3b344b, 0xf6c177, 0xeb6f92, 0x9ccfd8, 0x31748f,
            ],
            "rose-pine-dawn" => [
                0x907aa9, 0x464261, 0x9893a5, 0xf2e9e1, 0xea9d34, 0xb4637a, 0x56949f, 0x286983,
            ],
            "vesper" => [
                0xffc799, 0xffffff, 0x5c5c5c, 0x232323, 0xffc799, 0xff8080, 0x66ddcc, 0x99ffe4,
            ],
            _ => [
                0x89b4fa, 0xcdd6f4, 0x6c7086, 0x313244, 0xf9e2af, 0xf38ba8, 0x94e2d5, 0xa6e3a1,
            ],
        };
        if name == "terminal" {
            return Self {
                background: Color::Reset,
                accent: Color::Blue,
                text: Color::Reset,
                dim: Color::Gray,
                selection: Color::Reset,
                working: Color::Yellow,
                blocked: Color::LightRed,
                done: Color::Cyan,
                idle: Color::Green,
                unknown: Color::Gray,
            };
        }
        Self {
            background: rgb(background),
            accent: rgb(values[0]),
            text: rgb(values[1]),
            dim: rgb(values[2]),
            selection: rgb(values[3]),
            working: rgb(values[4]),
            blocked: rgb(values[5]),
            done: rgb(values[6]),
            idle: rgb(values[7]),
            unknown: rgb(values[2]),
        }
    }
}
