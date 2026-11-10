#!/usr/bin/env python3
"""Match the hymns in data/hymns.ndjson to scores in the `freehymns/hymns` repo.

Usage:  match_freehymns.py FREEHYMNS_DIR OUT.json [--min 0.5]

Matching compares the full lyrics (word trigrams), not titles, so different
titles for the same hymn still match. Each match records how many lines the
first verse and the refrain have in the source text; abc2mid.py uses that to
find where the chorus starts (one score phrase per lyric line).
"""
import json
import re
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def words(s: str):
    s = unicodedata.normalize("NFKD", s).lower().replace("’", "'")
    s = re.sub(r"(?<=[a-z])-(?=[a-z])", "", s)       # syllable hyphens: a-way -> away
    s = re.sub(r"[^a-z' ]+", " ", s).replace("'", "")
    return s.split()


def grams(ws, n=3):
    return {" ".join(ws[i:i + n]) for i in range(len(ws) - n + 1)}


def parse_text(path: Path):
    raw = path.read_text(encoding="utf-8", errors="replace")
    body, _, tail = raw.partition("\n---")
    lines = body.splitlines()
    title = lines[0].strip() if lines else path.stem
    tune = None
    for ln in (body + "\n" + tail).splitlines():
        if ln.startswith("Tune:"):
            tune = ln.split(":", 1)[1].strip()
    # blocks: "Verse N:" / "Refrain:" headers followed by lines
    blocks, cur = [], None
    for ln in lines[1:]:
        m = re.match(r"^(Verse\s*\d*|Refrain|Chorus)[^:]*:\s*$", ln.strip(), re.I)
        if m:
            cur = {"kind": "refrain" if m.group(1).lower() in ("refrain", "chorus") else "verse", "lines": []}
            blocks.append(cur)
        elif cur is not None and ln.strip() and not ln.startswith("Tune:"):
            cur["lines"].append(ln.strip())
    first_verse = next((b for b in blocks if b["kind"] == "verse"), None)

    def syllables(lines):
        n = 0
        for ln in lines:
            for w in ln.split():
                if re.search(r"[A-Za-z\u00C0-\u024F]", w):
                    n += 1 + w.strip("-").count("-")
        return n

    refrain = next((b for b in blocks if b["kind"] == "refrain"), None)
    text = " ".join(l for b in blocks for l in b["lines"])
    return {
        "title": title,
        "text": text,
        "tune": tune or path.stem,
        "verse_lines": len(first_verse["lines"]) if first_verse else 0,
        "refrain_lines": len(refrain["lines"]) if refrain else 0,
        "verse_syllables": syllables(first_verse["lines"]) if first_verse else 0,
    }


def main():
    fh = Path(sys.argv[1])
    out = Path(sys.argv[2])
    min_score = float(sys.argv[sys.argv.index("--min") + 1]) if "--min" in sys.argv else 0.5

    abc = {p.stem.lower(): p for p in (fh / "music").rglob("*.abc")}
    sources = []
    for p in sorted((fh / "en").rglob("*.txt")):
        d = parse_text(p)
        score_file = abc.get(d["tune"].lower()) or abc.get(p.stem.lower())
        if not score_file or not d["text"]:
            continue
        d["abc"] = str(score_file.relative_to(fh))
        d["g"] = grams(words(d["text"]))
        sources.append(d)

    # Hymns that already have an Open Hymnal tune are left alone; freehymns matches are redone.
    existing = {
        m["no"] for m in json.loads((ROOT / "data" / "tune_matches.json").read_text())
        if m.get("src", "oh") != "fh"
    }
    hymns = [json.loads(l) for l in (ROOT / "data" / "hymns.ndjson").read_text(encoding="utf-8").splitlines() if l.strip()]
    found = []
    for h in hymns:
        no = int(h["hymn_no"])
        if no in existing:
            continue
        body = re.sub(r"(?im)^\s*(chorus|refrain|cho\.?|[A-Za-z/\. ]{1,14}):\s*", "", h["body"].replace("\r", ""))
        body = re.sub(r"(?m)^\s*\d+[\.\)]?\s+", "", body)
        hg = grams(words(body))
        if not hg:
            continue
        best = (0.0, None)
        for s in sources:
            if not s["g"]:
                continue
            sc = len(hg & s["g"]) / min(len(hg), len(s["g"]))
            if sc > best[0]:
                best = (sc, s)
        if best[1] and best[0] >= min_score:
            s = best[1]
            found.append({
                "no": no, "title": h["hymn_title"], "file": s["abc"], "how": f"freehymns lyrics {best[0]:.2f}",
                "score": round(best[0], 3), "matched_title": s["title"],
                "verse_lines": s["verse_lines"], "refrain_lines": s["refrain_lines"],
                "verse_syllables": s["verse_syllables"],
            })
    out.write_text(json.dumps(found, indent=1))
    print(f"{len(sources)} usable sources; {len(found)} new matches (>= {min_score})")


if __name__ == "__main__":
    main()
