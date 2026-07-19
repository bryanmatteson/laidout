//! Benchmark corpus: a JSON formatter ported from the Go prototype
//! (`internal/doc/examples/json.go`), minus its `maxInline` heuristic — with
//! an optimal printer, inline-versus-multiline is the engine's decision, not
//! the formatter's.

#![allow(deprecated)]

use laidout::doc::{concat, group, join, nest, softline, tag, text, Doc, TagId};
use laidout::doc::{concat2, line};
use laidout::tags;

pub const TAG_KEY: TagId = tags::CUSTOM_START;
pub const TAG_STRING: TagId = tags::CUSTOM_START + 1;
pub const TAG_NUMBER: TagId = tags::CUSTOM_START + 2;
pub const TAG_BOOL: TagId = tags::CUSTOM_START + 3;
pub const TAG_PUNCT: TagId = tags::CUSTOM_START + 4;

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

const INDENT: u16 = 2;

fn string_doc(s: &str, t: TagId) -> Doc {
    tag(t, text(format!("\"{}\"", s.escape_default())))
}

fn comma_line() -> Doc {
    concat2(tag(TAG_PUNCT, text(",")), line())
}

fn bracketed(open: &str, close: &str, items: Vec<Doc>) -> Doc {
    if items.is_empty() {
        return tag(TAG_PUNCT, text(format!("{open}{close}")));
    }
    group(concat([
        tag(TAG_PUNCT, text(open)),
        nest(INDENT, concat([softline(), join(comma_line(), items)])),
        softline(),
        tag(TAG_PUNCT, text(close)),
    ]))
}

pub fn format(v: &Value) -> Doc {
    match v {
        Value::Null => text("null"),
        Value::Bool(b) => tag(TAG_BOOL, text(if *b { "true" } else { "false" })),
        Value::Int(n) => tag(TAG_NUMBER, text(n.to_string())),
        Value::Str(s) => string_doc(s, TAG_STRING),
        Value::Arr(items) => bracketed("[", "]", items.iter().map(format).collect()),
        Value::Obj(entries) => bracketed(
            "{",
            "}",
            entries
                .iter()
                .map(|(k, v)| {
                    group(concat([
                        string_doc(k, TAG_KEY),
                        tag(TAG_PUNCT, text(":")),
                        text(" "),
                        format(v),
                    ]))
                })
                .collect(),
        ),
    }
}

/// The sample document from the Go prototype.
pub fn complex_value() -> Value {
    Value::Obj(vec![
        ("name".into(), Value::Str("Example".into())),
        (
            "config".into(),
            Value::Obj(vec![
                ("timeout".into(), Value::Int(30)),
                ("retries".into(), Value::Int(3)),
                (
                    "servers".into(),
                    Value::Arr(vec![
                        Value::Obj(vec![
                            ("host".into(), Value::Str("server1".into())),
                            ("port".into(), Value::Int(8080)),
                        ]),
                        Value::Obj(vec![
                            ("host".into(), Value::Str("server2".into())),
                            ("port".into(), Value::Int(8081)),
                        ]),
                    ]),
                ),
            ]),
        ),
        ("enabled".into(), Value::Bool(true)),
        (
            "tags".into(),
            Value::Arr(vec![
                Value::Str("production".into()),
                Value::Str("v2".into()),
            ]),
        ),
    ])
}
