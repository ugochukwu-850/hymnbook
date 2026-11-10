//! Colour palettes. Dark first, with a matching light mode.
//! One accent (warm gold) and soft neutral surfaces; no gradients.

use gpui_kit::Rgba;

#[derive(Clone, Copy)]
pub struct Palette {
    /// Window background, behind the panels.
    pub bg: Rgba,
    /// Panel surface (library, reader).
    pub panel: Rgba,
    /// Raised surface on top of a panel (search field, chips, player).
    pub raised: Rgba,
    /// Hover state for rows and buttons.
    pub hover: Rgba,
    /// Hairline border.
    pub line: Rgba,
    pub text: Rgba,
    pub text_2: Rgba,
    pub text_3: Rgba,
    pub accent: Rgba,
    /// Accent at low opacity, for selected rows and chorus cards.
    pub accent_soft: Rgba,
    /// Text/icon colour on top of `accent`.
    pub on_accent: Rgba,
    pub dark: bool,
}

const fn hex(v: u32) -> Rgba {
    Rgba {
        r: ((v >> 16) & 0xFF) as f32 / 255.0,
        g: ((v >> 8) & 0xFF) as f32 / 255.0,
        b: (v & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

const fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba { a, ..c }
}

impl Palette {
    pub const fn dark() -> Self {
        let accent = hex(0xF0B35A);
        Self {
            bg: hex(0x08090C),
            panel: hex(0x121419),
            raised: hex(0x1B1E26),
            hover: hex(0x181B22),
            line: with_alpha(hex(0xFFFFFF), 0.07),
            text: hex(0xEEEFF2),
            text_2: hex(0xA6AAB6),
            text_3: hex(0x6C7180),
            accent,
            accent_soft: with_alpha(accent, 0.13),
            on_accent: hex(0x1A1204),
            dark: true,
        }
    }

    pub const fn light() -> Self {
        let accent = hex(0xB86A0B);
        Self {
            bg: hex(0xE9EAEE),
            panel: hex(0xFFFFFF),
            raised: hex(0xF1F2F6),
            hover: hex(0xF6F7FA),
            line: with_alpha(hex(0x000000), 0.08),
            text: hex(0x14161C),
            text_2: hex(0x545968),
            text_3: hex(0x8A8F9E),
            accent,
            accent_soft: with_alpha(accent, 0.10),
            on_accent: hex(0xFFFFFF),
            dark: false,
        }
    }
}
