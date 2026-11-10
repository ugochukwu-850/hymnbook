//! The app logo: an "F" with a musical note, drawn from plain shapes so it
//! matches `assets/icon/icon.svg` exactly. Coordinates are on a 512 grid.

use super::*;
use crate::theme::{BRAND_GOLD, BRAND_INK};

pub fn logo(size: f32) -> Div {
    let k = size / 512.0;
    let shape = |x: f32, y: f32, w: f32, h: f32, radius: f32| {
        div()
            .absolute()
            .left(px(x * k))
            .top(px(y * k))
            .w(px(w * k))
            .h(px(h * k))
            .rounded(px(radius * k))
            .bg(BRAND_INK)
    };
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded(px(112.0 * k))
        .bg(BRAND_GOLD)
        // the F: stem, top bar, middle bar
        .child(shape(150.0, 112.0, 62.0, 288.0, 31.0))
        .child(shape(150.0, 112.0, 212.0, 62.0, 31.0))
        .child(shape(150.0, 222.0, 150.0, 56.0, 28.0))
        // the note: head and stem
        .child(shape(286.0, 314.0, 88.0, 88.0, 44.0))
        .child(shape(356.0, 232.0, 18.0, 126.0, 9.0))
}
