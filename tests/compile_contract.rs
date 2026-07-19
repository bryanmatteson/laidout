#[test]
fn borrowed_layout_prevents_workspace_reentry() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/layout_ref_reentry.rs");
}

#[cfg(feature = "research")]
#[test]
fn arbitrary_cost_models_cannot_enter_the_pruning_frontier() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/unsealed_frontier.rs");
}
