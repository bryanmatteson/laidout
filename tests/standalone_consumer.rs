use std::convert::Infallible;
use std::num::NonZeroU32;

use laidout::{
    render, render_into, solve_into, ActiveAnnotations, AnnotationId, Doc, LayoutStrategy,
    LayoutVisitor, RenderError, RenderOptions, RenderWorkspace, WorkspaceMode,
};

#[derive(Debug, Eq, Hash, PartialEq)]
enum ApplicationToken {
    Key,
    Value,
}

#[derive(Default)]
struct EventCounter {
    bytes: usize,
    annotations: usize,
}

impl LayoutVisitor<ApplicationToken> for EventCounter {
    type Error = Infallible;

    fn enter_annotation(
        &mut self,
        _id: AnnotationId,
        _annotation: &ApplicationToken,
    ) -> Result<(), Self::Error> {
        self.annotations += 1;
        Ok(())
    }

    fn text(
        &mut self,
        text: &str,
        _annotations: ActiveAnnotations<'_, ApplicationToken>,
    ) -> Result<(), Self::Error> {
        self.bytes += text.len();
        Ok(())
    }

    fn newline(
        &mut self,
        _indent: u32,
        _annotations: ActiveAnnotations<'_, ApplicationToken>,
    ) -> Result<(), Self::Error> {
        self.bytes += 1;
        Ok(())
    }
}

#[test]
fn public_kernel_needs_no_downstream_console_types_or_dependency() {
    let doc = Doc::group(Doc::concat([
        Doc::annotate(ApplicationToken::Key, Doc::text("status:")),
        Doc::line(),
        Doc::annotate(ApplicationToken::Value, Doc::text("ready")),
    ]));
    let prepared = doc.prepare().unwrap();

    for strategy in [LayoutStrategy::Fast, LayoutStrategy::Exact] {
        let options = RenderOptions::new(NonZeroU32::new(8).unwrap()).with_strategy(strategy);
        let owned = render(&doc, options).unwrap();
        assert_eq!(owned.text(), "status:\nready");

        let mut workspace = RenderWorkspace::growable();
        let warm = render_into(&prepared, options, &mut workspace).unwrap();
        assert_eq!(warm.text(), "status:\nready");
        drop(warm);

        let warm_layout = solve_into(&prepared, options, &mut workspace).unwrap();
        let mut warm_visitor = EventCounter::default();
        warm_layout.visit(&mut warm_visitor).unwrap();
        assert_eq!(warm_visitor.bytes, "status:\nready".len());
        assert_eq!(warm_visitor.annotations, 2);

        workspace.set_mode(WorkspaceMode::Fixed);
        let fixed = render_into(&prepared, options, &mut workspace).unwrap();
        assert_eq!(fixed.text(), "status:\nready");
        drop(fixed);
        let layout = solve_into(&prepared, options, &mut workspace).unwrap();
        let mut visitor = EventCounter::default();
        layout.visit(&mut visitor).unwrap();
        assert_eq!(visitor.bytes, "status:\nready".len());
        assert_eq!(visitor.annotations, 2);
    }

    let mut empty_fixed = RenderWorkspace::fixed(Default::default()).unwrap();
    assert!(matches!(
        render_into(
            &prepared,
            RenderOptions::new(NonZeroU32::new(8).unwrap()),
            &mut empty_fixed,
        ),
        Err(RenderError::WorkspaceExhausted { .. })
    ));
}
