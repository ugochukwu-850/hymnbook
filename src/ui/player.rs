//! The player bar: transport, voices, instrument and the clickable song map.

use super::*;
use crate::audio::{Instrument, Section, SectionKind, Status};

const PLAYER_HEIGHT: f32 = 100.0;

fn section_name(sec: &Section) -> String {
    match sec.kind {
        SectionKind::Verse => format!("Verse {}", sec.verse),
        SectionKind::Chorus => "Chorus".to_string(),
    }
}

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
    name.chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default()
}

impl Audible {
    /// One segment per verse/chorus, sized by length. Click a segment to jump there.
    fn song_map(&self, st: &Status, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let mut row = div().flex().flex_row().gap(px(3.0)).flex_1().h(px(26.0));
        if !st.loaded || st.duration <= 0.0 {
            // No tune: just a quiet thin track.
            return row
                .items_center()
                .child(div().flex_1().h(px(4.0)).rounded_full().bg(p.raised));
        }
        let started = st.playing || st.position > 0.01;
        for (i, sec) in st.sections.iter().enumerate() {
            let current = i == st.section && started;
            let done = if i < st.section {
                1.0
            } else if i == st.section {
                ((st.position - sec.start) / sec.len).clamp(0.0, 1.0) as f32
            } else {
                0.0
            };
            let fill = if current {
                p.accent
            } else {
                Rgba {
                    a: 0.30,
                    ..p.accent
                }
            };
            let label = match sec.kind {
                SectionKind::Verse => sec.verse.to_string(),
                SectionKind::Chorus => "C".to_string(),
            };
            row = row.child(
                div()
                    .id(("seg", i))
                    .relative()
                    .h_full()
                    .w(relative((sec.len / st.duration) as f32))
                    .min_w(px(18.0))
                    .rounded(px(8.0))
                    .overflow_hidden()
                    .bg(p.raised)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .h_full()
                            .w(relative(done))
                            .bg(fill),
                    )
                    .child(
                        div()
                            .absolute()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family(MONO)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(10.5))
                            .text_color(if current { p.on_accent } else { p.text_2 })
                            .child(label),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.jump_to_section(i, cx))),
            );
        }
        row
    }

    pub(super) fn player_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
        } else if let Some(sec) = st.sections.get(st.section) {
            if st.playing {
                format!("Playing \u{00B7} {}", section_name(sec))
            } else if st.position < 0.01 {
                "Ready to play".to_string()
            } else if (st.position - sec.start).abs() < 0.05 {
                format!("Up next \u{00B7} {}", section_name(sec))
            } else {
                format!("Paused \u{00B7} {}", section_name(sec))
            }
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
                            .font_weight(FontWeight::BOLD)
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
        let mut voices = div()
            .flex()
            .flex_row()
            .gap(px(6.0))
            .w(px(170.0))
            .justify_end();
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
                    pill_button("inst-organ", "Organ", cur == Instrument::Organ, p).on_click(
                        cx.listener(|this, _, _, cx| this.set_instrument(Instrument::Organ, cx)),
                    ),
                )
                .child(
                    pill_button("inst-flute", "Flute", cur == Instrument::Flute, p).on_click(
                        cx.listener(|this, _, _, cx| this.set_instrument(Instrument::Flute, cx)),
                    ),
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
                Icon::new(if st.playing {
                    IconName::Pause
                } else {
                    IconName::Play
                })
                .size_5()
                .text_color(play_fg),
            );
        if playable {
            play = play
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.toggle_play(cx)));
        }

        let center = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10.0))
            .w(px(600.0))
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
                    .child(self.song_map(&st, cx))
                    .child(time_text(
                        if st.loaded {
                            clock(st.duration)
                        } else {
                            "\u{2013}:\u{2013}\u{2013}".into()
                        },
                        true,
                        p,
                    )),
            );

        // Right: the "pause after each verse" switch (when a tune exists) + status chip.
        let mut right = div()
            .flex()
            .flex_col()
            .items_end()
            .justify_center()
            .gap(px(8.0))
            .flex_1();
        if playable {
            let on = st.stop_each;
            right = right.child(
                div()
                    .id("stop-each")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .h(px(30.0))
                    .px(px(12.0))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(if on { p.accent_soft } else { p.raised })
                    .font_family(SANS)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(12.0))
                    .text_color(if on { p.accent } else { p.text_2 })
                    .hover(move |s| s.bg(if on { p.accent_soft } else { p.hover }))
                    .child(
                        div()
                            .size(px(8.0))
                            .rounded_full()
                            .border_1()
                            .border_color(if on { p.accent } else { p.text_3 })
                            .bg(if on {
                                p.accent
                            } else {
                                Rgba { a: 0.0, ..p.accent }
                            }),
                    )
                    .child("Pause after each verse")
                    .on_click(cx.listener(move |this, _, _, cx| this.set_stop_each(!on, cx))),
            );
        }
        right = right.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .h(px(24.0))
                .px(px(12.0))
                .rounded_full()
                .font_family(SANS)
                .text_size(px(11.0))
                .text_color(p.text_3)
                .child(div().size(px(6.0)).rounded_full().bg(p.accent))
                .child("Experimental tune"),
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
