use std::num::NonZeroU32;

use laidout::{Doc, RenderOptions, RenderWorkspace, solve_into};

fn main() {
    let prepared = Doc::<u32>::text("x").prepare().unwrap();
    let options = RenderOptions::new(NonZeroU32::new(80).unwrap());
    let mut workspace = RenderWorkspace::growable();
    let first = solve_into(&prepared, options, &mut workspace).unwrap();
    let second = solve_into(&prepared, options, &mut workspace).unwrap();
    let _ = (first.cost(), second.cost());
}
