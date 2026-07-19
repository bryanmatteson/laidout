//! Render styled nested Unicode text through crate-root consumer APIs only.

use std::io::{self, Write};

use laidout::{concat, render, tag, text, AnnotationSpan, RenderOptions};

const EMPHASIS: u32 = 100;
const ACCENT: u32 = 101;

fn main() -> io::Result<()> {
    let doc = tag(
        EMPHASIS,
        concat([text("status "), tag(ACCENT, text("界")), text(": ready")]),
    );
    let rendered = render(&doc, &RenderOptions::new(80)).expect("exact example render");

    assert_eq!(rendered.text, "status 界: ready");
    assert_eq!(
        rendered.spans,
        vec![
            AnnotationSpan {
                tag: EMPHASIS,
                range: 0..17,
                parent: None,
            },
            AnnotationSpan {
                tag: ACCENT,
                range: 7..10,
                parent: Some(0),
            },
        ]
    );
    let runs = rendered.annotated_runs();
    assert_eq!(
        runs.iter()
            .map(|run| (run.text, run.tags.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("status ", vec![EMPHASIS]),
            ("界", vec![EMPHASIS, ACCENT]),
            (": ready", vec![EMPHASIS]),
        ]
    );

    let mut stdout = io::stdout().lock();
    for run in runs {
        if run.tags.contains(&EMPHASIS) {
            write!(stdout, "\x1b[1m")?;
        }
        if run.tags.contains(&ACCENT) {
            write!(stdout, "\x1b[36m")?;
        }
        write!(stdout, "{}\x1b[0m", run.text)?;
    }
    writeln!(stdout)
}
