# Aligned tables

`Table<A>` compiles flat cells into ordinary `Doc<A>` nodes. Layout remains
pure: table construction invokes no callback and rendering uses the same
solver as every other document.

## Behavior

- Rows and headers accept application-owned annotations through `Doc<A>`.
- Columns support left, right, and center alignment.
- `min_padding` controls separation from the next cell.
- Missing column specifications use left alignment and two columns of padding.
- Widths use each text run's stored display columns.
- The compact alternative pads cells to shared column widths.
- The fallback emits cells vertically.
- `Doc::align` anchors subsequent rows at the table's starting column.
- Padding is unannotated. Cell and header annotations remain structural.

The builder rejects headers and cells that contain layout choices or lack a
flat projection. The restriction preserves deterministic column measurement
and prevents a table from discarding a cell's internal alternatives.

## Example

```rust
use laidout::{Alignment, Column, Doc, Table};

let table = Table::new([
    Column::labeled(Doc::text("name")),
    Column::labeled(Doc::text("count")).alignment(Alignment::Right),
])
.header()
.row([Doc::text("alpha"), Doc::text("12")])
.row([Doc::text("beta"), Doc::text("3")]);

let doc = table.build()?;
# Ok::<(), laidout::TableError>(())
```

`tests/table.rs` covers alignment, ragged rows, Unicode display width,
compact/fallback selection, and invalid cells.
