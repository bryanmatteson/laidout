use std::num::NonZeroU32;

use laidout::{render, LayoutStrategy, RenderOptions};

#[path = "../support/sql.rs"]
#[allow(dead_code)]
mod corpus;
use corpus::{complex_query, Formatter};

#[test]
fn complex_query_retains_clause_order_and_indentation() {
    let doc = Formatter::default().format_query(&complex_query());
    let rendered = render(
        &doc,
        RenderOptions::new(NonZeroU32::new(80).unwrap()).with_strategy(LayoutStrategy::Exact),
    )
    .unwrap();
    let text = rendered.text();
    let with = text.find("WITH order_stats").unwrap();
    let select = text.find("SELECT").unwrap();
    let from = text.rfind("FROM users u").unwrap();
    let where_clause = text.find("WHERE u.status").unwrap();
    let order = text.find("ORDER BY total_spent DESC").unwrap();
    assert!(with < select && select < from && from < where_clause && where_clause < order);
    assert!(text.ends_with("LIMIT 10"));
    assert!(text.contains("\n    ON os.user_id = u.id"));
}
