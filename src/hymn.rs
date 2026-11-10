//! Hymn data: loading, lyric parsing, and search.

use serde::Deserialize;

const RAW: &str = include_str!("../data/hymns.ndjson");

#[derive(Deserialize)]
struct RawHymn {
    hymn_no: String,
    hymn_title: String,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Verse(Option<u32>),
    Chorus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Voice part this line is for, e.g. `Sop`, `ATB`, `S/A`.
    pub voice: Option<String>,
    /// Dynamics marking from the score text, e.g. `mf`, `p` (kept for playback).
    pub dynamic: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone)]
pub struct Hymn {
    pub number: u32,
    /// Title as shown in the UI (Title Case).
    pub title: String,
    pub blocks: Vec<Block>,
    /// Lowercased title for matching.
    title_key: String,
    /// Lowercased title + lyrics for matching.
    full_key: String,
}

impl Hymn {
    pub fn verse_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|b| matches!(b.kind, BlockKind::Verse(_)))
            .count()
    }

    pub fn has_chorus(&self) -> bool {
        self.blocks.iter().any(|b| b.kind == BlockKind::Chorus)
    }

    /// Distinct voice tags in order of first appearance, e.g. `["Sop", "ATB"]`.
    pub fn voice_parts(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for v in self
            .blocks
            .iter()
            .flat_map(|b| b.lines.iter())
            .filter_map(|l| l.voice.as_ref())
        {
            if !out.contains(v) {
                out.push(v.clone());
            }
        }
        out
    }

    /// First lyric line, used as a list subtitle.
    pub fn first_line(&self) -> &str {
        self.blocks
            .iter()
            .flat_map(|b| b.lines.iter())
            .map(|l| l.text.as_str())
            .find(|t| !t.is_empty())
            .unwrap_or("")
    }
}

/// Load all hymns, sorted by number. Malformed rows are skipped.
pub fn load() -> Vec<Hymn> {
    let mut hymns: Vec<Hymn> = RAW
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<RawHymn>(l).ok())
        .filter_map(|r| {
            let number = r.hymn_no.trim().parse().ok()?;
            let blocks = parse_body(&r.body);
            let title = title_case(&r.hymn_title);
            let title_key = normalize(&r.hymn_title);
            let mut full_key = title_key.clone();
            full_key.push('\n');
            full_key.push_str(&normalize(&r.body));
            Some(Hymn {
                number,
                title,
                blocks,
                title_key,
                full_key,
            })
        })
        .collect();
    hymns.sort_by_key(|h| h.number);
    hymns
}

fn normalize(s: &str) -> String {
    s.replace(['\u{2019}', '\u{2018}'], "'")
        .replace(['\u{201C}', '\u{201D}'], "\"")
        .to_lowercase()
}

/// "ALL HAIL, IMMANUEL!" -> "All Hail, Immanuel!"
pub fn title_case(s: &str) -> String {
    const SMALL: [&str; 13] = [
        "a", "an", "and", "as", "at", "but", "for", "in", "of", "on", "or", "the", "to",
    ];
    let mut out = String::with_capacity(s.len());
    let mut first = true;
    for (i, word) in s.split(' ').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let lower = word.to_lowercase();
        let bare: String = lower.chars().filter(|c| c.is_alphabetic()).collect();
        if !first && SMALL.contains(&bare.as_str()) {
            out.push_str(&lower);
        } else {
            out.push_str(&capitalize_word(&lower));
        }
        if !bare.is_empty() {
            first = false;
        }
    }
    out
}

/// Capitalise the first letter, and any letter after `-` or `(`.
/// Letters after an apostrophe stay lowercase ("Thy", "Ev'ry", "Jesus'").
fn capitalize_word(w: &str) -> String {
    let mut out = String::with_capacity(w.len());
    let mut up = true;
    for c in w.chars() {
        if c.is_alphabetic() && up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
        if c == '-' || c == '(' || c == '"' || c == '\u{201C}' {
            up = true;
        }
    }
    out
}

// ---------------------------------------------------------------- parsing

const VOICE_WORDS: [&str; 26] = [
    "s", "a", "t", "b", "sop", "soprano", "alto", "ten", "tenor", "bass", "atb", "tb", "sa", "sat",
    "at", "sb", "st", "ab", "all", "gen", "duet", "quartet", "parts", "unison", "harmony", "others",
];

fn is_voice_prefix(prefix: &str) -> bool {
    let mut any = false;
    for tok in prefix.split(|c: char| c == '/' || c == '.' || c == ' ' || c == ',') {
        if tok.is_empty() {
            continue;
        }
        let t = tok.to_lowercase();
        if t == "solo" || VOICE_WORDS.contains(&t.as_str()) {
            any = true;
        } else {
            return false;
        }
    }
    any
}

fn is_chorus_prefix(prefix: &str) -> bool {
    let p = prefix.trim().to_lowercase();
    let p = p.trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ');
    matches!(p, "chorus" | "refrain" | "cho" | "full chorus" | "cho." | "ref")
}

/// Split `"Prefix: rest"` at the first colon if the prefix is short.
fn split_prefix(s: &str) -> Option<(&str, &str)> {
    let idx = s.find(':')?;
    if idx > 18 {
        return None;
    }
    Some((&s[..idx], s[idx + 1..].trim()))
}

/// `"2. Ye gates"` / `"4 Peace be"` / `"3"` -> (2, "Ye gates")
fn split_verse_number(s: &str) -> Option<(u32, &str)> {
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 2 {
        return None;
    }
    let n: u32 = s[..digits].parse().ok()?;
    let rest = &s[digits..];
    let rest = match rest.chars().next() {
        None => "",
        Some('.') | Some(')') => rest[1..].trim(),
        Some(c) if c.is_whitespace() => rest.trim(),
        _ => return None,
    };
    Some((n, rest))
}

const DYNAMICS: [&str; 8] = ["f", "ff", "fff", "mf", "mp", "p", "pp", "ppp"];

/// Split a leading dynamics marking: `"mf AT the Name"` -> (`mf`, `"AT the Name"`).
/// Only when followed by a capitalised word, so real lyrics are never eaten.
fn split_dynamic(s: &str) -> (Option<String>, &str) {
    if let Some((first, rest)) = s.split_once(' ') {
        let rest = rest.trim_start();
        if DYNAMICS.contains(&first) && rest.chars().next().is_some_and(|c| c.is_uppercase()) {
            return (Some(first.to_string()), rest);
        }
    }
    (None, s)
}

/// "ALL hail to Thee" -> "All hail to Thee". Hymn books capitalise the first
/// word fully; shouting it in the UI looks wrong. Needs a following word that
/// contains lowercase so "HALLELUJAH! HALLELUJAH!" and "I"/"O" are untouched.
fn soften_first_word(s: &str) -> String {
    let Some((first, rest)) = s.split_once(' ') else {
        return s.to_string();
    };
    let letters: Vec<char> = first.chars().filter(|c| c.is_alphabetic()).collect();
    let shouting = letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase());
    let next_has_lower = rest.chars().any(|c| c.is_lowercase());
    let next_word_lower = rest
        .split(' ')
        .next()
        .is_some_and(|w| w.chars().any(|c| c.is_lowercase()));
    if shouting && next_has_lower && next_word_lower {
        format!("{} {}", capitalize_word(&first.to_lowercase()), rest)
    } else {
        s.to_string()
    }
}

fn make_line(text: &str) -> Line {
    let (voice, rest) = match split_prefix(text) {
        Some((prefix, rest)) if is_voice_prefix(prefix) => {
            (Some(prefix.trim().to_string()), rest)
        }
        _ => (None, text),
    };
    let (dynamic, rest) = split_dynamic(rest);
    Line {
        voice,
        dynamic,
        text: soften_first_word(rest),
    }
}

pub fn parse_body(body: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    // True when the previous non-blank line was followed by a blank line.
    let mut after_blank = true;

    for raw in body.lines() {
        let s = raw.trim();
        if s.is_empty() {
            after_blank = true;
            continue;
        }

        // Chorus header, e.g. "Chorus:" or "Refrain: Hail to the King".
        if let Some((prefix, rest)) = split_prefix(s) {
            if is_chorus_prefix(prefix) {
                let mut b = Block {
                    kind: BlockKind::Chorus,
                    lines: Vec::new(),
                };
                if !rest.is_empty() {
                    b.lines.push(make_line(rest));
                }
                blocks.push(b);
                after_blank = false;
                continue;
            }
        }

        // Numbered verse start.
        if let Some((n, rest)) = split_verse_number(s) {
            let mut b = Block {
                kind: BlockKind::Verse(Some(n)),
                lines: Vec::new(),
            };
            if !rest.is_empty() {
                b.lines.push(make_line(rest));
            }
            blocks.push(b);
            after_blank = false;
            continue;
        }

        let line = make_line(s);
        match blocks.last_mut() {
            // Same block continues unless a blank line split it; voice-tagged
            // lines after a blank still belong to the previous block
            // (choruses are often split by blank lines between parts).
            Some(last) if !after_blank || line.voice.is_some() => last.lines.push(line),
            _ => blocks.push(Block {
                kind: BlockKind::Verse(None),
                lines: vec![line],
            }),
        }
        after_blank = false;
    }

    // Verse 1 is normally unnumbered in the book. Label it when later verses
    // are numbered, so the reader can show a consistent 1, 2, 3 gutter.
    let has_second = blocks.iter().any(|b| b.kind == BlockKind::Verse(Some(2)));
    if has_second {
        if let Some(first) = blocks
            .iter_mut()
            .take_while(|b| !matches!(b.kind, BlockKind::Verse(Some(_))))
            .find(|b| b.kind == BlockKind::Verse(None))
        {
            first.kind = BlockKind::Verse(Some(1));
        }
    }
    blocks
}

// ----------------------------------------------------------------- search

/// Indices into `hymns` matching `query`, best first.
///
/// - empty query: everything, in number order
/// - digits only: hymns whose number starts with it (exact match first)
/// - otherwise: every word must appear in title or lyrics; title hits first
pub fn search(hymns: &[Hymn], query: &str) -> Vec<usize> {
    let q = normalize(query.trim());
    if q.is_empty() {
        return (0..hymns.len()).collect();
    }

    if q.chars().all(|c| c.is_ascii_digit()) {
        let mut hits: Vec<usize> = hymns
            .iter()
            .enumerate()
            .filter(|(_, h)| h.number.to_string().starts_with(&q))
            .map(|(i, _)| i)
            .collect();
        hits.sort_by_key(|&i| (hymns[i].number.to_string() != q, hymns[i].number));
        return hits;
    }

    let terms: Vec<&str> = q.split_whitespace().collect();
    let mut hits: Vec<(u8, usize)> = hymns
        .iter()
        .enumerate()
        .filter_map(|(i, h)| {
            if !terms.iter().all(|t| h.full_key.contains(t)) {
                return None;
            }
            let rank = if h.title_key.starts_with(&q) {
                0
            } else if terms.iter().all(|t| h.title_key.contains(t)) {
                1
            } else {
                2
            };
            Some((rank, i))
        })
        .collect();
    hits.sort_by_key(|&(rank, i)| (rank, hymns[i].number));
    hits.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_thousand_in_order() {
        let h = load();
        assert_eq!(h.len(), 1000);
        assert_eq!(h[0].number, 1);
        assert_eq!(h[999].number, 1000);
        assert!(h.windows(2).all(|w| w[0].number < w[1].number));
    }

    #[test]
    fn title_case_handles_small_words_and_apostrophes() {
        assert_eq!(title_case("ALL HAIL, IMMANUEL!"), "All Hail, Immanuel!");
        assert_eq!(
            title_case("YE GATES LIFT UP YOUR HEADS ON HIGH"),
            "Ye Gates Lift Up Your Heads on High"
        );
        assert_eq!(title_case("PRAISE GOD (Doxology)"), "Praise God (Doxology)");
        assert_eq!(title_case("THE LORD'S MY SHEPHERD"), "The Lord's My Shepherd");
    }

    #[test]
    fn parses_verses_and_chorus() {
        let b = parse_body("Line one\r\nLine two\r\n\r\nChorus:\r\nSop: Hail\r\nATB: Hail to\r\n\r\n2. Second\r\nmore");
        assert_eq!(b.len(), 3);
        // verse 1 is unnumbered in the source; labelled because verse 2 exists
        assert_eq!(b[0].kind, BlockKind::Verse(Some(1)));
        assert_eq!(b[0].lines.len(), 2);
        assert_eq!(b[1].kind, BlockKind::Chorus);
        assert_eq!(b[1].lines[0].voice.as_deref(), Some("Sop"));
        assert_eq!(b[1].lines[0].text, "Hail");
        assert_eq!(b[2].kind, BlockKind::Verse(Some(2)));
        assert_eq!(b[2].lines[0].text, "Second");
    }

    #[test]
    fn verse_number_without_dot() {
        let b = parse_body("4 Peace be within");
        assert_eq!(b[0].kind, BlockKind::Verse(Some(4)));
        assert_eq!(b[0].lines[0].text, "Peace be within");
    }

    #[test]
    fn voice_lines_after_blank_stay_in_chorus() {
        let b = parse_body("Chorus:\nSA: Behold\n\nTB: Behold, what manner");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].lines.len(), 2);
        assert_eq!(b[0].lines[1].voice.as_deref(), Some("TB"));
    }

    #[test]
    fn non_voice_prefix_is_plain_text() {
        let b = parse_body("Note: this is lyric text");
        assert_eq!(b[0].lines[0].voice, None);
        assert_eq!(b[0].lines[0].text, "Note: this is lyric text");
    }

    #[test]
    fn compound_voice_tags() {
        for tag in ["S/A", "Alto/Ten/Bass", "Sop. Solo", "Full Chorus"] {
            let line = make_line(&format!("{tag}: la la"));
            if tag == "Full Chorus" {
                assert!(is_chorus_prefix(tag));
            } else {
                assert_eq!(line.voice.as_deref(), Some(tag));
            }
        }
    }

    #[test]
    fn dynamics_are_split_off() {
        let l = make_line("mf AT the Name of Jesus");
        assert_eq!(l.dynamic.as_deref(), Some("mf"));
        assert_eq!(l.text, "At the Name of Jesus");
        // lowercase lyric text is never eaten
        assert_eq!(make_line("p and so on").dynamic, None);
        // dynamic after a voice tag
        let l = make_line("Sop: f Hail");
        assert_eq!(l.voice.as_deref(), Some("Sop"));
        assert_eq!(l.dynamic.as_deref(), Some("f"));
    }

    #[test]
    fn first_word_is_softened() {
        assert_eq!(make_line("ALL hail to Thee").text, "All hail to Thee");
        assert_eq!(make_line("HALLELUJAH! HALLELUJAH!").text, "HALLELUJAH! HALLELUJAH!");
        assert_eq!(make_line("O come all ye faithful").text, "O come all ye faithful");
    }

    #[test]
    fn o_in_title_is_capital() {
        assert_eq!(title_case("WE LOVE THE PLACE, O GOD"), "We Love the Place, O God");
    }

    #[test]
    fn first_verse_gets_number_one() {
        let b = parse_body("First\n\nChorus:\nHail\n\n2. Second");
        assert_eq!(b[0].kind, BlockKind::Verse(Some(1)));
        // single-verse hymn stays unnumbered
        assert_eq!(parse_body("Only verse").first().unwrap().kind, BlockKind::Verse(None));
    }

    #[test]
    fn every_hymn_parses_to_something() {
        for h in load() {
            assert!(!h.blocks.is_empty(), "hymn {} has no blocks", h.number);
        }
    }

    #[test]
    fn search_by_number_exact_first() {
        let h = load();
        let r = search(&h, "1");
        assert_eq!(h[r[0]].number, 1);
        assert!(r.iter().all(|&i| h[i].number.to_string().starts_with('1')));
        let r = search(&h, "500");
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn search_title_beats_lyrics() {
        let h = load();
        let r = search(&h, "all hail");
        assert!(!r.is_empty());
        assert_eq!(h[r[0]].number, 1);
    }

    #[test]
    fn search_lyrics_and_curly_quotes() {
        let h = load();
        assert!(!search(&h, "boundless love").is_empty());
        assert!(search(&h, "zzzzqqq").is_empty());
        assert_eq!(search(&h, "").len(), 1000);
    }
}
