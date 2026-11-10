//! The start-up splash: logo rises and fades in, then the whole overlay fades out.

use std::time::Duration;

use super::*;
use crate::app::SPLASH_FADE;

impl Audible {
    pub(super) fn splash_overlay(&self) -> Option<AnyElement> {
        let p = self.pal;
        let leaving = match self.splash {
            Splash::Gone => return None,
            Splash::Leaving => true,
            Splash::Showing => false,
        };

        let loader = div()
            .w(px(140.0))
            .h(px(3.0))
            .rounded_full()
            .bg(p.raised)
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .w(px(56.0))
                    .rounded_full()
                    .bg(p.accent)
                    .with_animation(
                        "splash-load",
                        Animation::new(Duration::from_millis(1100))
                            .repeat()
                            .with_easing(ease_in_out),
                        |el, d| el.ml(px(84.0 * d)),
                    ),
            );

        let content = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .child(logo(124.0))
            .child(div().h(px(14.0)))
            .child(
                div()
                    .font_family(SANS)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(26.0))
                    .text_color(p.text)
                    .child("Hymn Book"),
            )
            .child(
                div()
                    .font_family(SANS)
                    .text_size(px(13.0))
                    .text_color(p.text_3)
                    .child("Faith Tabernacle"),
            )
            .child(div().h(px(22.0)))
            .child(loader)
            .with_animation(
                "splash-in",
                Animation::new(Duration::from_millis(700)).with_easing(ease_out_quint()),
                |el, d| el.opacity(d).mt(px(28.0 * (1.0 - d))),
            );

        let overlay = div()
            .id("splash")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(p.bg)
            // swallow clicks so nothing underneath reacts during the splash
            .occlude()
            .child(content);

        Some(if leaving {
            overlay
                .with_animation(
                    "splash-out",
                    Animation::new(SPLASH_FADE).with_easing(ease_in_out),
                    |el, d| el.opacity(1.0 - d),
                )
                .into_any_element()
        } else {
            overlay.into_any_element()
        })
    }
}
