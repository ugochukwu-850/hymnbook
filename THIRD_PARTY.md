# Third-party material

The application code is MIT licensed (see `LICENSE`). The files below are
bundled with it and keep their own licenses.

| What | Where | License |
|------|-------|---------|
| **Tunes** (MIDI converted from ABC scores) | `assets/tunes/`, `data/tune_matches.json` | Public domain. Source: [The Open Hymnal Project](https://github.com/mzealey/openhymnal) scores, which mark each file "public domain". Converted by `tools/abc2mid.py`. |
| **SoundFont** `TimGM6mb.sf2` | `assets/soundfont/` | GPL-2.0 (Tim Brechbill, David Bolton; some samples public domain). It is separate data that the program loads; if you redistribute the app, ship the SoundFont under its own terms. Replace it with any General MIDI SoundFont. |
| **Fonts** Noto Sans, Noto Serif, Noto Sans Mono | `assets/fonts/` | SIL Open Font License 1.1 (`assets/fonts/LICENSE-Noto.txt`). |
| **Icons** (Lucide) | via the `gpui-kit-assets` crate | ISC. |
| **Hymn texts** | `data/hymns.ndjson` | See the note below. |

## About the hymn texts

`data/hymns.ndjson` holds the 1000 hymn texts of the *Faith Tabernacle Hymn
Book* as shipped in its Android app. Most of these hymns are old and in the
public domain, but some were written in the 20th century and may still be under
copyright, and the *collection* was compiled by someone else. If you are a
rights holder and want an item removed, open an issue and it will be taken out
promptly. You can also run the app with your own `data/hymns.ndjson` (same
format: `hymn_no`, `hymn_title`, `body`).

## Rust dependencies

See `Cargo.lock`. Notably [GPUI](https://www.gpui.rs) (Apache-2.0),
`gpui-kit`, `cpal`, `rustysynth` (MIT), `midly` (MIT).
