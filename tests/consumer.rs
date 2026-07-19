use std::convert::Infallible;
use std::num::NonZeroU32;

use laidout::{
    render, solve_into, ActiveAnnotations, Doc, LayoutStrategy, LayoutVisitor, RenderOptions,
    RenderWorkspace,
};

fn options(width: u32, strategy: LayoutStrategy) -> RenderOptions {
    RenderOptions::new(NonZeroU32::new(width).unwrap()).with_strategy(strategy)
}

#[test]
fn nested_and_empty_annotations_are_structural_spans() {
    let doc = Doc::annotate(
        10u32,
        Doc::concat([
            Doc::text("A"),
            Doc::annotate(11, Doc::text("界")),
            Doc::annotate(12, Doc::empty()),
            Doc::hard_line(),
            Doc::nest(2, Doc::text("z")),
        ]),
    );
    let rendered = render(&doc, options(80, LayoutStrategy::Exact)).unwrap();
    assert_eq!(rendered.text(), "A界\nz");
    let spans = rendered
        .resolved_spans()
        .map(|(span, tag)| {
            (
                *tag,
                span.range.clone(),
                span.parent.map(|parent| parent.index()),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(spans[0], (10, 0..6, None));
    assert_eq!(spans[1], (11, 1..4, Some(0)));
    assert_eq!(spans[2], (12, 4..4, Some(0)));
}

#[test]
fn penalties_select_branches_without_changing_bytes() {
    let doc = Doc::choice(
        Doc::annotate(1u32, Doc::penalize(3, Doc::text("same"))),
        Doc::annotate(2, Doc::text("same")),
    );
    let rendered = render(&doc, options(80, LayoutStrategy::Exact)).unwrap();
    assert_eq!(rendered.text(), "same");
    assert_eq!(rendered.resolved_spans().next().unwrap().1, &2);
    assert_eq!(rendered.cost().burden(), 0);
}

#[test]
fn overflow_remains_the_dominant_cost_component() {
    let doc = Doc::<u32>::choice(
        Doc::text("ab"),
        Doc::penalize(
            100,
            Doc::concat([Doc::text("a"), Doc::hard_line(), Doc::text("b")]),
        ),
    );
    let rendered = render(&doc, options(1, LayoutStrategy::Exact)).unwrap();
    assert_eq!(rendered.text(), "a\nb");
    assert_eq!(rendered.cost().overflow(), 0);
    assert_eq!(rendered.cost().burden(), 101);
}

#[test]
fn equal_cost_ties_preserve_the_preferred_branch() {
    let doc = Doc::choice(
        Doc::annotate(1u32, Doc::text("same")),
        Doc::annotate(2, Doc::text("same")),
    );
    let rendered = render(&doc, options(80, LayoutStrategy::Exact)).unwrap();
    assert_eq!(rendered.resolved_spans().next().unwrap().1, &1);
}

#[derive(Debug, Eq, PartialEq)]
enum Event {
    Enter(u32),
    Text(String, Vec<u32>),
    Penalty(u32),
    Exit(u32),
}

#[derive(Default)]
struct EventVisitor(Vec<Event>);

impl LayoutVisitor<u32> for EventVisitor {
    type Error = Infallible;

    fn enter_annotation(
        &mut self,
        _id: laidout::AnnotationId,
        value: &u32,
    ) -> Result<(), Self::Error> {
        self.0.push(Event::Enter(*value));
        Ok(())
    }

    fn exit_annotation(
        &mut self,
        _id: laidout::AnnotationId,
        value: &u32,
    ) -> Result<(), Self::Error> {
        self.0.push(Event::Exit(*value));
        Ok(())
    }

    fn penalty(&mut self, amount: u32) -> Result<(), Self::Error> {
        self.0.push(Event::Penalty(amount));
        Ok(())
    }

    fn text(
        &mut self,
        text: &str,
        annotations: ActiveAnnotations<'_, u32>,
    ) -> Result<(), Self::Error> {
        self.0.push(Event::Text(
            text.to_owned(),
            annotations.iter().map(|(_, value)| *value).collect(),
        ));
        Ok(())
    }

    fn newline(
        &mut self,
        _indent: u32,
        _annotations: ActiveAnnotations<'_, u32>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn visitor_replays_the_complete_structural_witness() {
    let doc = Doc::annotate(7u32, Doc::penalize(3, Doc::text("x")));
    let prepared = doc.prepare().unwrap();
    let mut workspace = RenderWorkspace::growable();
    let layout = solve_into(
        &prepared,
        options(80, LayoutStrategy::Exact),
        &mut workspace,
    )
    .unwrap();
    let mut visitor = EventVisitor::default();
    layout.visit(&mut visitor).unwrap();
    assert_eq!(
        visitor.0,
        [
            Event::Enter(7),
            Event::Penalty(3),
            Event::Text("x".into(), vec![7]),
            Event::Exit(7),
        ]
    );
}

#[test]
fn concat_and_zero_penalty_are_canonical_identities() {
    let value = Doc::<u32>::text("x");
    assert_eq!(Doc::<u32>::concat([]), Doc::empty());
    assert_eq!(Doc::concat([value.clone()]), value);
    assert_eq!(Doc::penalize(0, value.clone()), value);
}
