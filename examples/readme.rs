//! A small formatter that renders one prepared document at a chosen width.
use std::num::NonZeroU32;

use laidout::{table, Alignment, Column, Doc, RenderOptions, Renderer};

#[path = "../support/json.rs"]
#[allow(dead_code)]
mod json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let kind = args.next().unwrap_or_else(|| "json".into());
    let width = args.next().unwrap_or_else(|| "50".into()).parse::<u32>()?;
    let width = NonZeroU32::new(width).ok_or("width must be nonzero")?;
    let doc = match kind.as_str() {
        "json" => json::format(&json::Value::Obj(vec![
            ("project".into(), json::Value::Str("laidout".into())),
            ("features".into(), json::Value::Arr(vec![
                json::Value::Str("Unicode".into()),
                json::Value::Str("annotations".into()),
                json::Value::Str("exact layout".into()),
            ])),
            ("ready".into(), json::Value::Bool(true)),
        ])),
        "table" => table([
            Column::labeled(Doc::text("PROJECT")),
            Column::labeled(Doc::text("STARS")).alignment(Alignment::Right),
            Column::labeled(Doc::text("STATUS")),
        ]).header()
            .row([Doc::text("laidout"), Doc::text("128"), Doc::text("ready")])
            .row([Doc::text("東京"), Doc::text("42"), Doc::text("local")])
            .build()?,
        _ => return Err("usage: readme [json|table] [width]".into()),
    };
    let prepared = doc.prepare()?;
    let mut renderer = Renderer::new();
    let rendered = renderer.render_prepared(&prepared, RenderOptions::new(width))?;
    println!("{}", rendered.text());
    if kind == "json" && rendered.resolved_spans().count() == 0 {
        return Err("JSON annotations were lost".into());
    }
    Ok(())
}
