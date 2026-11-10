#!/usr/bin/env python3
"""Convert Open Hymnal ABC scores to multi-track MIDI (one track per voice).

Supports the subset of ABC the Open Hymnal uses: major keys, several voices
(`V:` / inline `[V:]`), tempo (`Q:`), ties, tuplets, broken rhythm, chords,
bar-wise accidentals. Repeats are NOT expanded (one pass through the music).

Usage:
    abc2mid.py MATCHES.json OPEN_HYMNAL_DIR OUT_DIR
MATCHES.json: [{"no": 4, "file": "Complete/X/Y.abc", ...}, ...]
Writes OUT_DIR/<0000>.mid for each hymn and OUT_DIR/index.json.
"""
import json
import re
import sys
from fractions import Fraction
from pathlib import Path

import mido

TPQ = 480
STEP = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
SHARPS = "FCGDAEB"
FLATS = "BEADGCF"
MAJOR_FIFTHS = {
    "C": 0, "G": 1, "D": 2, "A": 3, "E": 4, "B": 5, "F#": 6, "C#": 7,
    "F": -1, "Bb": -2, "Eb": -3, "Ab": -4, "Db": -5, "Gb": -6, "Cb": -7,
}
TUPLET = {2: Fraction(3, 2), 3: Fraction(2, 3), 4: Fraction(3, 4), 5: Fraction(3, 5),
          6: Fraction(2, 3), 7: Fraction(2, 3), 8: Fraction(3, 4), 9: Fraction(2, 3)}
SATB = {"S1V1": "Soprano", "S1V2": "Alto", "S2V1": "Tenor", "S2V2": "Bass"}
LEN_RE = re.compile(r"(\d*)(/*)(\d*)")
DECOR = set("HIJKLMNOPQRSTUVWhijklmnopqrstuvwy~.")


def key_offsets(k: str) -> dict:
    k = (k.split("%")[0].strip().split() or ["C"])[0]
    if k not in MAJOR_FIFTHS:
        raise ValueError(f"unsupported key {k!r}")
    n = MAJOR_FIFTHS[k]
    off = {c: 0 for c in "ABCDEFG"}
    for c in (SHARPS[:n] if n > 0 else FLATS[:-n]):
        off[c] = 1 if n > 0 else -1
    return off


def parse_len(s: str, i: int):
    m = LEN_RE.match(s, i)
    num, slashes, den = m.groups()
    if not num and not slashes:
        return Fraction(1), i
    n = int(num) if num else 1
    if slashes:
        d = int(den) if den else 2 ** len(slashes)
    else:
        d = 1
    return Fraction(n, d), m.end()


class Voice:
    def __init__(self, name):
        self.name = name
        self.t = Fraction(0)
        self.events = []          # [start, dur, pitch]
        self.ties = {}            # pitch -> event
        self.last = []            # events of the previous note (broken rhythm)
        self.bar_acc = {}


class Score:
    def __init__(self):
        self.unit = Fraction(1)   # quarter-lengths per default note length
        self.keyoff = key_offsets("C")
        self.bpm = None
        self.voices = {}
        self.order = []
        self.cur = None
        self.tuplet_left = 0
        self.tuplet_ratio = Fraction(1)
        self.next_mult = Fraction(1)
        # lyric alignment (melody voice only), used to find where the chorus starts
        self.melody = None
        self.rec = None            # slot times of the melody line being parsed
        self.last_line = None      # melody line the next w: lines belong to
        self.lines = []            # [{'slots': [quarter-lengths], 'w': [str]}]
        self.slur_open = False
        self.slur_in = False

    def voice(self, name):
        if name not in self.voices:
            self.voices[name] = Voice(name)
            self.order.append(name)
        return self.voices[name]

    # ------------------------------------------------------------- fields
    def field(self, key, val):
        val = val.split("%")[0].strip()
        if key == "L":
            a, b = val.split("/")
            self.unit = Fraction(int(a), int(b)) * 4
        elif key == "K":
            self.keyoff = key_offsets(val)
        elif key == "Q":
            m = re.search(r"(\d+)/(\d+)\s*=\s*(\d+)", val)
            if m and self.bpm is None:
                a, b, bpm = map(int, m.groups())
                self.bpm = bpm * Fraction(a, b) * 4
        elif key == "V":
            self.cur = self.voice(val.split()[0].strip('"'))

    # -------------------------------------------------------------- notes
    def pitch(self, s, i, v):
        acc = ""
        while s[i] in "^_=":
            acc += s[i]
            i += 1
        ch = s[i]
        i += 1
        base = 60 if ch.isupper() else 72
        octv = 0
        while i < len(s) and s[i] in "',":
            octv += 12 if s[i] == "'" else -12
            i += 1
        letter = ch.upper()
        midi = base + octv + STEP[letter]
        key = (letter, midi - STEP[letter])
        if acc:
            off = {"^": 1, "^^": 2, "_": -1, "__": -2, "=": 0}[acc]
            v.bar_acc[key] = off
        else:
            off = v.bar_acc.get(key, self.keyoff[letter])
        return midi + off, i

    def emit(self, v, pitches, dur):
        evs = []
        cont = False
        for p in pitches:
            ev = v.ties.pop(p, None)
            if ev is not None:
                cont = True
                ev[1] += dur
            else:
                ev = [v.t, dur, p]
                v.events.append(ev)
            evs.append(ev)
        # any tie target not continued is dropped
        v.ties = {}
        v.last = evs
        return evs, cont

    def slot(self, v, start, cont):
        """Record a note of the melody voice.

        In the Open Hymnal every note (tied or slurred ones too) consumes one
        lyric token ('*' marks the ones without a syllable), so each note is a slot."""
        if self.rec is not None and v.name == self.melody:
            self.rec.append(start)

    def scale(self):
        r = self.next_mult
        self.next_mult = Fraction(1)
        if self.tuplet_left > 0:
            r *= self.tuplet_ratio
            self.tuplet_left -= 1
        return r

    def music(self, s):
        i, n = 0, len(s)
        while i < n:
            c = s[i]
            if c.isspace() or c in "\\&`":
                i += 1
            elif c == "%":
                break
            elif c == "[" and i + 2 < n and s[i + 1].isalpha() and s[i + 2] == ":":
                j = s.index("]", i)
                self.field(s[i + 1], s[i + 3:j])
                i = j + 1
            elif c == "[" and i + 1 < n and s[i + 1] in "|0123456789":
                i += 1
                self._bar()
            elif c == "[":
                v = self.cur
                j = s.index("]", i)
                inner, k, pitches, first = s[i + 1:j], 0, [], Fraction(1)
                while k < len(inner):
                    if inner[k] in "^_=ABCDEFGabcdefg":
                        p, k = self.pitch(inner, k, v)
                        m, k = parse_len(inner, k)
                        if not pitches:
                            first = m
                        pitches.append(p)
                    else:
                        k += 1
                m, i = parse_len(s, j + 1)
                dur = self.unit * first * m * self.scale()
                tie = i < n and s[i] == "-"
                start = v.t
                evs, cont = self.emit(v, pitches, dur)
                self.slot(v, start, cont)
                if tie:
                    i += 1
                    v.ties = {e[2]: e for e in evs}
                v.t += dur
            elif c in "|:]":
                self._bar()
                i += 1
                while i < n and s[i].isdigit():
                    i += 1
            elif c in "!\"+{":
                end = {"!": "!", '"': '"', "+": "+", "{": "}"}[c]
                i = s.index(end, i + 1) + 1
            elif c == "(":
                if i + 1 < n and s[i + 1].isdigit():
                    p = int(s[i + 1])
                    self.tuplet_left, self.tuplet_ratio = p, TUPLET.get(p, Fraction(2, 3))
                    i += 2
                else:
                    self.slur_open = True
                    i += 1
            elif c == ")":
                self.slur_open = self.slur_in = False
                i += 1
            elif c == "-":
                i += 1
            elif c in "<>":
                run = 0
                while i < n and s[i] == c:
                    run += 1
                    i += 1
                big, small = Fraction(2) - Fraction(1, 2 ** run), Fraction(1, 2 ** run)
                if c == "<":
                    big, small = small, big
                v = self.cur
                if v.last:
                    old = v.last[0][1]
                    new = old * big
                    for e in v.last:
                        e[1] = new
                    v.t += new - old
                self.next_mult = small
            elif c in "zZxX":
                i += 1
                m, i = parse_len(s, i)
                self.cur.last = []
                self.cur.ties = {}
                self.cur.t += self.unit * m * self.scale()
            elif c in "^_=ABCDEFGabcdefg":
                v = self.cur
                p, i = self.pitch(s, i, v)
                m, i = parse_len(s, i)
                dur = self.unit * m * self.scale()
                tie = i < n and s[i] == "-"
                start = v.t
                evs, cont = self.emit(v, [p], dur)
                self.slot(v, start, cont)
                if tie:
                    i += 1
                    v.ties = {e[2]: e for e in evs}
                v.t += dur
            elif c in DECOR:
                i += 1
            else:
                i += 1

    def _bar(self):
        if self.cur is not None:
            self.cur.bar_acc = {}


def parse_abc(text: str) -> Score:
    sc = Score()
    in_body = False
    for raw in text.split("\n"):
        line = raw.rstrip("\r")
        if not line.strip() or line.startswith("%"):
            continue
        if line[:2] == "w:":
            if sc.last_line is not None:
                sc.last_line["w"].append(line[2:])
            continue
        if line[:2] == "W:":
            continue
        m = re.match(r"^([A-Za-z]):(.*)$", line)
        if m and not line.startswith("[") and m.group(1) in "XTCSMLKQVPZNOGHRBDFIUWwr":
            key, val = m.group(1), m.group(2)
            if key == "V" and not in_body:
                sc.voice(val.split()[0])
            if key in "LKQ" or key == "V":
                sc.field(key, val)
            if key == "K":
                in_body = True
            continue
        if in_body:
            if sc.melody is None and sc.order:
                sc.melody = sc.order[0]
            sc.rec = []
            sc.music(line)
            sc.last_line = {"slots": sc.rec, "w": []} if sc.rec else None
            if sc.last_line:
                sc.lines.append(sc.last_line)
            sc.rec = None
    return sc


def lyric_tokens(w: str):
    """Syllable tokens of one `w:` line: '*' skips a note, '_' holds, '|' is a bar hint."""
    toks = []
    for t in re.split(r"[\s\-]+", w.strip()):
        if t and t != "|":
            toks.append(t)
    return toks


def is_real(t: str) -> bool:
    return t not in ("*", "_")


def find_chorus(sc: Score):
    """Quarter-length where the chorus starts, or None.

    Verse lines carry lyrics in every `w:` set; chorus lines only in the first.
    The chorus begins at the first lyric of set 1 that comes after the last
    real syllable of set 2 on the same line, provided no later line has any
    real syllable in set 2."""
    # Layout B: chorus starts on a fresh music line that has a single lyric set,
    # after verse lines that have several, and every later line has one too.
    sets = [len(l["w"]) for l in sc.lines]
    for j in range(1, len(sets)):
        if sets[j] == 1 and max(sets[:j]) >= 2 and all(n == 1 for n in sets[j:]) \
                and all(n >= 2 for n in sets[:j] if n):
            # The chorus may start with a short pickup at the end of the last
            # verse line (its words appear only in lyric set 1 there).
            prev = sc.lines[j - 1]
            t1, t2 = lyric_tokens(prev["w"][0]), lyric_tokens(prev["w"][1])
            real2 = [k for k, t in enumerate(t2) if is_real(t)]
            last2 = real2[-1] if real2 else -1
            cand = [k for k, t in enumerate(t1) if is_real(t) and k > last2]
            slots = prev["slots"]
            if cand and cand[0] < len(slots) and len(slots) - cand[0] <= 4:
                return slots[cand[0]]
            return sc.lines[j]["slots"][0]
    # Layout A: chorus starts mid-line (see below).
    lines = [l for l in sc.lines if len(l["w"]) >= 2]
    if not lines:
        return None
    toks = [(lyric_tokens(l["w"][0]), lyric_tokens(l["w"][1]), l["slots"]) for l in lines]
    for i, (t1, t2, slots) in enumerate(toks):
        real2 = [k for k, t in enumerate(t2) if is_real(t)]
        last2 = real2[-1] if real2 else -1
        cand = [k for k, t in enumerate(t1) if is_real(t) and k > last2]
        if not cand or cand[0] >= len(slots):
            continue
        later_ok = all(not any(is_real(t) for t in t2b) for _, t2b, _ in toks[i + 1:])
        if later_ok:
            return slots[cand[0]]
    return None


def to_midi(sc: Score, path: Path, program: int = 19):
    mid = mido.MidiFile(type=1, ticks_per_beat=TPQ)
    bpm = float(sc.bpm or 90)
    meta = mido.MidiTrack()
    meta.append(mido.MetaMessage("set_tempo", tempo=mido.bpm2tempo(bpm), time=0))
    meta.append(mido.MetaMessage("end_of_track", time=0))
    mid.tracks.append(meta)
    names = [n for n in sc.order if sc.voices[n].events]
    for ch, name in enumerate(names):
        v = sc.voices[name]
        label = SATB.get(name, name) if len(names) == 4 else name
        tr = mido.MidiTrack()
        tr.append(mido.MetaMessage("track_name", name=label, time=0))
        tr.append(mido.Message("program_change", channel=ch, program=program, time=0))
        evs = []
        for start, dur, pitch in v.events:
            on = int(round(float(start) * TPQ))
            off = int(round(float(start + dur) * TPQ))
            off = max(on + 1, off - (10 if off - on > 40 else 0))   # small gap re-articulates
            evs.append((on, 1, pitch))
            evs.append((off, 0, pitch))
        evs.sort(key=lambda e: (e[0], e[1]))
        last = 0
        for tick, is_on, pitch in evs:
            msg = mido.Message("note_on" if is_on else "note_off", channel=ch,
                               note=max(0, min(127, pitch)), velocity=80 if is_on else 0,
                               time=tick - last)
            tr.append(msg)
            last = tick
        tr.append(mido.MetaMessage("end_of_track", time=0))
        mid.tracks.append(tr)
    mid.save(path)
    return names


def main():
    matches = json.load(open(sys.argv[1]))
    lyrics = {}
    hp = Path(__file__).resolve().parent.parent / "data" / "hymns.ndjson"
    for ln in hp.read_text(encoding="utf-8").splitlines():
        if ln.strip():
            r = json.loads(ln)
            lyrics[int(r["hymn_no"])] = r["body"]
    oh, out = Path(sys.argv[2]), Path(sys.argv[3])
    out.mkdir(parents=True, exist_ok=True)
    index, cache = {}, {}
    for m in matches:
        f = m["file"]
        if f not in cache:
            txt = (oh / f).read_text(encoding="latin-1")
            try:
                sc = parse_abc(txt)
                names = [n for n in sc.order if sc.voices[n].events]
                ends = {round(float(sc.voices[n].t), 3) for n in names}
                cache[f] = (sc, names, ends)
            except Exception as e:  # noqa: BLE001 - report and continue
                cache[f] = None
                print(f"FAIL {f}: {type(e).__name__}: {e}")
        if cache[f] is None:
            continue
        sc, names, ends = cache[f]
        dest = out / f"{int(m['no']):04d}.mid"
        to_midi(sc, dest)
        quarters = max(float(sc.voices[n].t) for n in names)
        bpm = float(sc.bpm or 90)
        body = lyrics.get(int(m["no"]), "")
        has_chorus = bool(re.search(r"(?im)^\s*(chorus|refrain|cho\.?)\s*:", body))
        cq = find_chorus(sc) if has_chorus else None
        index[str(int(m["no"]))] = {
            "title": m["title"],
            "source": f,
            "voices": [SATB.get(n, n) if len(names) == 4 else n for n in names],
            "bpm": bpm,
            "seconds": round(quarters * 60 / bpm, 1),
            "length": round(quarters * 60 / bpm, 3),
            "has_chorus": has_chorus,
            "chorus_at": round(float(cq) * 60 / bpm, 3) if cq is not None else None,
            "aligned": len(ends) == 1,
        }
    json.dump(index, open(out / "index.json", "w"), indent=1)
    if len(sys.argv) > 4:   # optional: emit the Rust table that embeds the tunes
        rel = Path(sys.argv[4])
        lines = [
            "// @generated by tools/abc2mid.py - do not edit.",
            "// Tunes: Open Hymnal Project scores (public domain), converted to MIDI.",
            "use crate::audio::TuneAsset;",
            "",
            "pub static TUNES: &[TuneAsset] = &[",
        ]
        for k in sorted(index, key=int):
            v = index[k]
            voices = ", ".join(json.dumps(x) for x in v["voices"])
            lines.append(
                f"    TuneAsset {{ hymn: {int(k)}, midi: include_bytes!(\"../../assets/tunes/{int(k):04d}.mid\"), "
                f"voices: &[{voices}], source: {json.dumps(v['source'])}, length: {v['length']}, "
                f"chorus_at: {('Some(%s)' % v['chorus_at']) if v['chorus_at'] is not None else 'None'} }},"
            )
        lines.append("];")
        rel.write_text("\n".join(lines) + "\n")
    bad = [k for k, v in index.items() if not v["aligned"]]
    print(f"wrote {len(index)} midi files; voices misaligned in: {bad or 'none'}")


if __name__ == "__main__":
    main()
