# Audible: Faith Tabernacle Hymn Book (desktop)

## Purpose
Members have forgotten how the hymns sound. Desktop app (Rust + GPUI) to browse
all 1000 hymns, search them, read lyrics, and (Phase 2) hear the tune.

## Data
- Source: `data/hymns.ndjson`, extracted from the Android hymn book APK v1.0.9.
  1000 rows: `hymn_no`, `hymn_title`, `body`. Embedded at compile time.
- Body keeps `\r\n`, numbered verses (`2.`), `Chorus:` blocks, voice tags
  (`Sop:`, `ATB:`, `TB:`, `SA:`, `Gen:`).
- No audio/notation in the APK. Tunes come from public-domain MIDI/notation
  matched by title/first line (Phase 2). Audio is labeled **Experimental**:
  the public tune may differ from the church's arrangement.

## Design language: "Swiss Paper"
- Off-white page, near-black ink, ONE accent (deep indigo). Dark theme later.
- 1px hard borders, square corners, no gradients/blur/shadows.
- Hymn numbers in tabular monospace. Lyrics serif; UI sans.
- Layout: left list (search + rows) | lyrics reader | bottom player bar.

## Phase 1 (build now)
- Parse + index hymns. Search by number, title, lyric text (case-insensitive).
- List with selection, reader with parsed verses/chorus/voice-tag chips.
- Player bar present but stubbed: "No tune yet" state.
- Favorites deferred.

## Phase 2 (later)
- Match hymns to PD tunes, MIDI -> synth (rustysynth + rodio, flute/organ).
- Per-voice mute/solo (S/A/T/B) where the source has parts.
- Experimental badge on every generated tune.

## Testing
- Unit tests: parser (verse/chorus/tag splitting), search ranking.
- Manual: launch, search "hail", "1", select, scroll.
