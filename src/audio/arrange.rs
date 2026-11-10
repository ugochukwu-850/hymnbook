//! Turning a one-pass tune into the full song: verse, chorus, verse, chorus...

use super::midi::Note;
use super::synth::{EPS, Ev};
use super::{Section, SectionKind};

pub(super) struct Arranged {
    pub events: Vec<Ev>,
    pub sections: Vec<Section>,
    pub stops: Vec<f64>,
    pub total: f64,
}

/// Lay the single-pass tune out as verse 1, chorus, verse 2, chorus, ...
pub(super) fn arrange(
    notes: &[Note],
    length: f64,
    chorus_at: Option<f64>,
    verses: u32,
) -> Arranged {
    let verses = verses.max(1);
    let mut sections = Vec::new();
    let mut stops = Vec::new();
    // (kind, verse, source start, source end)
    let mut plan: Vec<(SectionKind, u32, f64, f64)> = Vec::new();
    for v in 1..=verses {
        match chorus_at.filter(|&c| c > 0.0 && c < length) {
            Some(c) => {
                plan.push((SectionKind::Verse, v, 0.0, c));
                plan.push((SectionKind::Chorus, v, c, length));
            }
            None => plan.push((SectionKind::Verse, v, 0.0, length)),
        }
    }

    let mut events = Vec::new();
    let mut offset = 0.0;
    for (i, &(kind, verse, a, b)) in plan.iter().enumerate() {
        let len = b - a;
        sections.push(Section {
            kind,
            verse,
            start: offset,
            len,
        });
        for n in notes
            .iter()
            .filter(|n| n.start >= a - EPS && n.start < b - EPS)
        {
            let on = offset + (n.start - a);
            // A note never rings into the next section.
            let off = (offset + (n.end - a)).min(offset + len).max(on + 0.01);
            events.push(Ev {
                time: on,
                track: n.track,
                on: true,
                key: n.key,
                vel: n.vel,
            });
            events.push(Ev {
                time: off,
                track: n.track,
                on: false,
                key: n.key,
                vel: 0,
            });
        }
        offset += len;
        // Pause point: after each verse's last section (verse, or its chorus).
        let ends_group = plan.get(i + 1).is_none_or(|nx| nx.0 == SectionKind::Verse);
        if ends_group && i + 1 < plan.len() {
            stops.push(offset);
        }
    }
    // Offs before ons at the same instant so repeated notes re-articulate.
    events.sort_by(|x, y| x.time.total_cmp(&y.time).then(x.on.cmp(&y.on)));
    Arranged {
        events,
        sections,
        stops,
        total: offset,
    }
}
