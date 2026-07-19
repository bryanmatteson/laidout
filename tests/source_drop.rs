use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU32;
use std::thread;

use laidout::{render, Doc, LayoutStrategy, RenderOptions};
use static_assertions::{assert_impl_all, assert_not_impl_any};

assert_impl_all!(Doc<String>: Send, Sync);
assert_not_impl_any!(Doc<std::rc::Rc<()>>: Send, Sync);

fn unary_chain(depth: usize) -> Doc<String> {
    let mut doc = Doc::text("leaf");
    for _ in 0..depth {
        doc = Doc::group(doc);
    }
    doc
}

#[test]
fn unique_deep_source_chain_drops_without_recursion() {
    drop(unary_chain(100_000));
}

#[test]
fn branching_and_shared_graphs_drop_without_recursion() {
    let shared = unary_chain(20_000);
    let mut branches = Vec::new();
    for _ in 0..64 {
        branches.push(Doc::choice(shared.clone(), Doc::text("fallback")));
    }
    drop(Doc::concat(branches));
    drop(shared);
}

#[test]
fn cloned_roots_can_take_their_final_drops_on_different_threads() {
    let root = unary_chain(50_000);
    let left = root.clone();
    let right = root.clone();
    let left_thread = thread::spawn(move || drop(left));
    let right_thread = thread::spawn(move || drop(right));
    drop(root);
    left_thread.join().unwrap();
    right_thread.join().unwrap();
}

#[test]
fn deep_rebuilt_graphs_compare_and_hash_iteratively() {
    let left = unary_chain(20_000);
    let right = unary_chain(20_000);
    assert_eq!(left, right);

    let mut left_hash = DefaultHasher::new();
    left.hash(&mut left_hash);
    let mut right_hash = DefaultHasher::new();
    right.hash(&mut right_hash);
    assert_eq!(left_hash.finish(), right_hash.finish());
}

#[test]
fn exact_solving_uses_workspace_frames_for_deep_unary_documents() {
    let doc = unary_chain(50_000);
    let rendered = render(
        &doc,
        RenderOptions::new(NonZeroU32::new(80).unwrap()).with_strategy(LayoutStrategy::Exact),
    )
    .unwrap();
    assert_eq!(rendered.text(), "leaf");
}

#[test]
fn debug_marks_sharing_without_addresses() {
    let shared = Doc::<String>::text("x");
    let debug = format!("{:?}", Doc::choice(shared.clone(), shared));
    assert!(debug.contains("#"));
    assert!(!debug.contains("0x"));
}
