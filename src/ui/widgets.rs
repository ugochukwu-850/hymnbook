//! Small reusable controls.

use super::*;

/// Round icon-only button.
pub fn icon_button(id: &'static str, icon: IconName, p: Palette, hover: Rgba) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(36.0))
        .rounded_full()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(Icon::new(icon).size_4().text_color(p.text_2))
        // Keep the click from starting a window drag.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// Small text pill that is either active (accent) or quiet.
pub fn pill_button(
    id: impl Into<ElementId>,
    label: &'static str,
    active: bool,
    p: Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .px(px(12.0))
        .h(px(28.0))
        .rounded_full()
        .cursor_pointer()
        .font_family(SANS)
        .font_weight(FontWeight::BOLD)
        .text_size(px(12.0))
        .bg(if active {
            p.accent_soft
        } else {
            Rgba { a: 0.0, ..p.raised }
        })
        .text_color(if active { p.accent } else { p.text_3 })
        .child(label)
}

/// `m:ss`
pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}
