//! Small, classified token constructors for formatter ergonomics.

use crate::doc::{tag, text, Doc};
use crate::tags;

pub fn whitespace(s: impl AsRef<str>) -> Doc {
    tag(tags::WHITESPACE, text(s))
}

pub fn indent(s: impl AsRef<str>) -> Doc {
    tag(tags::INDENT, text(s))
}

pub fn word(s: impl AsRef<str>) -> Doc {
    tag(tags::WORD, text(s))
}

pub fn symbol(s: impl AsRef<str>) -> Doc {
    tag(tags::SYMBOL, text(s))
}

pub fn space() -> Doc {
    whitespace(" ")
}

macro_rules! symbol_token {
    ($name:ident, $text:literal) => {
        pub fn $name() -> Doc {
            symbol($text)
        }
    };
}

symbol_token!(dot, ".");
symbol_token!(comma, ",");
symbol_token!(colon, ":");
symbol_token!(semicolon, ";");
symbol_token!(lbrace, "{");
symbol_token!(rbrace, "}");
symbol_token!(lbracket, "[");
symbol_token!(rbracket, "]");
symbol_token!(lchevron, "<");
symbol_token!(rchevron, ">");
symbol_token!(lparen, "(");
symbol_token!(rparen, ")");
symbol_token!(dquote, "\"");
symbol_token!(squote, "'");
symbol_token!(backtick, "`");
symbol_token!(backslash, "\\");
symbol_token!(slash, "/");
symbol_token!(equal, "=");
symbol_token!(star, "*");
symbol_token!(question_mark, "?");
symbol_token!(bang, "!");
symbol_token!(pipe, "|");
