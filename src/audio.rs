//! Tune playback: MIDI -> software synthesizer (SoundFont) -> sound card.
//!
//! One MIDI track per voice (Soprano/Alto/Tenor/Bass) so each can be muted.
//! The audio callback owns timing: events fire on exact sample boundaries.

use std::io::Cursor;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

/// General MIDI SoundFont (TimGM6mb, GPL-2). See assets/soundfont/README.md.
static SOUNDFONT: &[u8] = include_bytes!("../assets/soundfont/TimGM6mb.sf2");

/// One embedded tune (generated into `tunes_gen.rs`).
pub struct TuneAsset {
    pub hymn: u32,
    pub midi: &'static [u8],
    pub voices: &'static [&'static str],
    #[allow(dead_code)]
    pub source: &'static str,
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

struct Ev {
    time: f64,
    track: usize,
    on: bool,
    key: u8,
    vel: u8,
}

struct Loaded {
    events: Vec<Ev>,
    duration: f64,
}

#[derive(Clone, Copy, Default)]
pub struct Status {
    pub loaded: bool,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
}

struct Shared {
    synth: Synthesizer,
    sample_rate: f64,
    tune: Option<Loaded>,
    pos: f64,
    next: usize,
    playing: bool,
    enabled: [bool; 16],
    left: Vec<f32>,
    right: Vec<f32>,
}

/// Seconds of reverb tail to let ring after the last note.
const TAIL: f64 = 1.2;

impl Shared {
    fn new(rate: u32) -> Result<Self, String> {
        let font = Arc::new(
            SoundFont::new(&mut Cursor::new(SOUNDFONT)).map_err(|e| format!("soundfont: {e:?}"))?,
        );
        let settings = SynthesizerSettings::new(rate as i32);
        let synth = Synthesizer::new(&font, &settings).map_err(|e| format!("synth: {e:?}"))?;
        let mut shared = Shared {
            synth,
            sample_rate: rate as f64,
            tune: None,
            pos: 0.0,
            next: 0,
            playing: false,
            enabled: [true; 16],
            left: vec![0.0; 8192],
            right: vec![0.0; 8192],
        };
        for ch in 0..16 {
            shared.synth.process_midi_message(ch, 0xC0, Instrument::Organ.program(), 0);
        }
        Ok(shared)
    }

    fn fire_due(&mut self) {
        loop {
            let Some(t) = self.tune.as_ref() else { return };
            let Some(e) = t.events.get(self.next) else { return };
            if e.time > self.pos + 1e-9 {
                return;
            }
            let (track, on, key, vel) = (e.track, e.on, e.key, e.vel);
            self.next += 1;
            if !on {
                self.synth.note_off(track as i32, key as i32);
            } else if self.enabled[track.min(15)] {
                self.synth.note_on(track as i32, key as i32, vel as i32);
            }
        }
    }

    fn halt(&mut self, immediate: bool) {
        self.playing = false;
        self.synth.note_off_all(immediate);
    }

    fn fill(&mut self, out: &mut [f32], channels: usize) {
        let frames = out.len() / channels;
        if self.left.len() < frames {
            self.left.resize(frames, 0.0);
            self.right.resize(frames, 0.0);
        }
        let mut done = 0;
        while done < frames {
            let mut chunk = frames - done;
            if self.playing {
                self.fire_due();
                if let Some(t) = &self.tune {
                    if let Some(e) = t.events.get(self.next) {
                        let dt = ((e.time - self.pos) * self.sample_rate).ceil();
                        chunk = chunk.min(dt.max(1.0) as usize);
                    }
                }
            }
            let (l, r) = (
                &mut self.left[done..done + chunk],
                &mut self.right[done..done + chunk],
            );
            self.synth.render(l, r);
            if self.playing {
                self.pos += chunk as f64 / self.sample_rate;
                let finished = self
                    .tune
                    .as_ref()
                    .is_some_and(|t| self.next >= t.events.len() && self.pos >= t.duration + TAIL);
                if finished {
                    self.pos = 0.0;
                    self.next = 0;
                    self.halt(true);
                }
            }
            done += chunk;
        }
        for (i, frame) in out.chunks_mut(channels).enumerate() {
            let (l, r) = (self.left[i], self.right[i]);
            match frame.len() {
                1 => frame[0] = (l + r) * 0.5,
                _ => {
                    frame[0] = l;
                    frame[1] = r;
                    for extra in &mut frame[2..] {
                        *extra = 0.0;
                    }
                }
            }
        }
    }
}

fn parse(midi: &[u8]) -> Result<Loaded, String> {
    let smf = Smf::parse(midi).map_err(|e| e.to_string())?;
    let tpq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as f64,
        Timing::Timecode(..) => return Err("SMPTE timing is not supported".into()),
    };

    // Tempo map across all tracks: (tick, microseconds per quarter).
    let mut tempos: Vec<(u64, f64)> = vec![(0, 500_000.0)];
    for track in &smf.tracks {
        let mut tick = 0u64;
        for ev in track {
            tick += ev.delta.as_int() as u64;
            if let TrackEventKind::Meta(MetaMessage::Tempo(t)) = ev.kind {
                tempos.push((tick, t.as_int() as f64));
            }
        }
    }
    tempos.sort_by_key(|t| t.0);
    let to_secs = |tick: u64| -> f64 {
        let (mut secs, mut last, mut us) = (0.0, 0u64, 500_000.0);
        for &(t, u) in &tempos {
            if t >= tick {
                break;
            }
            secs += (t - last) as f64 * us / tpq / 1e6;
            last = t;
            us = u;
        }
        secs + (tick - last) as f64 * us / tpq / 1e6
    };

    let mut events = Vec::new();
    let mut voice_ix = 0usize;
    for track in &smf.tracks {
        let mut tick = 0u64;
        let mut had_notes = false;
        for ev in track {
            tick += ev.delta.as_int() as u64;
            if let TrackEventKind::Midi { message, .. } = ev.kind {
                let (on, key, vel) = match message {
                    MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                        (true, key.as_int(), vel.as_int())
                    }
                    MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                        (false, key.as_int(), 0)
                    }
                    _ => continue,
                };
                had_notes = true;
                events.push(Ev { time: to_secs(tick), track: voice_ix, on, key, vel });
            }
        }
        if had_notes {
            voice_ix += 1;
        }
    }
    if events.is_empty() {
        return Err("tune has no notes".into());
    }
    // Offs before ons at the same instant so repeated notes re-articulate.
    events.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.on.cmp(&b.on)));
    let duration = events.last().map_or(0.0, |e| e.time);
    Ok(Loaded { events, duration })
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

        let shared = Shared::new(rate)?;
        let instrument = Instrument::Organ;
        let shared = Arc::new(Mutex::new(shared));

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

        Ok(Self { shared, instrument, _stream: stream })
    }

    fn with<R>(&self, f: impl FnOnce(&mut Shared) -> R) -> Option<R> {
        self.shared.lock().ok().map(|mut s| f(&mut s))
    }

    /// Replace the current tune (stops playback). `None` unloads.
    pub fn load(&self, asset: Option<&TuneAsset>) -> Result<(), String> {
        let tune = match asset {
            Some(a) => Some(parse(a.midi)?),
            None => None,
        };
        self.with(|s| {
            s.halt(true);
            s.pos = 0.0;
            s.next = 0;
            s.enabled = [true; 16];
            s.tune = tune;
        });
        Ok(())
    }

    pub fn play(&self) {
        self.with(|s| {
            if s.tune.is_some() {
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

    pub fn status(&self) -> Status {
        self.with(|s| Status {
            loaded: s.tune.is_some(),
            playing: s.playing,
            position: s.pos,
            duration: s.tune.as_ref().map_or(0.0, |t| t.duration),
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
                s.synth.process_midi_message(ch, 0xC0, instrument.program(), 0);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render `secs` of audio through the real `fill` path (no sound card).
    fn render(sh: &mut Shared, secs: f64) -> Vec<f32> {
        let frames = (secs * sh.sample_rate) as usize;
        let mut out = vec![0.0f32; frames * 2];
        for block in out.chunks_mut(1024 * 2) {
            sh.fill(block, 2);
        }
        out
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    fn shared_with(hymn: u32) -> Shared {
        let mut sh = Shared::new(44_100).unwrap();
        let t = crate::tunes_gen::TUNES.iter().find(|t| t.hymn == hymn).unwrap();
        sh.tune = Some(parse(t.midi).unwrap());
        sh.playing = true;
        sh
    }

    #[test]
    fn playing_makes_sound_and_stops_at_the_end() {
        let mut sh = shared_with(457);
        let first = render(&mut sh, 5.0);
        assert!(rms(&first) > 0.01, "silent! rms={}", rms(&first));
        assert!(sh.playing && sh.pos > 4.9);
        // Run past the end (28.8 s + tail): playback must stop and rewind.
        render(&mut sh, 30.0);
        assert!(!sh.playing, "should have stopped");
        assert_eq!(sh.pos, 0.0);
        // After the stop everything decays to silence.
        let after = render(&mut sh, 2.0);
        assert!(rms(&after[after.len() / 2..]) < 0.001);
    }

    #[test]
    fn muting_all_voices_is_silent_and_one_voice_is_quieter() {
        let mut all = shared_with(457);
        let loud = rms(&render(&mut all, 8.0));

        let mut none = shared_with(457);
        none.enabled = [false; 16];
        assert!(rms(&render(&mut none, 8.0)) < 0.0005, "muted voices still sound");

        let mut bass_only = shared_with(457);
        bass_only.enabled = [false; 16];
        bass_only.enabled[3] = true; // Bass is the 4th track
        let one = rms(&render(&mut bass_only, 8.0));
        assert!(one > 0.005 && one < loud, "bass-only rms {one} vs full {loud}");
    }

    #[test]
    fn flute_sounds_different_from_organ() {
        let mut organ = shared_with(457);
        let a = render(&mut organ, 6.0);
        let mut flute = shared_with(457);
        for ch in 0..16 {
            flute.synth.process_midi_message(ch, 0xC0, Instrument::Flute.program(), 0);
        }
        let b = render(&mut flute, 6.0);
        assert!(rms(&b) > 0.005);
        let diff: f32 = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32;
        assert!(diff > 0.005, "instrument change had no effect");
    }

    #[test]
    fn every_embedded_tune_parses_with_expected_voices() {
        for t in crate::tunes_gen::TUNES {
            let l = parse(t.midi).unwrap_or_else(|e| panic!("hymn {}: {e}", t.hymn));
            let tracks: std::collections::BTreeSet<usize> =
                l.events.iter().map(|e| e.track).collect();
            assert_eq!(tracks.len(), t.voices.len(), "hymn {} voice count", t.hymn);
            assert!(l.duration > 5.0 && l.duration < 300.0, "hymn {} duration {}", t.hymn, l.duration);
            // every note on has an off
            let ons = l.events.iter().filter(|e| e.on).count();
            assert_eq!(ons * 2, l.events.len(), "hymn {} unbalanced", t.hymn);
        }
    }

    #[test]
    fn amazing_grace_is_28_8_seconds() {
        let t = crate::tunes_gen::TUNES.iter().find(|t| t.hymn == 457).unwrap();
        let l = parse(t.midi).unwrap();
        // 16 bars of 3/4 at 100 bpm = 28.8 s (last note ends a hair early).
        assert!((l.duration - 28.8).abs() < 0.2, "{}", l.duration);
    }
}
