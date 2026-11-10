//! The synthesizer and its audio-thread timing.

use std::io::Cursor;
use std::sync::Arc;

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use super::{Instrument, Section};

/// General MIDI SoundFont (TimGM6mb, GPL-2). See assets/soundfont/README.md.
static SOUNDFONT: &[u8] = include_bytes!("../../assets/soundfont/TimGM6mb.sf2");

pub(super) struct Ev {
    pub time: f64,
    pub track: usize,
    pub on: bool,
    pub key: u8,
    pub vel: u8,
}

pub(super) struct Shared {
    pub synth: Synthesizer,
    pub sample_rate: f64,
    pub events: Vec<Ev>,
    pub sections: Vec<Section>,
    /// Timeline positions where "stop after each verse" pauses.
    pub stops: Vec<f64>,
    pub total: f64,
    pub loaded: bool,
    pub stop_each: bool,
    pub pos: f64,
    pub next: usize,
    pub playing: bool,
    pub enabled: [bool; 16],
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

/// Seconds of reverb tail to let ring after the last note.
const TAIL: f64 = 1.2;
pub(super) const EPS: f64 = 1e-6;

impl Shared {
    pub fn new(rate: u32) -> Result<Self, String> {
        let font = Arc::new(
            SoundFont::new(&mut Cursor::new(SOUNDFONT)).map_err(|e| format!("soundfont: {e:?}"))?,
        );
        let settings = SynthesizerSettings::new(rate as i32);
        let synth = Synthesizer::new(&font, &settings).map_err(|e| format!("synth: {e:?}"))?;
        let mut shared = Shared {
            synth,
            sample_rate: rate as f64,
            events: Vec::new(),
            sections: Vec::new(),
            stops: Vec::new(),
            total: 0.0,
            loaded: false,
            stop_each: false,
            pos: 0.0,
            next: 0,
            playing: false,
            enabled: [true; 16],
            left: vec![0.0; 8192],
            right: vec![0.0; 8192],
        };
        for ch in 0..16 {
            shared
                .synth
                .process_midi_message(ch, 0xC0, Instrument::Organ.program(), 0);
        }
        Ok(shared)
    }

    pub fn fire_due(&mut self) {
        while let Some(e) = self.events.get(self.next) {
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

    pub fn halt(&mut self, immediate: bool) {
        self.playing = false;
        self.synth.note_off_all(immediate);
    }

    pub fn seek(&mut self, secs: f64) {
        self.pos = secs.clamp(0.0, self.total);
        let p = self.pos;
        self.next = self.events.partition_point(|e| e.time < p - 1e-9);
        self.synth.note_off_all(true);
    }

    /// The next pause point after the current position, if pausing is on.
    pub fn next_stop(&self) -> Option<f64> {
        if !self.stop_each {
            return None;
        }
        self.stops.iter().copied().find(|&t| t > self.pos + 1e-9)
    }

    pub fn fill(&mut self, out: &mut [f32], channels: usize) {
        let frames = out.len() / channels;
        if self.left.len() < frames {
            self.left.resize(frames, 0.0);
            self.right.resize(frames, 0.0);
        }
        let mut done = 0;
        while done < frames {
            let mut chunk = frames - done;
            let mut stop = None;
            if self.playing {
                self.fire_due();
                if let Some(e) = self.events.get(self.next) {
                    let dt = ((e.time - self.pos) * self.sample_rate).ceil();
                    chunk = chunk.min(dt.max(1.0) as usize);
                }
                stop = self.next_stop();
                if let Some(s) = stop {
                    let dt = ((s - self.pos) * self.sample_rate).ceil();
                    chunk = chunk.min(dt.max(1.0) as usize);
                }
            }
            let (l, r) = (
                &mut self.left[done..done + chunk],
                &mut self.right[done..done + chunk],
            );
            self.synth.render(l, r);
            if self.playing {
                self.pos += chunk as f64 / self.sample_rate;
                if let Some(s) = stop.filter(|&s| self.pos >= s - EPS) {
                    // Pause exactly on the section boundary; resuming continues
                    // with the next verse.
                    self.pos = s;
                    self.next = self.events.partition_point(|e| e.time < s - 1e-9);
                    self.halt(false);
                } else if self.next >= self.events.len() && self.pos >= self.total + TAIL {
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
