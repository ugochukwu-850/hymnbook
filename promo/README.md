# Hymn Book promo film

A 44-second, 1080p/30fps motion-design film. The animation is a single HTML page
(`index.html`) driven by a tiny deterministic timeline engine, so every frame is a pure
function of time. `render.mjs` steps through time, screenshots each frame in headless
Chromium, and pipes the frames into ffmpeg.

The **music is the app itself**: the app's own sound engine playing *The Old Rugged Cross*
(verse, then the chorus entering at 0:20.57). Scenes are cut on the music's bar lines
(140 bpm, 6/4, one bar = 2.571 s).

## Rebuild it

```bash
# 1. Music from the app's engine (organ), 1 verse + chorus
HYMN_SAMPLE_OUT=cross.wav HYMN_SAMPLE_HYMN=590 HYMN_SAMPLE_VERSES=1 \
  cargo test render_website_sample -- --ignored

# 2. Sound effects + final mix
python3 promo/make_audio.py sfx.wav
ffmpeg -i cross.wav -i sfx.wav -filter_complex \
  "[0:a]loudnorm=I=-17:TP=-2:LRA=9,afade=t=in:d=0.5,afade=t=out:st=41.8:d=2.2,apad=whole_dur=44[m];\
   [1:a]volume=0.75,apad=whole_dur=44[s];[m][s]amix=inputs=2:normalize=0:duration=first,alimiter=limit=0.95:level=disabled" \
  -ar 48000 -ac 2 promo_audio.wav

# 3. Render (needs Node, `npm i playwright-core`, ffmpeg and a Chromium)
cd promo && npm i playwright-core
CHROME=/path/to/chrome node render.mjs video ../promo_audio.wav hymnbook-promo.mp4
#   or review single frames:  node render.mjs stills out_dir 12.5 20.8 30.4
```

Fonts (Inter, Instrument Serif, JetBrains Mono, Noto Serif) load from Google Fonts at render time.
