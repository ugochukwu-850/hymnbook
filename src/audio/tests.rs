use super::arrange::arrange;
use super::midi::parse_notes;
use super::synth::{EPS, Shared};
use super::*;

fn asset(hymn: u32) -> &'static TuneAsset {
    crate::tunes::TUNES.iter().find(|t| t.hymn == hymn).unwrap()
}

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

fn shared_with(hymn: u32, verses: u32) -> Shared {
    let mut sh = Shared::new(44_100).unwrap();
    let a = asset(hymn);
    let arr = arrange(&parse_notes(a.midi).unwrap(), a.length, a.chorus_at, verses);
    sh.events = arr.events;
    sh.sections = arr.sections;
    sh.stops = arr.stops;
    sh.total = arr.total;
    sh.loaded = true;
    sh.playing = true;
    sh
}

#[test]
fn every_embedded_tune_parses_with_expected_voices() {
    for t in crate::tunes::TUNES {
        let notes = parse_notes(t.midi).unwrap_or_else(|e| panic!("hymn {}: {e}", t.hymn));
        let tracks: std::collections::BTreeSet<usize> = notes.iter().map(|n| n.track).collect();
        assert_eq!(tracks.len(), t.voices.len(), "hymn {} voice count", t.hymn);
        assert!(
            t.length > 5.0 && t.length < 300.0,
            "hymn {} length {}",
            t.hymn,
            t.length
        );
        if let Some(c) = t.chorus_at {
            assert!(
                c > 3.0 && c < t.length - 3.0,
                "hymn {} chorus_at {c}",
                t.hymn
            );
        }
        // Only notes that start inside the (possibly trimmed) length are ever played.
        let last_end = notes
            .iter()
            .filter(|n| n.start < t.length - 1e-6)
            .map(|n| n.end)
            .fold(0.0, f64::max);
        assert!(
            last_end <= t.length + 1.0,
            "hymn {} notes run past its length",
            t.hymn
        );
    }
}

#[test]
fn verse_only_tune_repeats_once_per_verse() {
    let a = asset(457); // Amazing Grace, no chorus
    let arr = arrange(&parse_notes(a.midi).unwrap(), a.length, a.chorus_at, 4);
    assert_eq!(arr.sections.len(), 4);
    assert!(arr.sections.iter().all(|s| s.kind == SectionKind::Verse));
    assert!((arr.total - 4.0 * a.length).abs() < 1e-6);
    assert_eq!(
        arr.stops.len(),
        3,
        "pause after verses 1-3, not after the last"
    );
}

#[test]
fn verse_and_chorus_alternate() {
    let a = asset(590); // The Old Rugged Cross, chorus detected
    assert!(a.chorus_at.is_some());
    let arr = arrange(&parse_notes(a.midi).unwrap(), a.length, a.chorus_at, 4);
    let kinds: Vec<_> = arr.sections.iter().map(|s| (s.kind, s.verse)).collect();
    assert_eq!(kinds.len(), 8);
    for v in 0..4 {
        assert_eq!(kinds[2 * v], (SectionKind::Verse, v as u32 + 1));
        assert_eq!(kinds[2 * v + 1], (SectionKind::Chorus, v as u32 + 1));
    }
    // sections are contiguous: no gaps, no overlaps
    for w in arr.sections.windows(2) {
        assert!((w[0].start + w[0].len - w[1].start).abs() < 1e-9);
    }
    assert!((arr.total - 4.0 * a.length).abs() < 1e-6);
    assert_eq!(arr.stops.len(), 3);
    // pauses land at the end of each chorus
    assert!((arr.stops[0] - (arr.sections[1].start + arr.sections[1].len)).abs() < 1e-9);
}

#[test]
fn every_note_is_in_exactly_one_section_per_verse() {
    for t in crate::tunes::TUNES {
        let notes = parse_notes(t.midi).unwrap();
        let arr = arrange(&notes, t.length, t.chorus_at, 3);
        let ons = arr.events.iter().filter(|e| e.on).count();
        let offs = arr.events.len() - ons;
        assert_eq!(ons, offs, "hymn {}", t.hymn);
        let one_pass = notes.iter().filter(|n| n.start < t.length - EPS).count();
        assert_eq!(
            ons,
            one_pass * 3,
            "hymn {} lost or duplicated notes",
            t.hymn
        );
    }
}

#[test]
fn playing_makes_sound_and_stops_at_the_end() {
    let mut sh = shared_with(457, 1);
    let first = render(&mut sh, 5.0);
    assert!(rms(&first) > 0.01, "silent! rms={}", rms(&first));
    assert!(sh.playing && sh.pos > 4.9);
    render(&mut sh, 30.0);
    assert!(!sh.playing, "should have stopped");
    assert_eq!(sh.pos, 0.0);
    let after = render(&mut sh, 2.0);
    assert!(rms(&after[after.len() / 2..]) < 0.001);
}

#[test]
fn whole_hymn_plays_through_every_verse_by_default() {
    let a = asset(457);
    let mut sh = shared_with(457, 3);
    render(&mut sh, a.length * 2.0 + 2.0); // well into verse 3
    assert!(sh.playing, "must not stop between verses by default");
    let loud = rms(&render(&mut sh, 4.0));
    assert!(loud > 0.01, "verse 3 is silent");
}

#[test]
fn stop_after_each_verse_pauses_then_resumes_with_next_verse() {
    let a = asset(590);
    let mut sh = shared_with(590, 3);
    sh.stop_each = true;
    let first_stop = sh.stops[0];
    render(&mut sh, first_stop + 0.5);
    assert!(!sh.playing, "should have paused after verse 1 + chorus");
    assert!(
        (sh.pos - first_stop).abs() < 1e-6,
        "paused at {} not {first_stop}",
        sh.pos
    );
    // pressing play carries on with verse 2 and stops after it
    sh.playing = true;
    let second_stop = sh.stops[1];
    render(&mut sh, (second_stop - first_stop) + 0.5);
    assert!(!sh.playing);
    assert!((sh.pos - second_stop).abs() < 1e-6);
    assert!((second_stop - first_stop - a.length).abs() < 1e-6);
}

#[test]
fn jumping_to_a_section_starts_there() {
    let mut sh = shared_with(590, 4);
    let target = sh.sections[4]; // verse 3
    sh.seek(target.start);
    sh.playing = true;
    let out = render(&mut sh, 3.0);
    assert!(rms(&out) > 0.01, "silent after jump");
    assert!(sh.pos > target.start + 2.9);
    let idx = sh
        .sections
        .iter()
        .rposition(|x| x.start <= sh.pos + 1e-9)
        .unwrap();
    assert_eq!(idx, 4);
}

#[test]
fn muting_all_voices_is_silent_and_one_voice_is_quieter() {
    let mut all = shared_with(457, 1);
    let loud = rms(&render(&mut all, 8.0));

    let mut none = shared_with(457, 1);
    none.enabled = [false; 16];
    assert!(
        rms(&render(&mut none, 8.0)) < 0.0005,
        "muted voices still sound"
    );

    let mut bass_only = shared_with(457, 1);
    bass_only.enabled = [false; 16];
    bass_only.enabled[3] = true; // Bass is the 4th track
    let one = rms(&render(&mut bass_only, 8.0));
    assert!(
        one > 0.005 && one < loud,
        "bass-only rms {one} vs full {loud}"
    );
}

#[test]
fn flute_sounds_different_from_organ() {
    let mut organ = shared_with(457, 1);
    let a = render(&mut organ, 6.0);
    let mut flute = shared_with(457, 1);
    for ch in 0..16 {
        flute
            .synth
            .process_midi_message(ch, 0xC0, Instrument::Flute.program(), 0);
    }
    let b = render(&mut flute, 6.0);
    assert!(rms(&b) > 0.005);
    let diff: f32 = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32;
    assert!(diff > 0.005, "instrument change had no effect");
}

/// Not a real test: renders a hymn through the real engine to a 16-bit WAV, used for
/// the website's audio sample. Run with:
///   HYMN_SAMPLE_OUT=out.wav HYMN_SAMPLE_HYMN=457 HYMN_SAMPLE_VERSES=1 \
///   cargo test render_website_sample -- --ignored
#[test]
#[ignore]
fn render_website_sample() {
    let Ok(out) = std::env::var("HYMN_SAMPLE_OUT") else { return };
    let get = |k: &str, d: u32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let mut sh = shared_with(get("HYMN_SAMPLE_HYMN", 457), get("HYMN_SAMPLE_VERSES", 1));
    let secs = sh.total + 2.5;
    let samples = render(&mut sh, secs);

    let rate = 44_100u32;
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&2u16.to_le_bytes()); // stereo
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 4).to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(out, wav).unwrap();
}
