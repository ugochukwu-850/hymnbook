//! Drawing. Each file renders one region of the window.

mod library;
mod logo;
mod player;
mod reader;
mod splash;
mod titlebar;
mod widgets;

#[allow(unused_imports)]
use gpui_kit::component::{Icon, IconName};
#[allow(unused_imports)]
use gpui_kit::*;

#[allow(unused_imports)]
use crate::app::{Audible, Splash};
#[allow(unused_imports)]
use crate::theme::Palette;
#[allow(unused_imports)]
use logo::logo;
#[allow(unused_imports)]
use widgets::{clock, icon_button, pill_button};

pub const SANS: &str = "Noto Sans";
pub const SERIF: &str = "Noto Serif";
pub const MONO: &str = "Noto Sans Mono";

impl Render for Audible {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;

        div()
            .id("root")
            .track_focus(&self.focus)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(SANS)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "down" => this.step(1, cx),
                    "up" => this.step(-1, cx),
                    "b" if event.keystroke.modifiers.control => this.toggle_sidebar(cx),
                    _ => {}
                }
            }))
            .child(self.titlebar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(12.0))
                    .px(px(12.0))
                    .pb(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_1()
                            .min_h_0()
                            .children(self.sidebar_slot(cx))
                            .child(self.reader_pane(cx)),
                    )
                    .child(self.player_bar(cx)),
            )
            .children(self.splash_overlay())
    }
}
