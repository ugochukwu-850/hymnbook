//! Reading notes out of the bundled MIDI files.

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

pub(super) struct Note {
    pub start: f64,
    pub end: f64,
    pub track: usize,
    pub key: u8,
    pub vel: u8,
}

/// Read every note (start, end) of the MIDI file, in seconds.
pub(super) fn parse_notes(midi: &[u8]) -> Result<Vec<Note>, String> {
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

    let mut notes = Vec::new();
    let mut voice_ix = 0usize;
    for track in &smf.tracks {
        let mut tick = 0u64;
        let mut open: Vec<(u8, f64, u8)> = Vec::new(); // (key, start, vel)
        let before = notes.len();
        for ev in track {
            tick += ev.delta.as_int() as u64;
            let TrackEventKind::Midi { message, .. } = ev.kind else {
                continue;
            };
            let t = to_secs(tick);
            match message {
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                    open.push((key.as_int(), t, vel.as_int()));
                }
                MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                    if let Some(i) = open.iter().position(|o| o.0 == key.as_int()) {
                        let (k, start, vel) = open.remove(i);
                        notes.push(Note {
                            start,
                            end: t,
                            track: voice_ix,
                            key: k,
                            vel,
                        });
                    }
                }
                _ => {}
            }
        }
        for (k, start, vel) in open {
            notes.push(Note {
                start,
                end: start + 0.5,
                track: voice_ix,
                key: k,
                vel,
            });
        }
        if notes.len() > before {
            voice_ix += 1;
        }
    }
    if notes.is_empty() {
        return Err("tune has no notes".into());
    }
    Ok(notes)
}
