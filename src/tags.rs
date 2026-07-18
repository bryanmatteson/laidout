//! Built-in semantic tags used by text ingestion and token constructors.
//!
//! Tags 1 through 5 mirror the intent of the Go prototype's text flags.
//! Formatter-specific tags start at [`CUSTOM_START`]. Tags annotate output
//! spans only; they never affect layout.

use crate::doc::TagId;

pub const WHITESPACE: TagId = 1;
pub const SYMBOL: TagId = 2;
pub const INDENT: TagId = 3;
pub const NEWLINE: TagId = 4;
pub const WORD: TagId = 5;

pub const CUSTOM_START: TagId = 32;
