#!/usr/bin/env python3
"""Synthesises the film's small sound effects (pings, thumps, whooshes) to sfx.wav.
Usage: make_audio.py OUT.wav   (44.1 kHz, mono, 44 s). The music is rendered separately
by the app's own engine (see README)."""
import math, random, struct, sys, wave

SR, DUR = 44100, 44.0
buf = [0.0] * int(SR * DUR)

def add(t0, samples):
    i0 = int(t0 * SR)
    for i, v in enumerate(samples):
        if i0 + i < len(buf):
            buf[i0 + i] += v

def ping(freq=1320, amp=.30, dur=.30):
    n = int(dur * SR)
    return [amp * math.exp(-i / SR * 14) * (math.sin(2 * math.pi * freq * i / SR) + .35 * math.sin(2 * math.pi * freq * 2.01 * i / SR)) for i in range(n)]

def thump(amp=.55, dur=1.1):
    n = int(dur * SR)
    return [amp * math.exp(-i / SR * 4.5) * math.sin(2 * math.pi * (46 + 30 * math.exp(-i / SR * 9)) * i / SR) for i in range(n)]

def whoosh(amp=.22, dur=1.1, seed=1):
    random.seed(seed); n = int(dur * SR); out = []; lp = 0.0
    for i in range(n):
        k = i / n; env = math.sin(math.pi * k) ** 2
        a = .02 + .30 * k                      # opening low-pass => rising whoosh
        lp += a * (random.uniform(-1, 1) - lp)
        out.append(amp * env * lp * 3)
    return out

# cue sheet (seconds): matches the CLICK / scene times in index.html
add(5.30, thump())                       # logo lands
add(10.35, whoosh(seed=2))               # window rises
add(18.45, ping(1100))                   # tap a hymn
add(28.35, ping(1320))                   # tap verse 3
for t in (29.75, 30.20, 30.65): add(t, ping(880, .24))   # mute S, A, T
add(32.15, ping(1568))                   # pause-after-each-verse on
add(35.10, whoosh(amp=.26, dur=1.3, seed=3))   # light-mode wipe
add(38.75, thump(.5))                    # end logo lands

peak = max(1e-9, max(abs(v) for v in buf))
scale = min(1.0, .9 / peak)
with wave.open(sys.argv[1], 'wb') as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
    w.writeframes(b''.join(struct.pack('<h', int(max(-1, min(1, v * scale)) * 32767)) for v in buf))
print('wrote', sys.argv[1])
