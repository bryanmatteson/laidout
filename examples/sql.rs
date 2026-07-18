//! Render the recovered SQL corpus with greedy and optimal costs.

use pretty::corpus::sql::{complex_query, Formatter};
use pretty::cost::OverflowThenHeight;
use pretty::{frontier, greedy, to_string};

fn main() {
    let doc = Formatter::default().format_query(&complex_query());
    for width in [100u32, 80, 52] {
        let cm = OverflowThenHeight { width };
        let greedy = greedy::layout(&cm, &doc, width);
        let best = frontier::best(&cm, &doc);
        println!(
            "== width {width}  greedy cost {:?}  optimal cost {:?}",
            greedy.cost, best.cost
        );
        println!("{}\n", to_string(&best.out));
    }
}
