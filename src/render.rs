//! Shared output representation.
//!
//! Engines produce a rope of output events; rendering to lines and costing a
//! rendering are defined once here so every engine is measured identically.

use std::rc::Rc;

use crate::cost::{display_width, CostModel};
use crate::doc::TagId;

#[derive(Clone, Debug)]
pub enum Out {
    Empty,
    Text(Rc<str>, Option<TagId>),
    Newline(u32),
    Cat(Rc<Out>, Rc<Out>),
}

pub fn out_empty() -> Rc<Out> {
    Rc::new(Out::Empty)
}

pub fn out_text(s: Rc<str>, tag: Option<TagId>) -> Rc<Out> {
    Rc::new(Out::Text(s, tag))
}

pub fn out_newline(indent: u32) -> Rc<Out> {
    Rc::new(Out::Newline(indent))
}

pub fn out_cat(a: Rc<Out>, b: Rc<Out>) -> Rc<Out> {
    Rc::new(Out::Cat(a, b))
}

/// Flatten a rope into lines. Each line already includes its indentation.
pub fn to_lines(out: &Out) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut stack = vec![out];
    while let Some(o) = stack.pop() {
        match o {
            Out::Empty => {}
            Out::Text(s, _) => lines.last_mut().unwrap().push_str(s),
            Out::Newline(indent) => {
                let mut line = String::new();
                for _ in 0..*indent {
                    line.push(' ');
                }
                lines.push(line);
            }
            Out::Cat(a, b) => {
                stack.push(b);
                stack.push(a);
            }
        }
    }
    lines
}

pub fn to_string(out: &Out) -> String {
    to_lines(out).join("\n")
}

/// Cost of a finished rendering, computed from its lines. Because cost
/// models are incremental, this agrees with cost accumulated during layout.
pub fn cost_of_lines<C: CostModel>(cm: &C, lines: &[String]) -> C::Cost {
    let mut total = cm.zero();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            total = cm.add(&total, &cm.newline());
        }
        total = cm.add(&total, &cm.text(0, display_width(line)));
    }
    total
}

/// The whitespace-insensitive content of a rendering: every engine and every
/// layout of the same document must agree on this.
pub fn words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_owned).collect()
}
