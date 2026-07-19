//! Render the ported JSON corpus at several widths with both engines,
//! printing costs so greedy-versus-optimal gaps are visible.

use laidout::corpus::{complex_value, format};
use laidout::cost::OverflowThenHeight;
use laidout::{greedy, render_with, SolveLimits};

fn main() {
    let doc = format(&complex_value());
    for width in [120u32, 44, 28] {
        let cm = OverflowThenHeight { width };
        let g = greedy::layout(&cm, &doc, width);
        let best = render_with(&doc, &cm, SolveLimits::default()).expect("exact render");
        println!(
            "== width {width}  greedy cost {:?}  optimal cost {:?}",
            g.cost, best.cost
        );
        println!("{}", best.text);
        println!();
    }
}
