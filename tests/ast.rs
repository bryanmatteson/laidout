use laidout::corpus::ast::{asymmetric_file, sample_file, Formatter};
use laidout::cost::OverflowThenHeight;
use laidout::{brute, count_choices, frontier, greedy, to_string};
use num_bigint::BigUint;

#[test]
fn sample_matches_the_documented_go_like_shape() {
    let doc = Formatter::default().format_file(&sample_file());
    let best = frontier::best(&OverflowThenHeight { width: 100 }, &doc);
    assert_eq!(
        to_string(&best.out),
        concat!(
            "package main\n",
            "\n",
            "import (\n",
            "    \"fmt\"\n",
            "    \"strings\"\n",
            "\n",
            "    json \"encoding/json\"\n",
            "    \"encoding/xml\"\n",
            ")\n",
            "\n",
            "func (p Processor) processData(data []byte, options *Options) ",
            "(result ProcessResult, err error) {\n",
            "    // ... formatted statements\n",
            "}"
        )
    );
}

#[test]
fn asymmetric_signature_is_a_strict_frontier_win() {
    let doc = Formatter::default().format_file(&asymmetric_file());
    let cm = OverflowThenHeight { width: 18 };
    let greedy = greedy::layout(&cm, &doc, 18);
    let best = frontier::best(&cm, &doc);
    let oracle = brute::best(&cm, &doc, count_choices(&doc));

    assert_eq!(greedy.cost, (BigUint::from(0u8), 10));
    assert_eq!(best.cost, (BigUint::from(0u8), 7));
    assert!(best.cost < greedy.cost);
    assert_eq!(best.cost, oracle.cost);
    assert_eq!(
        to_string(&best.out),
        concat!(
            "package p\n",
            "\n",
            "func f(a A, b B) (\n",
            "    x XXXX,\n",
            "    y YYYY\n",
            ") {\n",
            "    work()\n",
            "}"
        )
    );
}
