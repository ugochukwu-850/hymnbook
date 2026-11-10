# Contributing

Thank you for helping! This is a hymn book for singers who have forgotten how
the tunes go, so the most valuable contributions are **correct tunes**.

## Ways to help

- **Report a wrong tune or a wrong word** – open an issue with the hymn number.
- **Add a tune** – see "Adding tunes" below. Only add tunes you may share (public
  domain, or your own with permission). Say where it came from.
- **Fix a bug / improve the UI** – pull requests are welcome.
- **Translate or localise** – open an issue first so we can agree on the approach.

## Build

```bash
# Linux needs: libasound2-dev libxkbcommon-dev libwayland-dev libx11-xcb-dev \
#   libxkbcommon-x11-dev libfontconfig-dev libvulkan-dev
cargo run --release
cargo test
```

## Code layout

```
src/main.rs        window + fonts
src/app.rs         state and behaviour (no drawing)
src/ui/            one file per screen region (library, reader, player, ...)
src/audio/         synthesizer, MIDI reading, verse/chorus arrangement
src/hymn.rs        hymn data: parsing lyrics, search
src/tunes/         bundled tunes (generated table)
tools/             icon + tune conversion scripts
```

## Adding tunes

1. Put the ABC scores in a folder and list your matches as
   `data/tune_matches.json` (`[{"no": 12, "title": "...", "file": "path.abc"}]`).
2. Run `python3 tools/abc2mid.py data/tune_matches.json <abc-dir> assets/tunes src/tunes/generated.rs`
   (needs `pip install mido`).
3. `cargo test` checks that every tune parses and has balanced notes.

Please check by ear that the tune matches the hymn before opening a PR.

## Style

`cargo fmt` and `cargo clippy` should be clean. Keep UI code in `src/ui/`, and
keep `app.rs` free of drawing.
