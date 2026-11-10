//! Application state and behaviour. Drawing lives in `ui/`.

use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

use crate::audio::{Engine, Instrument, Section, SectionKind, TuneAsset};
use crate::hymn::{self, BlockKind, Hymn};
use crate::theme::Palette;
use crate::tunes;

const MIN_LYRIC: f32 = 16.0;
const MAX_LYRIC: f32 = 34.0;

/// How long the splash screen shows before it starts to fade, and how long the fade lasts.
pub const SPLASH_HOLD: Duration = Duration::from_millis(1500);
pub const SPLASH_FADE: Duration = Duration::from_millis(450);
/// Slide time for folding/unfolding the library.
pub const SIDEBAR_SLIDE: Duration = Duration::from_millis(300);

/// Fraction of the reader's height at which the playing verse is kept.
const FOLLOW_LINE: f32 = 0.22;
/// Share of the remaining distance covered each frame (exponential easing).
const FOLLOW_EASE: f32 = 0.14;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Splash {
    Showing,
    Leaving,
    Gone,
}

pub struct Audible {
    pub hymns: Vec<Hymn>,
    /// Indices into `hymns` that match the current search, best first.
    pub filtered: Vec<usize>,
    /// Index into `hymns` of the open hymn.
    pub selected: Option<usize>,
    pub search: Entity<InputState>,
    pub list_scroll: UniformListScrollHandle,
    pub reader_scroll: ScrollHandle,
    pub focus: FocusHandle,
    pub pal: Palette,
    pub lyric_size: f32,
    pub sidebar_open: bool,
    /// Bumped on every fold/unfold so the slide animation restarts.
    pub sidebar_epoch: usize,
    /// True while the slide is running (the panel stays mounted until it ends).
    pub sidebar_animating: bool,
    pub tunes_only: bool,
    pub audio: Option<Engine>,
    pub audio_note: Option<String>,
    pub splash: Splash,
    /// Last playing flag the UI drew, so the ticker repaints the stop.
    ui_playing: bool,
    /// Section the reader last scrolled to, and whether the user took over scrolling.
    last_section: Option<usize>,
    follow_paused: bool,
}

impl Audible {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hymns = hymn::load();
        let filtered = hymn::search(&hymns, "");

        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search number, title or words"));
        cx.subscribe_in(&search, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.refilter(cx);
            }
        })
        .detach();

        // Splash timeline, then start the sound engine once the first frame is
        // on screen (loading the SoundFont takes a moment).
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let _ = this.update(cx, |this, cx| this.start_audio(cx));
            cx.background_executor().timer(SPLASH_HOLD).await;
            let _ = this.update(cx, |this, cx| {
                this.splash = Splash::Leaving;
                cx.notify();
            });
            cx.background_executor().timer(SPLASH_FADE).await;
            let _ = this.update(cx, |this, cx| {
                this.splash = Splash::Gone;
                cx.notify();
            });
        })
        .detach();

        // ~30 fps repaint while a tune plays (song map, clock, follow-along scroll).
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();

        Self {
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
            sidebar_epoch: 0,
            sidebar_animating: false,
            tunes_only: false,
            audio: None,
            audio_note: None,
            splash: Splash::Showing,
            ui_playing: false,
            last_section: None,
            follow_paused: false,
        }
    }

    // ------------------------------------------------------------- audio

    fn start_audio(&mut self, cx: &mut Context<Self>) {
        match Engine::new() {
            Ok(engine) => self.audio = Some(engine),
            Err(e) => {
                eprintln!("audio disabled: {e}");
                self.audio_note = Some(e);
            }
        }
        self.load_tune();
        cx.notify();
    }

    pub fn current_tune(&self) -> Option<&'static TuneAsset> {
        self.selected
            .and_then(|i| tunes::for_hymn(self.hymns[i].number))
    }

    /// Load the open hymn's tune into the engine (or unload if it has none).
    fn load_tune(&mut self) {
        let asset = self.current_tune();
        let verses = self
            .selected
            .map_or(1, |i| self.hymns[i].verse_count() as u32);
        self.last_section = None;
        self.follow_paused = false;
        if let Some(engine) = &self.audio {
            if let Err(e) = engine.load(asset, verses) {
                self.audio_note = Some(e);
            }
        }
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if let Some(a) = &self.audio {
            a.toggle();
        }
        self.ui_playing = true;
        cx.notify();
    }

    pub fn jump_to_section(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(a) = &self.audio {
            a.jump_to_section(index);
        }
        self.follow_paused = false;
        self.last_section = None;
        self.ui_playing = true;
        cx.notify();
    }

    pub fn set_stop_each(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Some(a) = &self.audio {
            a.set_stop_each(on);
        }
        cx.notify();
    }

    pub fn toggle_voice(&mut self, track: usize, cx: &mut Context<Self>) {
        if let Some(a) = &self.audio {
            a.toggle_voice(track);
        }
        cx.notify();
    }

    pub fn set_instrument(&mut self, inst: Instrument, cx: &mut Context<Self>) {
        if let Some(a) = &mut self.audio {
            a.set_instrument(inst);
        }
        cx.notify();
    }

    /// The user scrolled the lyrics by hand: stop auto-following until the next section.
    pub fn pause_follow(&mut self) {
        self.follow_paused = true;
    }

    // --------------------------------------------------------- following

    /// Index of the lyric block (verse or chorus) that a song section sings.
    pub fn block_for_section(hymn: &Hymn, sec: &Section) -> Option<usize> {
        match sec.kind {
            SectionKind::Chorus => hymn.blocks.iter().position(|b| b.kind == BlockKind::Chorus),
            SectionKind::Verse => hymn
                .blocks
                .iter()
                .position(|b| b.kind == BlockKind::Verse(Some(sec.verse)))
                .or_else(|| {
                    if sec.verse == 1 {
                        hymn.blocks
                            .iter()
                            .position(|b| b.kind == BlockKind::Verse(None))
                    } else {
                        None
                    }
                }),
        }
    }

    /// Runs ~30x/second: repaints while playing and eases the lyrics toward the
    /// verse being sung.
    fn tick(&mut self, cx: &mut Context<Self>) {
        let Some(audio) = &self.audio else { return };
        let st = audio.status();
        if !st.playing && !self.ui_playing {
            return;
        }
        self.ui_playing = st.playing;
        if st.playing {
            if self.last_section != Some(st.section) {
                self.last_section = Some(st.section);
                self.follow_paused = false;
            }
            if !self.follow_paused {
                if let (Some(h), Some(sec)) = (
                    self.selected.map(|i| &self.hymns[i]),
                    st.sections.get(st.section),
                ) {
                    if let Some(block) = Self::block_for_section(h, sec) {
                        self.follow(block);
                    }
                }
            }
        }
        cx.notify();
    }

    /// Ease the reader's scroll so lyric block `block` sits near the top.
    /// Child 0 of the scroll area is the title header; blocks follow.
    fn follow(&self, block: usize) {
        let handle = &self.reader_scroll;
        let Some(b) = handle.bounds_for_item(block + 1) else {
            return;
        };
        let view = handle.bounds();
        let want = f32::from(view.origin.y) + f32::from(view.size.height) * FOLLOW_LINE;
        let delta = f32::from(b.origin.y) - want;
        if delta.abs() < 0.5 {
            return;
        }
        let off = handle.offset();
        let max = f32::from(handle.max_offset().y);
        let y = (f32::from(off.y) - delta * FOLLOW_EASE).clamp(-max, 0.0);
        handle.set_offset(point(off.x, px(y)));
    }

    // --------------------------------------------------------- library

    pub fn refilter(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value();
        let mut found = hymn::search(&self.hymns, &query);
        if self.tunes_only {
            found.retain(|&i| tunes::for_hymn(self.hymns[i].number).is_some());
        }
        self.filtered = found;
        self.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    pub fn set_tunes_only(&mut self, only: bool, cx: &mut Context<Self>) {
        self.tunes_only = only;
        self.refilter(cx);
    }

    pub fn select(&mut self, hymn_ix: usize, cx: &mut Context<Self>) {
        self.selected = Some(hymn_ix);
        self.reader_scroll.set_offset(point(px(0.0), px(0.0)));
        self.load_tune();
        cx.notify();
    }

    /// Move selection up/down within the filtered list.
    pub fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
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

    // --------------------------------------------------------- chrome

    pub fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (pal, mode) = if self.pal.dark {
            (Palette::light(), ThemeMode::Light)
        } else {
            (Palette::dark(), ThemeMode::Dark)
        };
        self.pal = pal;
        Theme::change(mode, Some(window), cx);
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        self.sidebar_epoch += 1;
        self.sidebar_animating = true;
        cx.notify();
        let epoch = self.sidebar_epoch;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(SIDEBAR_SLIDE + Duration::from_millis(60))
                .await;
            let _ = this.update(cx, |this, cx| {
                // A newer toggle owns the animation flag now.
                if this.sidebar_epoch == epoch {
                    this.sidebar_animating = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn resize_lyrics(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.lyric_size = (self.lyric_size + delta).clamp(MIN_LYRIC, MAX_LYRIC);
        cx.notify();
    }
}
