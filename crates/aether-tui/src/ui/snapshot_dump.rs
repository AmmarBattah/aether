#![cfg(test)]

//! Prints one frame so a reviewer can eyeball the layout with
//! `cargo test -p aether-tui dump_frame -- --nocapture`.
use crate::testing::{populated_app, render_to};

#[test]
fn dump_frame() {
    println!("{}", render_to(&populated_app(), 110, 32));
}
