//! Custom title bar (we draw our own window chrome on every platform).

use super::*;

const TITLEBAR_HEIGHT: f32 = 52.0;
const MAC: bool = cfg!(target_os = "macos");
const WINDOWS: bool = cfg!(target_os = "windows");

impl Audible {
    pub(super) fn titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let close_hover = Rgba {
            r: 0.9,
            g: 0.28,
            b: 0.3,
            a: 1.0,
        };

        let brand = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .child(logo(30.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_family(SANS)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(14.0))
                            .text_color(p.text)
                            .child("Hymn Book"),
                    )
                    .child(
                        div()
                            .font_family(SANS)
                            .text_size(px(11.0))
                            .text_color(p.text_3)
                            .child("Faith Tabernacle"),
                    ),
            );

        let theme_icon = if p.dark {
            IconName::Sun
        } else {
            IconName::Moon
        };
        let mut controls = div().flex().flex_row().items_center().gap(px(4.0)).child(
            icon_button("theme", theme_icon, p, p.raised)
                .on_click(cx.listener(|this, _, window, cx| this.toggle_theme(window, cx))),
        );

        // macOS draws its own traffic-light buttons; elsewhere we draw ours.
        if !MAC {
            let mut min = icon_button("min", IconName::WindowMinimize, p, p.raised)
                .on_click(|_, window, _| window.minimize_window());
            let mut max = icon_button("max", IconName::WindowMaximize, p, p.raised)
                .on_click(|_, window, _| window.zoom_window());
            let mut close = icon_button("close", IconName::WindowClose, p, close_hover)
                .on_click(|_, _, cx| cx.quit());
            if WINDOWS {
                // Windows hit-tests these regions natively.
                min = min.window_control_area(WindowControlArea::Min);
                max = max.window_control_area(WindowControlArea::Max);
                close = close.window_control_area(WindowControlArea::Close);
            }
            controls = controls
                .child(div().w(px(8.0)))
                .child(min)
                .child(max)
                .child(close);
        }

        div()
            .id("titlebar")
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .h(px(TITLEBAR_HEIGHT))
            .flex_none()
            // leave room for the macOS traffic lights
            .pl(px(if MAC { 84.0 } else { 20.0 }))
            .pr(px(20.0))
            .window_control_area(WindowControlArea::Drag)
            // Compositor takes over the move (Wayland/X11).
            .on_mouse_down(MouseButton::Left, |event, window, _| {
                if event.click_count == 2 {
                    window.zoom_window();
                } else {
                    window.start_window_move();
                }
            })
            .on_mouse_down(MouseButton::Right, |event, window, _| {
                window.show_window_menu(event.position);
            })
            .child(brand)
            .child(controls)
    }
}
