//! Small, classified token constructors for formatter ergonomics.

use crate::doc::Doc;
use crate::tags;

/// Construct whitespace with the built-in whitespace tag.
pub fn whitespace(s: impl AsRef<str>) -> Doc {
    annotated(tags::WHITESPACE, s)
}

/// Construct indentation with the built-in indentation tag.
pub fn indent(s: impl AsRef<str>) -> Doc {
    annotated(tags::INDENT, s)
}

/// Construct a word with the built-in word tag.
pub fn word(s: impl AsRef<str>) -> Doc {
    annotated(tags::WORD, s)
}

/// Construct a symbol with the built-in symbol tag.
pub fn symbol(s: impl AsRef<str>) -> Doc {
    annotated(tags::SYMBOL, s)
}

/// Construct one tagged ASCII space.
pub fn space() -> Doc {
    whitespace(" ")
}

/// Construct text carrying an application-defined annotation.
pub fn annotated<A>(annotation: A, text: impl AsRef<str>) -> Doc<A> {
    Doc::annotate(annotation, Doc::text(text))
}

/// Construct whitespace carrying an application-defined annotation.
pub fn whitespace_with<A>(annotation: A, text: impl AsRef<str>) -> Doc<A> {
    annotated(annotation, text)
}

/// Construct indentation carrying an application-defined annotation.
pub fn indent_with<A>(annotation: A, text: impl AsRef<str>) -> Doc<A> {
    annotated(annotation, text)
}

/// Construct a word carrying an application-defined annotation.
pub fn word_with<A>(annotation: A, text: impl AsRef<str>) -> Doc<A> {
    annotated(annotation, text)
}

/// Construct a symbol carrying an application-defined annotation.
pub fn symbol_with<A>(annotation: A, text: impl AsRef<str>) -> Doc<A> {
    annotated(annotation, text)
}

macro_rules! symbol_token {
    ($name:ident, $text:literal, $docs:literal) => {
        #[doc = $docs]
        pub fn $name() -> Doc {
            symbol($text)
        }
    };
}

symbol_token!(dot, ".", "Construct a tagged period.");
symbol_token!(comma, ",", "Construct a tagged comma.");
symbol_token!(colon, ":", "Construct a tagged colon.");
symbol_token!(semicolon, ";", "Construct a tagged semicolon.");
symbol_token!(lbrace, "{", "Construct a tagged left brace.");
symbol_token!(rbrace, "}", "Construct a tagged right brace.");
symbol_token!(lbracket, "[", "Construct a tagged left bracket.");
symbol_token!(rbracket, "]", "Construct a tagged right bracket.");
symbol_token!(lchevron, "<", "Construct a tagged left angle bracket.");
symbol_token!(rchevron, ">", "Construct a tagged right angle bracket.");
symbol_token!(lparen, "(", "Construct a tagged left parenthesis.");
symbol_token!(rparen, ")", "Construct a tagged right parenthesis.");
symbol_token!(dquote, "\"", "Construct a tagged double quote.");
symbol_token!(squote, "'", "Construct a tagged single quote.");
symbol_token!(backtick, "`", "Construct a tagged backtick.");
symbol_token!(backslash, "\\", "Construct a tagged backslash.");
symbol_token!(slash, "/", "Construct a tagged slash.");
symbol_token!(equal, "=", "Construct a tagged equals sign.");
symbol_token!(star, "*", "Construct a tagged asterisk.");
symbol_token!(question_mark, "?", "Construct a tagged question mark.");
symbol_token!(bang, "!", "Construct a tagged exclamation mark.");
symbol_token!(pipe, "|", "Construct a tagged vertical bar.");
