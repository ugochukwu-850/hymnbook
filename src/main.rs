mod audio;
mod hymn;
mod tunes_gen;
mod theme;

use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, IconName, Theme, ThemeMode};
use gpui_kit::*;

use audio::{Engine, Instrument, TuneAsset};
use hymn::{Block, BlockKind, Hymn};
use theme::Palette;

const TITLEBAR_HEIGHT: f32 = 52.0;
const LIST_WIDTH: f32 = 380.0;
const ROW_HEIGHT: f32 = 68.0;
const PLAYER_HEIGHT: f32 = 96.0;

const SANS: &str = "Noto Sans";
const SERIF: &str = "Noto Serif";
const MONO: &str = "Noto Sans Mono";

const MIN_LYRIC: f32 = 16.0;
const MAX_LYRIC: f32 = 34.0;

/// The bundled tune for a hymn number, if one exists.
fn tune_for(hymn_no: u32) -> Option<&'static TuneAsset> {
    tunes_gen::TUNES.iter().find(|t| t.hymn == hymn_no)
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

struct Audible {
    hymns: Vec<Hymn>,
    /// Indices into `hymns` that match the current search, best first.
    filtered: Vec<usize>,
    /// Index into `hymns` of the open hymn.
    selected: Option<usize>,
    search: Entity<InputState>,
    list_scroll: UniformListScrollHandle,
    reader_scroll: ScrollHandle,
    focus: FocusHandle,
    pal: Palette,
    lyric_size: f32,
    sidebar_open: bool,
    tunes_only: bool,
    audio: Option<Engine>,
    audio_note: Option<String>,
    /// Last playing flag the UI rendered, so the ticker repaints the stop.
    ui_playing: bool,
}

impl Audible {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hymns = hymn::load();
        let filtered = hymn::search(&hymns, "");

        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search number, title or words")
        });
        cx.subscribe_in(&search, window, |this, state, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let _ = state;
                this.refilter(cx);
            }
        })
        .detach();

        let (audio, audio_note) = match Engine::new() {
            Ok(e) => (Some(e), None),
            Err(e) => {
                eprintln!("audio disabled: {e}");
                (None, Some(e))
            }
        };

        // Repaint ~10x/second while a tune plays (progress bar + clock).
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let alive = this
                    .update(cx, |this, cx| {
                        let playing = this
                            .audio
                            .as_ref()
                            .is_some_and(|a| a.status().playing);
                        if playing || this.ui_playing {
                            this.ui_playing = playing;
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();

        let mut this = Self {
            selected: if hymns.is_empty() { None } else { Some(0) },
            hymns,
            filtered,
            search,
            list_scroll: UniformListScrollHandle::new(),
            reader_scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            pal: Palette::dark(),
            lyric_size: 21.0,
            sidebar_open: true,
            tunes_only: false,
            audio,
            audio_note,
            ui_playing: false,
        };
        this.load_tune();
        this
    }

    fn current_tune(&self) -> Option<&'static TuneAsset> {
        self.selected.and_then(|i| tune_for(self.hymns[i].number))
    }

    /// Load the open hymn's tune into the engine (or unload if it has none).
    fn load_tune(&mut self) {
        let asset = self.current_tune();
        if let Some(engine) = &self.audio {
            if let Err(e) = engine.load(asset) {
                self.audio_note = Some(e);
            }
        }
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value();
        let mut found = hymn::search(&self.hymns, &query);
        if self.tunes_only {
            found.retain(|&i| tune_for(self.hymns[i].number).is_some());
        }
        self.filtered = found;
        self.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn set_tunes_only(&mut self, only: bool, cx: &mut Context<Self>) {
        self.tunes_only = only;
        self.refilter(cx);
    }

    fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if let Some(a) = &self.audio {
            a.toggle();
        }
        self.ui_playing = true;
        cx.notify();
    }

    fn toggle_voice(&mut self, track: usize, cx: &mut Context<Self>) {
        if let Some(a) = &self.audio {
            a.toggle_voice(track);
        }
        cx.notify();
    }

    fn set_instrument(&mut self, inst: Instrument, cx: &mut Context<Self>) {
        if let Some(a) = &mut self.audio {
            a.set_instrument(inst);
        }
        cx.notify();
    }

    fn select(&mut self, hymn_ix: usize, cx: &mut Context<Self>) {
        self.selected = Some(hymn_ix);
        self.reader_scroll.set_offset(point(px(0.0), px(0.0)));
        self.load_tune();
        cx.notify();
    }

    /// Move selection up/down within the filtered list.
    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.filtered.is_empty() {
            return;
        }
        let pos = self
            .selected
            .and_then(|s| self.filtered.iter().position(|&i| i == s));
        let next = match pos {
            Some(p) => (p as isize + delta).clamp(0, self.filtered.len() as isize - 1) as usize,
            None => 0,
        };
        self.list_scroll
            .scroll_to_item(next, ScrollStrategy::Center);
        self.select(self.filtered[next], cx);
    }

    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (pal, mode) = if self.pal.dark {
            (Palette::light(), ThemeMode::Light)
        } else {
            (Palette::dark(), ThemeMode::Dark)
        };
        self.pal = pal;
        Theme::change(mode, Some(window), cx);
        cx.notify();
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        cx.notify();
    }

    fn resize_lyrics(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.lyric_size = (self.lyric_size + delta).clamp(MIN_LYRIC, MAX_LYRIC);
        cx.notify();
    }
}

// ------------------------------------------------------------------ chrome

/// Round icon-only button used in the title bar.
fn icon_button(id: &'static str, icon: IconName, p: Palette, hover: Rgba) -> Stateful<Div> {
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

impl Audible {
    fn titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(30.0))
                    .rounded(px(9.0))
                    .bg(p.accent)
                    .font_family(SANS)
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_size(px(12.0))
                    .text_color(p.on_accent)
                    .child("FT"),
            )
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

        let theme_icon = if p.dark { IconName::Sun } else { IconName::Moon };

        div()
            .id("titlebar")
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .h(px(TITLEBAR_HEIGHT))
            .flex_none()
            .px(px(20.0))
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
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        icon_button("theme", theme_icon, p, p.raised).on_click(cx.listener(
                            |this, _, window, cx| this.toggle_theme(window, cx),
                        )),
                    )
                    .child(div().w(px(8.0)))
                    .child(
                        icon_button("min", IconName::WindowMinimize, p, p.raised)
                            .on_click(|_, window, _| window.minimize_window()),
                    )
                    .child(
                        icon_button("max", IconName::WindowMaximize, p, p.raised)
                            .on_click(|_, window, _| window.zoom_window()),
                    )
                    .child(
                        icon_button("close", IconName::WindowClose, p, close_hover)
                            .on_click(|_, _, cx| cx.quit()),
                    ),
            )
    }
}

// -------------------------------------------------------------------- list

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
    div().h(px(ROW_HEIGHT)).w_full().px(px(10.0)).py(px(3.0)).child(
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
                    .font_weight(FontWeight::SEMIBOLD)
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
                            .font_weight(FontWeight::SEMIBOLD)
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
    fn list_pane(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let count = self.filtered.len();
        let total = self.hymns.len();

        let tune_total = tunes_gen::TUNES.len();
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
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(12.5))
                .bg(if active { p.accent_soft } else { Rgba { a: 0.0, ..p.raised } })
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
                seg("flt-tunes", format!("With tune {tune_total}"), self.tunes_only)
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
                                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
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
                            Input::new(&self.search)
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
                        .font_weight(FontWeight::SEMIBOLD)
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
                                tune_for(this.hymns[hymn_ix].number).is_some(),
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

// ------------------------------------------------------------------ reader

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
        .bg(Rgba { a: 0.18, ..p.accent })
        .font_family(MONO)
        .font_weight(FontWeight::BOLD)
        .text_size(px(10.5))
        .text_color(p.accent)
        .child(tag.to_uppercase())
}

fn block_view(block: &Block, p: Palette, size: f32) -> Div {
    let line_h = (size * 1.62).round();
    let is_chorus = block.kind == BlockKind::Chorus;

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
                .bg(p.raised)
                .font_family(MONO)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(12.0))
                .text_color(p.text_2)
                .child(n),
        );
    }

    div().flex().flex_row().w_full().child(gutter).child(lines)
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
        .font_weight(FontWeight::SEMIBOLD)
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
    fn reader_pane(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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
                .child("Select a hymn to begin");
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

        let mut chips = div().flex().flex_row().flex_wrap().gap(px(8.0)).mt(px(14.0));
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
            chips = chips.child(meta_chip(
                format!("Parts: {}", parts.join(" \u{00B7} ")),
                p,
            ));
        }

        let body = h
            .blocks
            .iter()
            .fold(div().flex().flex_col().gap(px(26.0)).mt(px(36.0)).pb(px(64.0)), |col, b| {
                col.child(block_view(b, p, size))
            });

        pane.child(toolbar).child(
            div()
                .id("reader")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.reader_scroll)
                .child(
                    div().w_full().flex().justify_center().child(
                        div()
                            .w_full()
                            .max_w(px(780.0))
                            .px(px(40.0))
                            .child(
                                div()
                                    .font_family(SANS)
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_size(px(40.0))
                                    .line_height(px(46.0))
                                    .text_color(p.text)
                                    .child(h.title.clone()),
                            )
                            .child(chips)
                            .child(body),
                    ),
                ),
        )
    }
}

// ------------------------------------------------------------------ player

fn time_text(text: String, right: bool, p: Palette) -> impl IntoElement {
    let d = div()
        .w(px(40.0))
        .flex()
        .font_family(MONO)
        .text_size(px(11.0))
        .text_color(p.text_3)
        .child(text);
    if right { d.justify_end() } else { d }
}

fn voice_initial(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()
}

fn pill_button(id: impl Into<ElementId>, label: &'static str, active: bool, p: Palette) -> Stateful<Div> {
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
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(12.0))
        .bg(if active { p.accent_soft } else { Rgba { a: 0.0, ..p.raised } })
        .text_color(if active { p.accent } else { p.text_3 })
        .child(label)
}

impl Audible {
    fn player_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let tune = self.current_tune();
        let st = self.audio.as_ref().map(|a| a.status()).unwrap_or_default();
        let playable = tune.is_some() && self.audio.is_some();
        let (num, title) = match self.selected.map(|i| &self.hymns[i]) {
            Some(h) => (h.number.to_string(), h.title.clone()),
            None => ("\u{2013}".to_string(), "Nothing selected".to_string()),
        };
        let sub = if let Some(err) = self.audio_note.as_ref().filter(|_| self.audio.is_none()) {
            format!("Audio unavailable: {err}")
        } else if tune.is_none() {
            "No tune yet".to_string()
        } else if st.playing {
            "Playing".to_string()
        } else {
            "Ready to play".to_string()
        };

        let left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.0))
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(52.0))
                    .flex_none()
                    .rounded(px(14.0))
                    .bg(p.accent_soft)
                    .font_family(MONO)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(14.0))
                    .text_color(p.accent)
                    .child(num),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(
                        div()
                            .font_family(SANS)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(14.5))
                            .text_color(p.text)
                            .truncate()
                            .child(title),
                    )
                    .child(
                        div()
                            .font_family(SANS)
                            .text_size(px(12.5))
                            .text_color(p.text_3)
                            .child(sub),
                    ),
            );

        // Voice toggles (left of play) and instrument switch (right of play).
        let mut voices = div().flex().flex_row().gap(px(6.0)).w(px(170.0)).justify_end();
        let mut instrument = div().flex().flex_row().gap(px(2.0)).w(px(170.0));
        if let (Some(t), Some(engine)) = (tune, &self.audio) {
            for (i, name) in t.voices.iter().enumerate() {
                let on = engine.voice_enabled(i);
                voices = voices.child(
                    div()
                        .id(("voice", i))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(30.0))
                        .rounded_full()
                        .cursor_pointer()
                        .font_family(MONO)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .bg(if on { p.accent_soft } else { p.raised })
                        .text_color(if on { p.accent } else { p.text_3 })
                        .child(voice_initial(name))
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_voice(i, cx))),
                );
            }
            let cur = engine.instrument();
            instrument = instrument
                .child(
                    pill_button("inst-organ", "Organ", cur == Instrument::Organ, p)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_instrument(Instrument::Organ, cx)
                        })),
                )
                .child(
                    pill_button("inst-flute", "Flute", cur == Instrument::Flute, p)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_instrument(Instrument::Flute, cx)
                        })),
                );
        }

        let (play_bg, play_fg) = if playable {
            (p.accent, p.on_accent)
        } else {
            (p.raised, p.text_3)
        };
        let mut play = div()
            .id("play")
            .flex()
            .items_center()
            .justify_center()
            .size(px(44.0))
            .flex_none()
            .rounded_full()
            .bg(play_bg)
            .child(
                Icon::new(if st.playing { IconName::Pause } else { IconName::Play })
                    .size_5()
                    .text_color(play_fg),
            );
        if playable {
            play = play
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.toggle_play(cx)));
        }

        let frac = if st.duration > 0.0 {
            (st.position / st.duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let center = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .w(px(560.0))
            .flex_none()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(16.0))
                    .child(voices)
                    .child(play)
                    .child(instrument),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .w_full()
                    .child(time_text(clock(st.position), false, p))
                    .child(
                        div().flex_1().h(px(4.0)).rounded_full().bg(p.raised).child(
                            div().h_full().w(relative(frac)).rounded_full().bg(p.accent),
                        ),
                    )
                    .child(time_text(
                        if st.loaded { clock(st.duration) } else { "\u{2013}:\u{2013}\u{2013}".to_string() },
                        true,
                        p,
                    )),
            );

        let right = div().flex().flex_row().justify_end().flex_1().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .h(px(30.0))
                .px(px(14.0))
                .rounded_full()
                .bg(p.raised)
                .font_family(SANS)
                .text_size(px(12.0))
                .text_color(p.text_2)
                .child(div().size(px(6.0)).rounded_full().bg(p.accent))
                .child("Experimental"),
        );

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(24.0))
            .w_full()
            .h(px(PLAYER_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .rounded(px(26.0))
            .bg(p.panel)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .child(left)
            .child(center)
            .child(right)
    }
}

// ------------------------------------------------------------------ window

impl Render for Audible {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        div()
            .id("root")
            .track_focus(&self.focus)
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
                            .gap(px(12.0))
                            .children(self.sidebar_open.then(|| self.list_pane(cx)))
                            .child(self.reader_pane(cx)),
                    )
                    .child(self.player_bar(cx)),
            )
    }
}

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        Theme::change(ThemeMode::Dark, None, cx);

        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
        open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                is_minimizable: true,
                is_movable: true,
                is_resizable: true,
                // We draw our own titlebar (GNOME/Wayland has no server-side decorations).
                window_decorations: Some(WindowDecorations::Client),
                titlebar: Some(TitlebarOptions {
                    title: Some("Hymn Book".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..WindowOptions::default()
            },
            cx,
            |window, cx| cx.new(|cx| Audible::new(window, cx)),
        )
        .expect("Failed to open window");
    });
}
