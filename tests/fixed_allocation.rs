use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use laidout::{
    render_into, Doc, LayoutStrategy, RenderError, RenderOptions, RenderWorkspace,
    WorkspaceResource,
};

struct CountingAllocator;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static FAIL_ALLOCATION: AtomicU64 = AtomicU64::new(0);
static MEASUREMENT_LOCK: Mutex<()> = Mutex::new(());

fn should_fail(call: u64) -> bool {
    let fail = FAIL_ALLOCATION.load(Ordering::Relaxed);
    fail != 0 && call == fail
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            let call = ALLOCATIONS.fetch_add(1, Ordering::Relaxed) + 1;
            if should_fail(call) {
                return std::ptr::null_mut();
            }
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            let call = ALLOCATIONS.fetch_add(1, Ordering::Relaxed) + 1;
            if should_fail(call) {
                return std::ptr::null_mut();
            }
        }
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if ACTIVE.load(Ordering::Relaxed) {
            DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            let call = REALLOCATIONS.fetch_add(1, Ordering::Relaxed) + 1;
            if should_fail(call) {
                return std::ptr::null_mut();
            }
        }
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn measured<T>(operation: impl FnOnce() -> T) -> (T, [u64; 3]) {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    REALLOCATIONS.store(0, Ordering::Relaxed);
    DEALLOCATIONS.store(0, Ordering::Relaxed);
    FAIL_ALLOCATION.store(0, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::SeqCst);
    let result = operation();
    ACTIVE.store(false, Ordering::SeqCst);
    (
        result,
        [
            ALLOCATIONS.load(Ordering::Relaxed),
            REALLOCATIONS.load(Ordering::Relaxed),
            DEALLOCATIONS.load(Ordering::Relaxed),
        ],
    )
}

fn fail_on_allocation<T>(call: u64, operation: impl FnOnce() -> T) -> T {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    REALLOCATIONS.store(0, Ordering::Relaxed);
    DEALLOCATIONS.store(0, Ordering::Relaxed);
    FAIL_ALLOCATION.store(call, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::SeqCst);
    let result = operation();
    ACTIVE.store(false, Ordering::SeqCst);
    FAIL_ALLOCATION.store(0, Ordering::Relaxed);
    result
}

fn options(strategy: LayoutStrategy) -> RenderOptions {
    RenderOptions::new(NonZeroU32::new(24).unwrap()).with_strategy(strategy)
}

fn document() -> Doc<&'static str> {
    Doc::group(Doc::concat([
        Doc::annotate("key", Doc::text("alpha")),
        Doc::line(),
        Doc::annotate("value", Doc::text("beta gamma")),
        Doc::choice(Doc::text("!"), Doc::penalize(2, Doc::text("?"))),
    ]))
}

#[test]
fn adequately_reserved_fixed_fast_and_exact_render_without_allocator_calls() {
    let _measurement = MEASUREMENT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let prepared = document().prepare().unwrap();
    for strategy in [LayoutStrategy::Fast, LayoutStrategy::Exact] {
        let mut warm = RenderWorkspace::growable();
        let rendered = render_into(&prepared, options(strategy), &mut warm).unwrap();
        black_box(rendered.text());
        drop(rendered);
        let capacity = warm.capacity();
        let mut fixed = RenderWorkspace::fixed(capacity).unwrap();
        let (text, calls) = measured(|| {
            let rendered = render_into(&prepared, options(strategy), &mut fixed).unwrap();
            let text = black_box(rendered.text());
            assert!(!text.is_empty());
            let text_len = text.len();
            drop(rendered);
            text_len
        });
        assert!(text > 0);
        assert_eq!(calls, [0, 0, 0], "{strategy:?}");
    }
}

#[test]
fn undersized_fixed_failure_is_typed_and_allocation_free() {
    let _measurement = MEASUREMENT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let prepared = document().prepare().unwrap();
    let mut warm = RenderWorkspace::growable();
    drop(render_into(&prepared, options(LayoutStrategy::Fast), &mut warm).unwrap());
    let mut capacity = warm.capacity();
    capacity.fast.work_items = capacity.fast.work_items.saturating_sub(1);
    let mut fixed = RenderWorkspace::fixed(capacity).unwrap();
    let (error, calls) =
        measured(
            || match render_into(&prepared, options(LayoutStrategy::Fast), &mut fixed) {
                Ok(_) => panic!("undersized fixed workspace unexpectedly rendered"),
                Err(error) => error,
            },
        );
    assert!(matches!(
        error,
        RenderError::WorkspaceExhausted {
            resource: WorkspaceResource::FastWorkItems,
            ..
        }
    ));
    assert_eq!(calls, [0, 0, 0]);
}

#[test]
fn explicit_reservation_is_transactional_when_a_late_allocation_fails() {
    let _measurement = MEASUREMENT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let initial = laidout::RenderCapacity::new(
        laidout::FastSolveCapacity::new(2),
        laidout::ExactSolveCapacity::new(2, 2, 2, 2),
        2,
        laidout::VisitCapacity::new(2, 2),
        laidout::MaterializeCapacity::new(2, 2),
    );
    let mut workspace = RenderWorkspace::fixed(initial).unwrap();
    let requested = laidout::RenderCapacity::new(
        laidout::FastSolveCapacity::new(64),
        laidout::ExactSolveCapacity::new(64, 64, 64, 64),
        64,
        laidout::VisitCapacity::new(64, 64),
        laidout::MaterializeCapacity::new(64, 64),
    );

    let error = fail_on_allocation(2, || workspace.reserve(requested)).unwrap_err();
    assert!(matches!(error, laidout::ReserveError::Allocation { .. }));
    assert_eq!(workspace.capacity(), initial);
}

fn assert_one_less_is_allocation_free(
    strategy: LayoutStrategy,
    resource: WorkspaceResource,
    reduce: impl FnOnce(&mut laidout::RenderCapacity),
) {
    let prepared = document().prepare().unwrap();
    let mut warm = RenderWorkspace::growable();
    drop(render_into(&prepared, options(strategy), &mut warm).unwrap());
    let mut capacity = warm.capacity();
    reduce(&mut capacity);
    let mut fixed = RenderWorkspace::fixed(capacity).unwrap();
    let (error, calls) = measured(
        || match render_into(&prepared, options(strategy), &mut fixed) {
            Ok(_) => panic!("one-less {resource:?} capacity unexpectedly rendered"),
            Err(error) => error,
        },
    );
    assert!(matches!(
        error,
        RenderError::WorkspaceExhausted {
            resource: actual,
            ..
        } if actual == resource
    ));
    assert_eq!(calls, [0, 0, 0], "{resource:?}");
}

#[test]
fn every_one_less_fixed_resource_failure_is_allocation_free() {
    let _measurement = MEASUREMENT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_one_less_is_allocation_free(
        LayoutStrategy::Fast,
        WorkspaceResource::FastWorkItems,
        |capacity| capacity.fast.work_items -= 1,
    );
    for (resource, reduce) in [
        (
            WorkspaceResource::ExactWorkItems,
            (|capacity: &mut laidout::RenderCapacity| capacity.exact.work_items -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::MemoEntries,
            (|capacity: &mut laidout::RenderCapacity| capacity.exact.memo_entries -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::RetainedCandidates,
            (|capacity: &mut laidout::RenderCapacity| capacity.exact.retained_candidates -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::FrontierScratch,
            (|capacity: &mut laidout::RenderCapacity| capacity.exact.frontier_scratch -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::PlanNodes,
            (|capacity: &mut laidout::RenderCapacity| capacity.plan_nodes -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::VisitWorkItems,
            (|capacity: &mut laidout::RenderCapacity| capacity.visit.work_items -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::AnnotationDepth,
            (|capacity: &mut laidout::RenderCapacity| capacity.visit.annotation_depth -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::OutputBytes,
            (|capacity: &mut laidout::RenderCapacity| capacity.materialize.output_bytes -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::Spans,
            (|capacity: &mut laidout::RenderCapacity| capacity.materialize.spans -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
    ] {
        assert_one_less_is_allocation_free(LayoutStrategy::Exact, resource, reduce);
    }
}
