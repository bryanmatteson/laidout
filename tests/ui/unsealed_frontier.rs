use laidout::Doc;
use laidout::research::{CostModel, frontier};

#[derive(Debug)]
struct External;

impl CostModel for External {
    type Cost = u32;

    fn zero(&self) -> Self::Cost { 0 }
    fn add(&self, left: &Self::Cost, right: &Self::Cost) -> Self::Cost { left + right }
    fn text(&self, _column: u32, width: u32) -> Self::Cost { width }
    fn newline(&self) -> Self::Cost { 1 }
    fn penalty(&self, amount: u32) -> Self::Cost { amount }
}

fn main() {
    let doc = Doc::<u32>::text("x");
    let _ = frontier::best(&External, &doc);
}
