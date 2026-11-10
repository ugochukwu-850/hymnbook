//! The library panel: search, filter, and the hymn list.

use super::*;
use crate::hymn::Hymn;
use crate::tunes;

const LIST_WIDTH: f32 = 380.0;
const ROW_HEIGHT: f32 = 68.0;

fn hymn_row(
    h: &Hymn,
    selected: bool,
    ix: usize,
    p: Palette,
    has_tune: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let (tile_bg, tile_fg) = if selected {
        (p.accent, p.on_accent)
    } else {
        (p.raised, p.text_2)
    };
    let row_bg = if selected {
        p.accent_soft
    } else {
        Rgba { a: 0.0, ..p.panel }
    };
    let hover_bg = if selected { p.accent_soft } else { p.hover };

    // Outer box has the fixed height uniform_list measures; the inner rounded
    // box is inset so rows float with a gap between them.
    div()
        .h(px(ROW_HEIGHT))
        .w_full()
        .px(px(10.0))
        .py(px(3.0))
        .child(
            div()
                .id(("row", ix))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(14.0))
                .size_full()
                .px(px(12.0))
                .rounded(px(14.0))
                .bg(row_bg)
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(on_click)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(42.0))
                        .flex_none()
                        .rounded(px(12.0))
                        .bg(tile_bg)
                        .font_family(MONO)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(13.0))
                        .text_color(tile_fg)
                        .child(h.number.to_string()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(
                            div()
                                .font_family(SANS)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(14.5))
                                .text_color(p.text)
                                .truncate()
                                .child(h.title.clone()),
                        )
                        .child(
                            div()
                                .font_family(SANS)
                                .text_size(px(12.5))
                                .text_color(p.text_3)
                                .truncate()
                                .child(h.first_line().to_string()),
                        ),
                )
                .children(has_tune.then(|| {
                    div()
                        .flex_none()
                        .px(px(8.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .bg(if selected { p.accent } else { p.accent_soft })
                        .font_family(MONO)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(9.5))
                        .text_color(if selected { p.on_accent } else { p.accent })
                        .child("TUNE")
                })),
        )
}

impl Audible {
    /// The library column, wrapped so it can slide open and shut. The slot's width
    /// includes the 12px gap to the reader, so nothing jumps when it unmounts.
    pub(super) fn sidebar_slot(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.sidebar_open && !self.sidebar_animating {
            return None;
        }
        let full = LIST_WIDTH + 12.0;
        let slot = div()
            .h_full()
            .flex_none()
            .overflow_hidden()
            .w(px(full))
            .child(
                div()
                    .w(px(LIST_WIDTH))
                    .h_full()
                    .flex_none()
                    .mr(px(12.0))
                    .child(self.list_pane(cx)),
            );
        if self.sidebar_epoch == 0 {
            return Some(slot.into_any_element());
        }
        let opening = self.sidebar_open;
        Some(
            slot.with_animation(
                ("sidebar", self.sidebar_epoch),
                Animation::new(crate::app::SIDEBAR_SLIDE).with_easing(ease_in_out),
                move |el, d| {
                    let f = if opening { d } else { 1.0 - d };
                    el.w(px(full * f)).opacity(f)
                },
            )
            .into_any_element(),
        )
    }

    pub(super) fn list_pane(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let count = self.filtered.len();
        let total = self.hymns.len();
        let tune_total = tunes::TUNES.len();

        let seg = |id: &'static str, label: String, active: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .flex_1()
                .h(px(30.0))
                .rounded_full()
                .cursor_pointer()
                .font_family(SANS)
                .font_weight(FontWeight::BOLD)
                .text_size(px(12.5))
                .bg(if active {
                    p.accent_soft
                } else {
                    Rgba { a: 0.0, ..p.raised }
                })
                .text_color(if active { p.accent } else { p.text_2 })
                .child(label)
        };
        let filter_row = div()
            .flex()
            .flex_row()
            .gap(px(4.0))
            .p(px(3.0))
            .rounded_full()
            .bg(p.raised)
            .child(
                seg("flt-all", format!("All {total}"), !self.tunes_only)
                    .on_click(cx.listener(|this, _, _, cx| this.set_tunes_only(false, cx))),
            )
            .child(
                seg(
                    "flt-tunes",
                    format!("With tune {tune_total}"),
                    self.tunes_only,
                )
                .on_click(cx.listener(|this, _, _, cx| this.set_tunes_only(true, cx))),
            );

        let header = div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .flex_none()
            .px(px(20.0))
            .pt(px(20.0))
            .pb(px(10.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_family(SANS)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(22.0))
                            .text_color(p.text)
                            .child("Library"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .font_family(SANS)
                                    .text_size(px(12.0))
                                    .text_color(p.text_3)
                                    .child(if count == total {
                                        format!("{total} hymns")
                                    } else {
                                        format!("{count} of {total}")
                                    }),
                            )
                            .child(
                                icon_button("fold-library", IconName::PanelLeftClose, p, p.raised)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.toggle_sidebar(cx)),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(46.0))
                    .pl(px(16.0))
                    .pr(px(8.0))
                    .rounded_full()
                    .bg(p.raised)
                    .child(Icon::new(IconName::Search).size_4().text_color(p.text_3))
                    .child(
                        div().flex_1().child(
                            gpui_kit::component::input::Input::new(&self.search)
                                .appearance(false)
                                .cleanable(true),
                        ),
                    ),
            )
            .child(filter_row);

        let body = if count == 0 {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .child(
                    div()
                        .font_family(SANS)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(15.0))
                        .text_color(p.text_2)
                        .child("No hymns found"),
                )
                .child(
                    div()
                        .font_family(SANS)
                        .text_size(px(13.0))
                        .text_color(p.text_3)
                        .child("Try a number, a title, or a line you remember."),
                )
                .into_any_element()
        } else {
            uniform_list(
                "hymn-list",
                count,
                cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                    let p = this.pal;
                    range
                        .map(|row| {
                            let hymn_ix = this.filtered[row];
                            let selected = this.selected == Some(hymn_ix);
                            hymn_row(
                                &this.hymns[hymn_ix],
                                selected,
                                hymn_ix,
                                p,
                                tunes::for_hymn(this.hymns[hymn_ix].number).is_some(),
                                cx.listener(move |this, _, _, cx| this.select(hymn_ix, cx)),
                            )
                        })
                        .collect()
                }),
            )
            .track_scroll(&self.list_scroll)
            .flex_1()
            .pb(px(8.0))
            .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .w(px(LIST_WIDTH))
            .flex_none()
            .h_full()
            .rounded(px(24.0))
            .bg(p.panel)
            .border_1()
            .border_color(p.line)
            .overflow_hidden()
            .child(header)
            .child(body)
    }
}
