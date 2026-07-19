use std::convert::Infallible;
use std::num::NonZeroU32;

use laidout::{
    solve_into, ActiveAnnotations, AnnotationId, Doc, LayoutStrategy, LayoutVisitor, RenderOptions,
    RenderWorkspace,
};

#[derive(Debug, Eq, Hash, PartialEq)]
enum Token {
    Label,
    Value,
}

#[derive(Default)]
struct PlainText(String);

impl LayoutVisitor<Token> for PlainText {
    type Error = Infallible;

    fn text(
        &mut self,
        text: &str,
        _annotations: ActiveAnnotations<'_, Token>,
    ) -> Result<(), Self::Error> {
        self.0.push_str(text);
        Ok(())
    }

    fn newline(
        &mut self,
        indent: u32,
        _annotations: ActiveAnnotations<'_, Token>,
    ) -> Result<(), Self::Error> {
        self.0.push('\n');
        self.0.extend(std::iter::repeat_n(' ', indent as usize));
        Ok(())
    }

    fn enter_annotation(
        &mut self,
        _id: AnnotationId,
        _annotation: &Token,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn main() {
    let doc = Doc::group(Doc::concat([
        Doc::annotate(Token::Label, Doc::text("status:")),
        Doc::line(),
        Doc::annotate(Token::Value, Doc::text("ready")),
    ]));
    let prepared = doc.prepare().expect("prepare standalone document");
    let options =
        RenderOptions::new(NonZeroU32::new(8).unwrap()).with_strategy(LayoutStrategy::Exact);

    let mut workspace = RenderWorkspace::growable();
    let warm = solve_into(&prepared, options, &mut workspace).expect("warm exact workspace");
    let mut warm_output = PlainText::default();
    warm.visit(&mut warm_output).expect("warm layout visitor");
    assert_eq!(warm_output.0, "status:\nready");
    workspace.set_mode(laidout::WorkspaceMode::Fixed);

    let layout = solve_into(&prepared, options, &mut workspace).expect("fixed exact solve");
    let mut output = PlainText::default();
    layout.visit(&mut output).expect("visit selected layout");
    assert_eq!(output.0, "status:\nready");
    println!("{}", output.0);
}
