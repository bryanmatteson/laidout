//! Render the Go-like AST corpus at several widths and compare greedy with
//! optimal costs.

use laidout::corpus::ast::{asymmetric_file, sample_file, Formatter};
use laidout::cost::OverflowThenHeight;
use laidout::{frontier, greedy, to_string};

fn main() {
    let doc = Formatter::default().format_file(&sample_file());
    let asymmetric = Formatter::default().format_file(&asymmetric_file());
    let cm = OverflowThenHeight { width: 18 };
    let greedy = greedy::layout(&cm, &asymmetric, 18);
    let best = frontier::best(&cm, &asymmetric);
    println!(
        "asymmetric signature at 18: greedy {:?}, optimal {:?}",
        greedy.cost, best.cost
    );
    println!("{}\n", to_string(&best.out));
    for width in [100u32, 72, 52, 36] {
        let cm = OverflowThenHeight { width };
        let greedy = greedy::layout(&cm, &doc, width);
        let best = frontier::best(&cm, &doc);
        println!(
            "== width {width}  greedy cost {:?}  optimal cost {:?}",
            greedy.cost, best.cost
        );
        println!("{}", to_string(&best.out));
        println!();
    }
}
