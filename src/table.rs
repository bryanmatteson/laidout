//! Pure static compilation for aligned tables with flat cells.
//!
//! The builder measures each cell's flat projection once, constructs a
//! padded compact table and a vertically stacked fallback, then returns an
//! ordinary [`Doc::Choice`] wrapped in [`align`]. No callbacks or ambient
//! layout state enter the document tree.

use std::error::Error;
use std::fmt;
use std::rc::Rc;

use crate::cost::display_width;
use crate::doc::{align, choice, concat, empty, flatten, hardline, join, Doc};
use crate::tokens;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Alignment {
    #[default]
    Left,
    Right,
    Center,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Column {
    pub header: Option<Rc<Doc>>,
    pub alignment: Alignment,
    pub min_padding: u16,
}

impl Default for Column {
    fn default() -> Self {
        Self {
            header: None,
            alignment: Alignment::Left,
            min_padding: 2,
        }
    }
}

impl Column {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn labeled(header: Rc<Doc>) -> Self {
        Self {
            header: Some(header),
            ..Self::default()
        }
    }

    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn min_padding(mut self, min_padding: u16) -> Self {
        self.min_padding = min_padding;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TableError {
    NonFlattenableHeader { column: usize },
    NonFlattenableCell { row: usize, column: usize },
    ChoiceBearingHeader { column: usize },
    ChoiceBearingCell { row: usize, column: usize },
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Rc<Doc>>>,
    show_header: bool,
}

#[derive(Clone)]
struct FlatCell {
    doc: Rc<Doc>,
    width: u32,
}

impl Table {
    pub fn new(columns: impl IntoIterator<Item = Column>) -> Self {
        Self {
            columns: columns.into_iter().collect(),
            ..Self::default()
        }
    }

    pub fn header(mut self) -> Self {
        self.show_header = true;
        self
    }

    pub fn row(mut self, cells: impl IntoIterator<Item = Rc<Doc>>) -> Self {
        self.push_row(cells);
        self
    }

    pub fn push_row(&mut self, cells: impl IntoIterator<Item = Rc<Doc>>) {
        self.rows.push(cells.into_iter().collect());
    }

    /// Compile the table into ordinary document nodes.
    pub fn build(&self) -> Result<Rc<Doc>, TableError> {
        let column_count = self
            .rows
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(0)
            .max(self.columns.len());
        if column_count == 0 {
            return Ok(empty());
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
            return Ok(empty());
        }

        let mut widths = vec![0; column_count];
        if let Some(header) = &header {
            update_widths(&mut widths, header);
        }
        for row in &rows {
            update_widths(&mut widths, row);
        }

        let logical_rows = header.iter().chain(rows.iter()).collect::<Vec<_>>();
        let compact = join(
            hardline(),
            logical_rows
                .iter()
                .map(|row| compact_row(row, &columns, &widths)),
        );
        let fallback = join(
            hardline(),
            logical_rows
                .iter()
                .map(|row| join(hardline(), row.iter().map(|cell| cell.doc.clone()))),
        );

        let compiled = if compact == fallback {
            compact
        } else {
            choice(compact, fallback)
        };
        Ok(align(compiled))
    }
}

pub fn table(columns: impl IntoIterator<Item = Column>) -> Table {
    Table::new(columns)
}

fn empty_cell() -> FlatCell {
    FlatCell {
        doc: empty(),
        width: 0,
    }
}

fn flatten_cell(doc: &Rc<Doc>) -> Option<FlatCell> {
    let doc = flatten(doc)?;
    let width = flat_width(&doc)?;
    Some(FlatCell { doc, width })
}

fn contains_choice(doc: &Doc) -> bool {
    match doc {
        Doc::Empty | Doc::Text(_) | Doc::Line { .. } => false,
        Doc::Concat(left, right) => contains_choice(left) || contains_choice(right),
        Doc::Nest(_, inner) | Doc::Align(inner) | Doc::Tag(_, inner) => contains_choice(inner),
        Doc::Choice(_, _) => true,
    }
}

fn flat_width(doc: &Doc) -> Option<u32> {
    match doc {
        Doc::Empty => Some(0),
        Doc::Text(value) => Some(display_width(value)),
        Doc::Line { flat: Some(value) } => Some(display_width(value)),
        Doc::Line { flat: None } => None,
        Doc::Concat(left, right) => flat_width(left)?.checked_add(flat_width(right)?),
        Doc::Nest(_, inner) | Doc::Align(inner) | Doc::Tag(_, inner) => flat_width(inner),
        Doc::Choice(preferred, _) => flat_width(preferred),
    }
}

fn update_widths(widths: &mut [u32], row: &[FlatCell]) {
    for (column, cell) in row.iter().enumerate() {
        widths[column] = widths[column].max(cell.width);
    }
}

fn compact_row(row: &[FlatCell], columns: &[Column], widths: &[u32]) -> Rc<Doc> {
    concat(row.iter().enumerate().map(|(column, cell)| {
        let slack = widths[column] - cell.width;
        let (before, after) = match columns[column].alignment {
            Alignment::Left => (0, slack),
            Alignment::Right => (slack, 0),
            Alignment::Center => (slack / 2, slack - slack / 2),
        };
        let has_later_cell = column + 1 < row.len();
        let trailing = if has_later_cell {
            after + u32::from(columns[column].min_padding)
        } else {
            0
        };
        concat([spaces(before), cell.doc.clone(), spaces(trailing)])
    }))
}

fn spaces(width: u32) -> Rc<Doc> {
    if width == 0 {
        empty()
    } else {
        tokens::whitespace(" ".repeat(width as usize))
    }
}
