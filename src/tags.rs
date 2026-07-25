//! Built-in semantic tags used by text ingestion and token constructors.
//!
//! Tags 1 through 5 mirror the intent of the Go prototype's text flags.
//! Formatter-specific tags start at [`CUSTOM_START`]. Tags annotate output
//! spans only; they never affect layout.

use crate::doc::TagId;

/// Horizontal whitespace.
pub const WHITESPACE: TagId = 1;
/// Punctuation or another single-character symbol.
pub const SYMBOL: TagId = 2;
/// Leading indentation.
pub const INDENT: TagId = 3;
/// A reflow break introduced from horizontal whitespace.
pub const NEWLINE: TagId = 4;
/// An alphanumeric or identifier-like word.
pub const WORD: TagId = 5;

/// First tag value reserved for application-specific meanings.
pub const CUSTOM_START: TagId = 32;
