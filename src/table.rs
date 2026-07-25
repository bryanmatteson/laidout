//! Pure static compilation for aligned tables with flat cells.
//!
//! The builder measures each cell's flat projection once, constructs a
//! padded compact table and a vertically stacked fallback, then returns an
//! ordinary [`Doc::choice`] wrapped in [`Doc::align`]. No callbacks or ambient
//! layout state enter the document tree.

use crate::doc::{Doc, FlatAlternative, Node};
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
/// Horizontal alignment inside a measured table column.
pub enum Alignment {
    #[default]
    /// Place padding after the cell.
    Left,
    /// Place padding before the cell.
    Right,
    /// Split padding around the cell.
    Center,
}

#[derive(Debug, Eq, PartialEq)]
/// Configuration for one table column.
pub struct Column<A = u32> {
    /// Optional header document.
    pub header: Option<Doc<A>>,
    /// Cell alignment in this column.
    pub alignment: Alignment,
    /// Minimum separation from the following column.
    pub min_padding: u16,
}

impl<A> Clone for Column<A> {
    fn clone(&self) -> Self {
        Self {
            header: self.header.clone(),
            alignment: self.alignment,
            min_padding: self.min_padding,
        }
    }
}

impl<A> Default for Column<A> {
    fn default() -> Self {
        Self {
            header: None,
            alignment: Alignment::Left,
            min_padding: 2,
        }
    }
}

impl<A> Column<A> {
    /// Create a column without a header.
    pub fn unlabeled() -> Self {
        Self::default()
    }

    /// Create a column with a header.
    pub fn with_header(header: Doc<A>) -> Self {
        Self {
            header: Some(header),
            ..Self::default()
        }
    }

    /// Set the horizontal alignment.
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Set minimum padding after this column.
    pub fn min_padding(mut self, min_padding: u16) -> Self {
        self.min_padding = min_padding;
        self
    }
}

impl Column<u32> {
    /// Create an unlabelled column using built-in numeric annotations.
    pub fn new() -> Self {
        Self::unlabeled()
    }

    /// Create a labelled column using built-in numeric annotations.
    pub fn labeled(header: Doc) -> Self {
        Self::with_header(header)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
/// Failure while compiling a table to ordinary document nodes.
pub enum TableError {
    /// A header has no single-line projection.
    NonFlattenableHeader {
        /// Zero-based column index.
        column: usize,
    },
    /// A body cell has no single-line projection.
    NonFlattenableCell {
        /// Zero-based row index.
        row: usize,
        /// Zero-based column index.
        column: usize,
    },
    /// A header contains a layout choice, which table measurement forbids.
    ChoiceBearingHeader {
        /// Zero-based column index.
        column: usize,
    },
    /// A body cell contains a layout choice, which table measurement forbids.
    ChoiceBearingCell {
        /// Zero-based row index.
        row: usize,
        /// Zero-based column index.
        column: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
/// Stable category for a [`TableError`].
pub enum TableErrorKind {
    /// A header cannot be flattened.
    NonFlattenableHeader,
    /// A body cell cannot be flattened.
    NonFlattenableCell,
    /// A header contains a layout choice.
    ChoiceBearingHeader,
    /// A body cell contains a layout choice.
    ChoiceBearingCell,
}

impl TableError {
    /// Return the stable category of this error.
    pub const fn kind(&self) -> TableErrorKind {
        match self {
            Self::NonFlattenableHeader { .. } => TableErrorKind::NonFlattenableHeader,
            Self::NonFlattenableCell { .. } => TableErrorKind::NonFlattenableCell,
            Self::ChoiceBearingHeader { .. } => TableErrorKind::ChoiceBearingHeader,
            Self::ChoiceBearingCell { .. } => TableErrorKind::ChoiceBearingCell,
        }
    }
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFlattenableHeader { column } => {
                write!(f, "table header at column {column} has no flat projection")
            }
            Self::NonFlattenableCell { row, column } => {
                write!(
                    f,
                    "table cell at row {row}, column {column} has no flat projection"
                )
            }
            Self::ChoiceBearingHeader { column } => write!(
                f,
                "table header at column {column} contains a layout choice"
            ),
            Self::ChoiceBearingCell { row, column } => write!(
                f,
                "table cell at row {row}, column {column} contains a layout choice"
            ),
        }
    }
}

impl Error for TableError {}

#[derive(Debug, Eq, PartialEq)]
/// Builder for a statically measured aligned table.
pub struct Table<A = u32> {
    columns: Vec<Column<A>>,
    rows: Vec<Vec<Doc<A>>>,
    show_header: bool,
}

struct FlatCell<A> {
    doc: Doc<A>,
    width: u32,
}

impl<A> Clone for Table<A> {
    fn clone(&self) -> Self {
        Self {
            columns: self.columns.clone(),
            rows: self.rows.clone(),
            show_header: self.show_header,
        }
    }
}

impl<A> Default for Table<A> {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            show_header: false,
        }
    }
}

impl<A> Table<A> {
    /// Create a table from column specifications.
    pub fn new(columns: impl IntoIterator<Item = Column<A>>) -> Self {
        Self {
            columns: columns.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Include configured column headers in the output.
    pub fn header(mut self) -> Self {
        self.show_header = true;
        self
    }

    /// Append a body row and return the builder.
    pub fn row(mut self, cells: impl IntoIterator<Item = Doc<A>>) -> Self {
        self.push_row(cells);
        self
    }

    /// Append a body row in place.
    pub fn push_row(&mut self, cells: impl IntoIterator<Item = Doc<A>>) {
        self.rows.push(cells.into_iter().collect());
    }

    /// Compile the table into ordinary document nodes.
    pub fn build(&self) -> Result<Doc<A>, TableError> {
        let column_count = self
            .rows
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(0)
            .max(self.columns.len());
        if column_count == 0 {
            return Ok(Doc::empty());
        }

        let mut columns = self.columns.clone();
        columns.resize(column_count, Column::default());

        let header_len = columns
            .iter()
            .rposition(|column| column.header.is_some())
            .map_or(0, |last| last + 1);
        let header = (self.show_header && header_len > 0).then(|| {
            columns[..header_len]
                .iter()
                .enumerate()
                .map(|(column, spec)| match &spec.header {
                    Some(doc) if contains_choice(doc) => {
                        Err(TableError::ChoiceBearingHeader { column })
                    }
                    Some(doc) => {
                        flatten_cell(doc).ok_or(TableError::NonFlattenableHeader { column })
                    }
                    None => Ok(empty_cell()),
                })
                .collect::<Result<Vec<_>, _>>()
        });
        let header = match header {
            Some(header) => Some(header?),
            None => None,
        };

        let rows = self
            .rows
            .iter()
            .enumerate()
            .map(|(row, cells)| {
                cells
                    .iter()
                    .enumerate()
                    .map(|(column, doc)| {
                        if contains_choice(doc) {
                            Err(TableError::ChoiceBearingCell { row, column })
                        } else {
                            flatten_cell(doc).ok_or(TableError::NonFlattenableCell { row, column })
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;

        if header.is_none() && rows.is_empty() {
            return Ok(Doc::empty());
        }

        let mut widths = vec![0; column_count];
        if let Some(header) = &header {
            update_widths(&mut widths, header);
        }
        for row in &rows {
            update_widths(&mut widths, row);
        }

        let logical_rows = header.iter().chain(rows.iter()).collect::<Vec<_>>();
        let has_horizontal_layout = logical_rows.iter().any(|row| row.len() > 1);
        let compact = Doc::join(
            Doc::hard_line(),
            logical_rows
                .iter()
                .map(|row| compact_row(row, &columns, &widths)),
        );
        let fallback = Doc::join(
            Doc::hard_line(),
            logical_rows
                .iter()
                .map(|row| Doc::join(Doc::hard_line(), row.iter().map(|cell| cell.doc.clone()))),
        );

        let compiled = if has_horizontal_layout {
            Doc::choice(compact, fallback)
        } else {
            compact
        };
        Ok(Doc::align(compiled))
    }
}

/// Create a table builder from column specifications.
pub fn table<A>(columns: impl IntoIterator<Item = Column<A>>) -> Table<A> {
    Table::new(columns)
}

fn empty_cell<A>() -> FlatCell<A> {
    FlatCell {
        doc: Doc::empty(),
        width: 0,
    }
}

fn flatten_cell<A>(doc: &Doc<A>) -> Option<FlatCell<A>> {
    let doc = doc.flatten()?;
    let width = flat_width(&doc)?;
    Some(FlatCell { doc, width })
}

fn contains_choice<A>(doc: &Doc<A>) -> bool {
    let mut work = vec![doc];
    while let Some(doc) = work.pop() {
        match doc.root().as_ref() {
            Node::Group(_) | Node::Choice { .. } => return true,
            Node::Fill(children) if children.len() > 1 => return true,
            Node::Seq(children) | Node::Fill(children) => work.extend(children.iter()),
            Node::Nest { child, .. }
            | Node::Align(child)
            | Node::Annotate { child, .. }
            | Node::Penalty { child, .. } => work.push(child),
            Node::Empty | Node::Text(_) | Node::Break(_) | Node::HardLine => {}
        }
    }
    false
}

fn flat_width<A>(doc: &Doc<A>) -> Option<u32> {
    let mut width = 0u32;
    let mut work = vec![doc];
    while let Some(doc) = work.pop() {
        match doc.root().as_ref() {
            Node::Empty => {}
            Node::Text(value) => width = width.checked_add(value.columns())?,
            Node::Break(FlatAlternative::Space) => width = width.checked_add(1)?,
            Node::Break(FlatAlternative::Empty) => {}
            Node::HardLine => return None,
            Node::Seq(children) | Node::Fill(children) => {
                for child in children.iter().rev() {
                    work.push(child);
                }
            }
            Node::Group(child)
            | Node::Nest { child, .. }
            | Node::Align(child)
            | Node::Annotate { child, .. }
            | Node::Penalty { child, .. } => work.push(child),
            Node::Choice { preferred, .. } => work.push(preferred),
        }
    }
    Some(width)
}

fn update_widths<A>(widths: &mut [u32], row: &[FlatCell<A>]) {
    for (column, cell) in row.iter().enumerate() {
        widths[column] = widths[column].max(cell.width);
    }
}

fn compact_row<A>(row: &[FlatCell<A>], columns: &[Column<A>], widths: &[u32]) -> Doc<A> {
    Doc::concat(row.iter().enumerate().map(|(column, cell)| {
        let slack = widths[column] - cell.width;
        let (before, after) = match columns[column].alignment {
            Alignment::Left => (0, slack),
            Alignment::Right => (slack, 0),
            Alignment::Center => (slack / 2, slack - slack / 2),
        };
        let has_later_cell = column + 1 < row.len();
        let trailing = if has_later_cell {
            after
                .checked_add(u32::from(columns[column].min_padding))
                .expect("table padding width overflow")
        } else {
            0
        };
        Doc::concat([spaces(before), cell.doc.clone(), spaces(trailing)])
    }))
}

fn spaces<A>(width: u32) -> Doc<A> {
    if width == 0 {
        Doc::empty()
    } else {
        Doc::text(" ".repeat(width as usize))
    }
}
