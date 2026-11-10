//! Tune playback: MIDI -> software synthesizer (SoundFont) -> sound card.
//!
//! A tune file holds ONE pass of the music (the verse, then the chorus if the
//! hymn has one). Hymns are sung as verse 1, chorus, verse 2, chorus, ... so the
//! engine *arranges* the notes into that full sequence. Sections are joined on
//! exact sample boundaries (no gaps or clicks), and playback can jump to any
//! section or pause after each verse.
//!
//! One MIDI track per voice (Soprano/Alto/Tenor/Bass) so each can be muted.
//! The audio callback owns timing: events fire on exact sample boundaries.

mod arrange;
mod midi;
mod synth;
#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use arrange::arrange;
use midi::parse_notes;
use synth::Shared;

/// One embedded tune (generated into `tunes_gen.rs`).
pub struct TuneAsset {
    pub hymn: u32,
    pub midi: &'static [u8],
    pub voices: &'static [&'static str],
    #[allow(dead_code)]
    pub source: &'static str,
    /// Length of one pass of the music in seconds (including final rests).
    pub length: f64,
    /// Where the chorus music starts within the pass, if the tune has one.
    pub chorus_at: Option<f64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Instrument {
    Organ,
    Flute,
}

impl Instrument {
    /// General MIDI program number (0-based).
    fn program(self) -> i32 {
        match self {
            Instrument::Organ => 19, // Church Organ
            Instrument::Flute => 73, // Flute
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SectionKind {
    Verse,
    Chorus,
}

/// One stretch of the arranged song.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub kind: SectionKind,
    /// Verse number this section belongs to (1-based).
    pub verse: u32,
    /// Start on the song timeline, seconds.
    pub start: f64,
    pub len: f64,
}

#[derive(Clone, Default)]
pub struct Status {
    pub loaded: bool,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    /// Index into `sections` of the section playing now.
    pub section: usize,
    pub sections: Vec<Section>,
    pub stop_each: bool,
}

pub struct Engine {
    shared: Arc<Mutex<Shared>>,
    instrument: Instrument,
    _stream: cpal::Stream,
}

impl Engine {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no audio output device found")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!(
                "unsupported audio sample format {:?}",
                supported.sample_format()
            ));
        }
        let config = supported.config();
        let (rate, channels) = (config.sample_rate, config.channels as usize);

        let shared = Arc::new(Mutex::new(Shared::new(rate)?));
        let instrument = Instrument::Organ;

        let cb = Arc::clone(&shared);
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _| match cb.lock() {
                    Ok(mut s) => s.fill(data, channels),
                    Err(_) => data.fill(0.0),
                },
                |e| eprintln!("audio stream error: {e}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;

        Ok(Self {
            shared,
            instrument,
            _stream: stream,
        })
    }

    fn with<R>(&self, f: impl FnOnce(&mut Shared) -> R) -> Option<R> {
        self.shared.lock().ok().map(|mut s| f(&mut s))
    }

    /// Replace the current tune (stops playback). `None` unloads.
    /// `verses` is how many verses the hymn has; the song is arranged to match.
    pub fn load(&self, asset: Option<&TuneAsset>, verses: u32) -> Result<(), String> {
        let arranged = match asset {
            Some(a) => Some(arrange(
                &parse_notes(a.midi)?,
                a.length,
                a.chorus_at,
                verses,
            )),
            None => None,
        };
        self.with(|s| {
            s.halt(true);
            s.pos = 0.0;
            s.next = 0;
            s.enabled = [true; 16];
            match arranged {
                Some(a) => {
                    s.events = a.events;
                    s.sections = a.sections;
                    s.stops = a.stops;
                    s.total = a.total;
                    s.loaded = true;
                }
                None => {
                    s.events.clear();
                    s.sections.clear();
                    s.stops.clear();
                    s.total = 0.0;
                    s.loaded = false;
                }
            }
        });
        Ok(())
    }

    pub fn play(&self) {
        self.with(|s| {
            if s.loaded {
                s.playing = true;
            }
        });
    }

    pub fn pause(&self) {
        self.with(|s| s.halt(false));
    }

    pub fn toggle(&self) {
        if self.status().playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Jump to the start of a section and play from there.
    pub fn jump_to_section(&self, index: usize) {
        self.with(|s| {
            if let Some(sec) = s.sections.get(index).copied() {
                s.seek(sec.start);
                s.playing = true;
            }
        });
    }

    pub fn set_stop_each(&self, on: bool) {
        self.with(|s| s.stop_each = on);
    }

    pub fn status(&self) -> Status {
        self.with(|s| Status {
            loaded: s.loaded,
            playing: s.playing,
            position: s.pos,
            duration: s.total,
            section: s
                .sections
                .iter()
                .rposition(|x| x.start <= s.pos + 1e-9)
                .unwrap_or(0),
            sections: s.sections.clone(),
            stop_each: s.stop_each,
        })
        .unwrap_or_default()
    }

    pub fn voice_enabled(&self, track: usize) -> bool {
        self.with(|s| s.enabled[track.min(15)]).unwrap_or(true)
    }

    pub fn toggle_voice(&self, track: usize) {
        let track = track.min(15);
        self.with(|s| {
            s.enabled[track] = !s.enabled[track];
            if !s.enabled[track] {
                s.synth.note_off_all_channel(track as i32, false);
            }
        });
    }

    pub fn instrument(&self) -> Instrument {
        self.instrument
    }

    pub fn set_instrument(&mut self, instrument: Instrument) {
        self.instrument = instrument;
        self.with(|s| {
            for ch in 0..16 {
                s.synth
                    .process_midi_message(ch, 0xC0, instrument.program(), 0);
            }
        });
    }
}
