//! The lyrics reader: header, verse/chorus blocks, and follow-along styling.

use std::time::Duration;

use super::*;
use crate::hymn::{Block, BlockKind};

/// Opacity of lyric blocks that are not being sung while a tune plays.
const DIMMED: f32 = 0.42;

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    /// No song is running: everything at full strength.
    Normal,
    /// This block is being sung (value: song section index, restarts the fade-in).
    Active(usize),
    /// Another block is being sung.
    Dim,
}

fn voice_pill(tag: &str, p: Palette, line_h: f32) -> impl IntoElement {
    let h = 20.0;
    div()
        .flex()
        .items_center()
        .flex_none()
        .h(px(h))
        .mt(px((line_h - h) / 2.0))
        .px(px(9.0))
        .rounded_full()
        .bg(Rgba {
            a: 0.18,
            ..p.accent
        })
        .font_family(MONO)
        .font_weight(FontWeight::BOLD)
        .text_size(px(10.5))
        .text_color(p.accent)
        .child(tag.to_uppercase())
}

fn block_view(block: &Block, p: Palette, size: f32, active: bool) -> Div {
    let line_h = (size * 1.62).round();
    let is_chorus = block.kind == BlockKind::Chorus;
    let clear = Rgba { a: 0.0, ..p.accent };
    let marker = if active { p.accent } else { clear };

    let lines = block.lines.iter().fold(
        div().flex().flex_col().flex_1().min_w_0().gap(px(2.0)),
        |col, line| {
            let mut row = div().flex().flex_row().items_start().gap(px(12.0));
            if let Some(tag) = &line.voice {
                row = row.child(voice_pill(tag, p, line_h));
            }
            col.child(
                row.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family(SERIF)
                        .text_size(px(size))
                        .line_height(px(line_h))
                        .text_color(p.text)
                        .child(line.text.clone()),
                ),
            )
        },
    );

    if is_chorus {
        return div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .w_full()
            .p(px(22.0))
            .rounded(px(20.0))
            .bg(p.accent_soft)
            .border_1()
            .border_color(marker)
            .child(
                div()
                    .font_family(SANS)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(11.0))
                    .text_color(p.accent)
                    .child("CHORUS"),
            )
            .child(lines);
    }

    let badge = match block.kind {
        BlockKind::Verse(Some(n)) => Some(n.to_string()),
        _ => None,
    };
    let mut gutter = div().w(px(48.0)).flex_none().pt(px((line_h - 28.0) / 2.0));
    if let Some(n) = badge {
        gutter = gutter.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.0))
                .rounded_full()
                .bg(if active { p.accent } else { p.raised })
                .font_family(MONO)
                .font_weight(FontWeight::BOLD)
                .text_size(px(12.0))
                .text_color(if active { p.on_accent } else { p.text_2 })
                .child(n),
        );
    }

    // The accent bar sits in the margin so the text never shifts when it appears.
    div()
        .flex()
        .flex_row()
        .w_full()
        .ml(px(-14.0))
        .pl(px(12.0))
        .border_l_2()
        .border_color(marker)
        .child(gutter)
        .child(lines)
}

fn size_button(id: &'static str, label: &'static str, p: Palette) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(40.0))
        .h(px(32.0))
        .rounded_full()
        .cursor_pointer()
        .font_family(SANS)
        .font_weight(FontWeight::BOLD)
        .text_size(px(13.0))
        .text_color(p.text_2)
        .hover(move |s| s.bg(p.hover).text_color(p.text))
        .child(label)
}

fn meta_chip(text: String, p: Palette) -> impl IntoElement {
    div()
        .px(px(12.0))
        .h(px(26.0))
        .flex()
        .items_center()
        .rounded_full()
        .bg(p.raised)
        .font_family(SANS)
        .text_size(px(12.0))
        .text_color(p.text_2)
        .child(text)
}

impl Audible {
    pub(super) fn reader_pane(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let size = self.lyric_size;
        let pane = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded(px(24.0))
            .bg(p.panel)
            .border_1()
            .border_color(p.line)
            .overflow_hidden();

        let Some(h) = self.selected.map(|i| &self.hymns[i]) else {
            return pane
                .items_center()
                .justify_center()
                .font_family(SANS)
                .text_color(p.text_3)
                .child("Select a hymn to begin")
                .into_any_element();
        };

        // Which block is being sung right now (only while a song is running).
        let st = self.audio.as_ref().map(|a| a.status()).unwrap_or_default();
        let following = st.loaded && (st.playing || st.position > 0.01);
        let active_block = if following {
            st.sections
                .get(st.section)
                .and_then(|s| Self::block_for_section(h, s))
        } else {
            None
        };

        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .flex_none()
            .h(px(64.0))
            .px(px(32.0))
            .child({
                let mut left = div().flex().flex_row().items_center().gap(px(10.0));
                if !self.sidebar_open {
                    left = left.child(
                        icon_button("open-library", IconName::PanelLeftOpen, p, p.raised)
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                    );
                }
                left.child(
                    div()
                        .px(px(12.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .bg(p.accent_soft)
                        .font_family(MONO)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .text_color(p.accent)
                        .child(format!("HYMN {}", h.number)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .p(px(3.0))
                    .rounded_full()
                    .bg(p.raised)
                    .child(
                        size_button("smaller", "A\u{2212}", p)
                            .on_click(cx.listener(|this, _, _, cx| this.resize_lyrics(-2.0, cx))),
                    )
                    .child(
                        size_button("larger", "A+", p)
                            .on_click(cx.listener(|this, _, _, cx| this.resize_lyrics(2.0, cx))),
                    ),
            );

        let mut chips = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(8.0))
            .mt(px(14.0));
        let verses = h.verse_count();
        chips = chips.child(meta_chip(
            format!("{verses} {}", if verses == 1 { "verse" } else { "verses" }),
            p,
        ));
        if h.has_chorus() {
            chips = chips.child(meta_chip("Chorus".to_string(), p));
        }
        let parts = h.voice_parts();
        if !parts.is_empty() {
            chips = chips.child(meta_chip(format!("Parts: {}", parts.join(" \u{00B7} ")), p));
        }

        // The scroll area's direct children are: [title header, block 0, block 1, ...].
        // `follow` in app.rs relies on that order.
        let column = |child: AnyElement| {
            div()
                .flex_none()
                .w_full()
                .max_w(px(780.0))
                .px(px(40.0))
                .child(child)
        };
        let mut scroll = div()
            .id("reader")
            .flex()
            .flex_col()
            .items_center()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.reader_scroll)
            .on_scroll(cx.listener(|this, _, _, _| this.pause_follow()))
            .child(column(
                div()
                    .pb(px(36.0))
                    .child(
                        div()
                            .font_family(SANS)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(40.0))
                            .line_height(px(46.0))
                            .text_color(p.text)
                            .child(h.title.clone()),
                    )
                    .child(chips)
                    .into_any_element(),
            ));

        for (i, block) in h.blocks.iter().enumerate() {
            let focus = match active_block {
                None if following => Focus::Dim,
                None => Focus::Normal,
                Some(a) if a == i => Focus::Active(st.section),
                Some(_) => Focus::Dim,
            };
            let view = block_view(block, p, size, matches!(focus, Focus::Active(_)));
            let el: AnyElement = match focus {
                Focus::Normal => view.into_any_element(),
                Focus::Dim => view.opacity(DIMMED).into_any_element(),
                Focus::Active(sec) => view
                    .with_animation(
                        ("block-on", i * 10_000 + sec),
                        Animation::new(Duration::from_millis(380)).with_easing(ease_out_quint()),
                        |el, d| el.opacity(DIMMED + (1.0 - DIMMED) * d),
                    )
                    .into_any_element(),
            };
            scroll = scroll.child(column(el).pb(px(26.0)));
        }
        scroll = scroll.child(div().flex_none().h(px(260.0)));

        // The text fades in whenever a different hymn opens.
        let number = h.number as usize;
        pane.child(toolbar)
            .child(scroll.with_animation(
                ("reader-in", number),
                Animation::new(Duration::from_millis(260)).with_easing(ease_out_quint()),
                |el, d| el.opacity(d),
            ))
            .into_any_element()
    }
}
