//! Render the recovered SQL corpus with greedy and optimal costs.

use laidout::corpus::sql::{complex_query, Formatter};
use laidout::cost::OverflowThenHeight;
use laidout::{greedy, render_with, SolveLimits};

fn main() {
    let doc = Formatter::default().format_query(&complex_query());
    for width in [100u32, 80, 52] {
        let cm = OverflowThenHeight { width };
        let greedy = greedy::layout(&cm, &doc, width);
        let best = render_with(&doc, &cm, SolveLimits::default()).expect("exact render");
        println!(
            "== width {width}  greedy cost {:?}  optimal cost {:?}",
            greedy.cost, best.cost
        );
        println!("{}\n", best.text);
    }
}
