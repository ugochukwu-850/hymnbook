//! The tunes bundled with the app. `generated.rs` is written by
//! `tools/abc2mid.py`; do not edit it by hand.

mod generated;

pub use generated::TUNES;

use crate::audio::TuneAsset;

/// The bundled tune for a hymn number, if there is one.
pub fn for_hymn(hymn_no: u32) -> Option<&'static TuneAsset> {
    TUNES.iter().find(|t| t.hymn == hymn_no)
}
