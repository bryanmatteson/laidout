use pretty::corpus::sql::{complex_query, Formatter};
use pretty::cost::OverflowThenHeight;
use pretty::{frontier, greedy, to_string};

#[test]
fn complex_query_matches_the_prototype_fixture_at_80() {
    let doc = Formatter::default().format_query(&complex_query());
    let cm = OverflowThenHeight { width: 80 };
    let best = frontier::best(&cm, &doc);
    let greedy = greedy::layout(&cm, &doc, 80);

    assert!(best.cost <= greedy.cost);
    assert_eq!(
        to_string(&best.out),
        concat!(
            "WITH order_stats (user_id, total_orders) AS (\n",
            "    SELECT\n",
            "        user_id,\n",
            "        COUNT(*) AS total_orders\n",
            "    FROM orders\n",
            "    GROUP BY user_id\n",
            ")\n",
            "SELECT\n",
            "    u.name,\n",
            "    os.total_orders,\n",
            "    COALESCE(SUM(o.amount), 0) AS total_spent\n",
            "FROM users u\n",
            "LEFT JOIN order_stats os\n",
            "    ON os.user_id = u.id\n",
            "LEFT JOIN orders o\n",
            "    ON o.user_id = u.id\n",
            "WHERE u.status = 'active'\n",
            "    AND o.created_at >= CURRENT_DATE - INTERVAL '1 year'\n",
            "GROUP BY\n",
            "    u.name,\n",
            "    os.total_orders\n",
            "HAVING COUNT(*) > 5\n",
            "ORDER BY total_spent DESC\n",
            "LIMIT 10"
        )
    );
}
