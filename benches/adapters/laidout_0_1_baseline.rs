//! Frozen self-contained Laidout 0.1 exact engine benchmark adapter.
//! This module imports no mutable production layout code.

mod frozen {
    pub mod doc {
        //! The document algebra.
        //!
        //! `Choice` is the primitive; `group` is sugar built with `flatten`. A `Line`
        //! node always renders as a break in every engine — its `flat` field exists
        //! only so `flatten` knows what the flat projection of that break is (space,
        //! nothing, or impossible).

        use std::error::Error;
        use std::fmt;
        use std::rc::Rc;

        use unicode_width::UnicodeWidthStr;

        /// Semantic tag identifier. Orthogonal to layout: tags never influence
        /// measurement or cost, they only annotate output spans.
        pub type TagId = u32;

        /// Terminal-column policy used when a text run is constructed.
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
        pub enum WidthMode {
            /// Unicode width with East Asian ambiguous characters treated as narrow.
            #[default]
            Narrow,
            /// Unicode width with East Asian ambiguous characters treated as wide.
            Cjk,
        }

        /// Failure to turn input text into measured document runs.
        #[non_exhaustive]
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum TextError {
            /// A terminal control character appeared where document text was expected.
            ControlCharacter { character: char, byte_offset: usize },
            /// The measured display width cannot be represented by the document model.
            DisplayWidthOverflow { byte_offset: usize },
        }

        impl fmt::Display for TextError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::ControlCharacter {
                        character,
                        byte_offset,
                    } => write!(
                        f,
                        "control character {character:?} at UTF-8 byte offset {byte_offset}"
                    ),
                    Self::DisplayWidthOverflow { byte_offset } => write!(
                        f,
                        "text display width exceeds u32::MAX at UTF-8 byte offset {byte_offset}"
                    ),
                }
            }
        }

        impl Error for TextError {}

        /// Immutable text plus the authoritative number of terminal columns it uses.
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub struct TextRun {
            value: Rc<str>,
            columns: u32,
        }

        impl TextRun {
            fn try_new(value: &str, width_mode: WidthMode) -> Result<Self, TextError> {
                if let Some((byte_offset, character)) = value
                    .char_indices()
                    .find(|(_, character)| character.is_control())
                {
                    return Err(TextError::ControlCharacter {
                        character,
                        byte_offset,
                    });
                }
                let columns = measured_columns(value, width_mode, value.len())?;
                Ok(Self {
                    value: Rc::from(value),
                    columns,
                })
            }

            fn trusted_ascii(value: &'static str) -> Self {
                Self {
                    value: Rc::from(value),
                    columns: u32::try_from(value.len()).expect("trusted text run fits in u32"),
                }
            }

            /// The original UTF-8 text.
            pub fn value(&self) -> &str {
                &self.value
            }

            /// The display width fixed when this run was constructed.
            pub fn columns(&self) -> u32 {
                self.columns
            }
        }

        pub(crate) fn measured_columns(
            value: &str,
            width_mode: WidthMode,
            byte_offset: usize,
        ) -> Result<u32, TextError> {
            let width = match width_mode {
                WidthMode::Narrow => UnicodeWidthStr::width(value),
                WidthMode::Cjk => UnicodeWidthStr::width_cjk(value),
            };
            columns_from_width(width, byte_offset)
        }

        fn columns_from_width(width: usize, byte_offset: usize) -> Result<u32, TextError> {
            u32::try_from(width).map_err(|_| TextError::DisplayWidthOverflow { byte_offset })
        }

        /// Fallible constructor for untrusted single-line text.
        pub fn try_text(value: impl AsRef<str>) -> Result<Rc<Doc>, TextError> {
            try_text_with(value, WidthMode::Narrow)
        }

        /// Fallible constructor for untrusted text under an explicit width policy.
        pub fn try_text_with(
            value: impl AsRef<str>,
            width_mode: WidthMode,
        ) -> Result<Rc<Doc>, TextError> {
            Ok(Rc::new(Doc::Text(TextRun::try_new(
                value.as_ref(),
                width_mode,
            )?)))
        }

        #[non_exhaustive]
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub enum Doc {
            Empty,
            /// Validated text with an authoritative display width.
            Text(TextRun),
            /// A line break. `flat` is the flat projection used by `flatten`:
            /// `Some(" ")` for `line`, `Some("")` for `softline`, `None` for
            /// `hardline` (no flat form exists).
            Line {
                flat: Option<TextRun>,
            },
            Concat(Rc<Doc>, Rc<Doc>),
            /// Adds to the indentation applied after line breaks inside.
            Nest(u16, Rc<Doc>),
            /// Sets indentation after line breaks inside to the current column.
            ///
            /// This is the classic `align` combinator. Unlike [`nest`], its
            /// indentation is determined when layout reaches the node rather than
            /// when the document is constructed.
            Align(Rc<Doc>),
            /// Layout alternative. Engines prefer the left branch on cost ties.
            Choice(Rc<Doc>, Rc<Doc>),
            Tag(TagId, Rc<Doc>),
            /// Add branch-local cost without occupying terminal columns.
            Penalty {
                amount: u32,
                doc: Rc<Doc>,
            },
        }

        pub fn empty() -> Rc<Doc> {
            Rc::new(Doc::Empty)
        }

        /// Convenience constructor for trusted literals.
        ///
        /// Panics with the corresponding [`TextError`] when `value` contains a
        /// control character or cannot be measured.
        pub fn text(value: impl AsRef<str>) -> Rc<Doc> {
            try_text(value).unwrap_or_else(|error| panic!("{error}"))
        }

        pub fn line() -> Rc<Doc> {
            Rc::new(Doc::Line {
                flat: Some(TextRun::trusted_ascii(" ")),
            })
        }

        pub fn softline() -> Rc<Doc> {
            Rc::new(Doc::Line {
                flat: Some(TextRun::trusted_ascii("")),
            })
        }

        pub fn hardline() -> Rc<Doc> {
            Rc::new(Doc::Line { flat: None })
        }

        pub fn concat2(a: Rc<Doc>, b: Rc<Doc>) -> Rc<Doc> {
            Rc::new(Doc::Concat(a, b))
        }

        pub fn concat(docs: impl IntoIterator<Item = Rc<Doc>>) -> Rc<Doc> {
            let mut iter = docs.into_iter();
            let first = match iter.next() {
                Some(d) => d,
                None => return empty(),
            };
            iter.fold(first, concat2)
        }

        pub fn nest(n: u16, d: Rc<Doc>) -> Rc<Doc> {
            Rc::new(Doc::Nest(n, d))
        }

        pub fn align(d: Rc<Doc>) -> Rc<Doc> {
            Rc::new(Doc::Align(d))
        }

        pub fn choice(preferred: Rc<Doc>, alternative: Rc<Doc>) -> Rc<Doc> {
            Rc::new(Doc::Choice(preferred, alternative))
        }

        pub fn tag(t: TagId, d: Rc<Doc>) -> Rc<Doc> {
            Rc::new(Doc::Tag(t, d))
        }

        /// Add branch-local burden to `doc` without changing its rendered text.
        pub fn penalize(amount: u32, doc: Rc<Doc>) -> Rc<Doc> {
            if amount == 0 {
                doc
            } else {
                Rc::new(Doc::Penalty { amount, doc })
            }
        }

        pub fn join(sep: Rc<Doc>, docs: impl IntoIterator<Item = Rc<Doc>>) -> Rc<Doc> {
            let mut out: Option<Rc<Doc>> = None;
            for d in docs {
                out = Some(match out {
                    None => d,
                    Some(acc) => concat2(concat2(acc, sep.clone()), d),
                });
            }
            out.unwrap_or_else(empty)
        }

        /// The flat projection: every `line` a space, every `softline` nothing.
        /// Returns `None` when the document contains a `hardline` (no flat form).
        pub fn flatten(d: &Rc<Doc>) -> Option<Rc<Doc>> {
            match &**d {
                Doc::Empty | Doc::Text(_) => Some(d.clone()),
                Doc::Line { flat: Some(run) } => Some(Rc::new(Doc::Text(run.clone()))),
                Doc::Line { flat: None } => None,
                Doc::Concat(a, b) => Some(concat2(flatten(a)?, flatten(b)?)),
                // Indentation only manifests after breaks; a flat form has none.
                Doc::Nest(_, inner) => flatten(inner),
                Doc::Align(inner) => flatten(inner).map(align),
                Doc::Choice(preferred, _) => flatten(preferred),
                Doc::Tag(t, inner) => Some(tag(*t, flatten(inner)?)),
                Doc::Penalty { amount, doc } => Some(penalize(*amount, flatten(doc)?)),
            }
        }

        /// `group(d)`: prefer the flat projection of `d`, else `d` broken.
        /// A document with no flat form is returned unchanged.
        pub fn group(d: Rc<Doc>) -> Rc<Doc> {
            match flatten(&d) {
                Some(flat) => choice(flat, d),
                None => d,
            }
        }

        /// Number of `Choice` nodes; the brute-force oracle is exponential in this.
        pub fn count_choices(d: &Doc) -> usize {
            match d {
                Doc::Empty | Doc::Text(_) | Doc::Line { .. } => 0,
                Doc::Concat(a, b) => count_choices(a) + count_choices(b),
                Doc::Nest(_, inner)
                | Doc::Align(inner)
                | Doc::Tag(_, inner)
                | Doc::Penalty { doc: inner, .. } => count_choices(inner),
                Doc::Choice(l, r) => 1 + count_choices(l) + count_choices(r),
            }
        }

        #[cfg(any())]
        mod tests {
            use super::{columns_from_width, TextError};

            #[test]
            #[cfg(target_pointer_width = "64")]
            fn width_overflow_is_a_typed_error_with_its_source_offset() {
                let width = usize::try_from(u64::from(u32::MAX) + 1).unwrap();
                assert_eq!(
                    columns_from_width(width, 17),
                    Err(TextError::DisplayWidthOverflow { byte_offset: 17 })
                );
            }
        }
    }
    pub mod cost {
        //! Parametric cost models, in the style of "A Pretty Expressive Printer"
        //! (Porncharoenwase, Nguyen, Torlak; OOPSLA 2023).
        //!
        //! A cost model assigns a cost to placing a text run at a column and to
        //! taking a line break; total cost is the sum over a rendering. The critical
        //! contract is *incrementality*: placing one run of width `a + b` at column
        //! `c` must cost exactly `text(c, a) + text(c + a, b)`, so that engines that
        //! see text in different-sized pieces agree on cost.

        use std::fmt::Debug;

        use num_bigint::BigUint;
        use unicode_width::UnicodeWidthStr;

        /// Additive cost interface used by all rendering engines.
        ///
        /// Arbitrary implementations are accepted by the brute-force and greedy
        /// engines, which do not use algebraic laws to discard layouts. The exact
        /// frontier accepts only [`LawfulCostModel`], whose implementations are sealed
        /// to the models proved in `docs/cost-model-proofs.md`.
        pub trait CostModel {
            type Cost: Clone + Ord + Debug;

            fn zero(&self) -> Self::Cost;
            fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost;
            /// Cost of placing `width` display columns of text starting at `col`.
            fn text(&self, col: u32, width: u32) -> Self::Cost;
            /// Cost of one line break.
            fn newline(&self) -> Self::Cost;
            /// Cost of an explicit branch-local stylistic penalty.
            fn penalty(&self, amount: u32) -> Self::Cost;
        }

        mod sealed {
            pub trait Sealed {}
        }

        /// A cost model whose laws are established for exact Pareto-frontier pruning.
        ///
        /// The laws are:
        ///
        /// - `zero` is a two-sided identity for associative `add`;
        /// - `add` is monotone in both arguments;
        /// - `text` is incremental under splitting;
        /// - `text(c, width)` is nondecreasing in `c`;
        /// - primitive text, newline, and penalty costs are nonnegative; and
        /// - penalty cost depends only on its amount.
        ///
        /// This trait is sealed because Rust trait bounds cannot express or verify
        /// those semantic laws for arbitrary downstream implementations. See the
        /// machine-checked obligations and mathematical derivation in
        /// `docs/cost-model-proofs.md`.
        ///
        /// ```compile_fail
        /// use laidout::{CostModel, LawfulCostModel};
        ///
        /// #[derive(Debug)]
        /// struct External;
        ///
        /// impl CostModel for External {
        ///     type Cost = u64;
        ///     fn zero(&self) -> u64 { 0 }
        ///     fn add(&self, a: &u64, b: &u64) -> u64 { a + b }
        ///     fn text(&self, _col: u32, width: u32) -> u64 { u64::from(width) }
        ///     fn newline(&self) -> u64 { 1 }
        ///     fn penalty(&self, amount: u32) -> u64 { u64::from(amount) }
        /// }
        ///
        /// impl LawfulCostModel for External {}
        /// ```
        pub trait LawfulCostModel: CostModel + sealed::Sealed {}

        /// Default model: lexicographic (squared overflow area, line breaks).
        ///
        /// Overflow past `width` is squared per line, which telescopes under the
        /// incrementality contract: `text(c, w)` contributes
        /// `over(c + w)^2 - over(c)^2`, so a full line ending at column `e` costs
        /// exactly `over(e)^2` regardless of how the line was split into runs.
        /// Height is a strictly weaker criterion, so the printer never trades
        /// overflow for fewer lines.
        #[derive(Clone, Copy, Debug)]
        pub struct OverflowThenHeight {
            pub width: u32,
        }

        impl OverflowThenHeight {
            fn over(&self, col: u32) -> u64 {
                u64::from(col.saturating_sub(self.width))
            }
        }

        impl CostModel for OverflowThenHeight {
            type Cost = (BigUint, u32);

            fn zero(&self) -> Self::Cost {
                (BigUint::from(0u8), 0)
            }

            fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost {
                (&a.0 + &b.0, a.1.saturating_add(b.1))
            }

            fn text(&self, col: u32, width: u32) -> Self::Cost {
                let start = self.over(col);
                let end = self.over(col.checked_add(width).expect("cost-model column overflow"));
                (BigUint::from(end * end - start * start), 0)
            }

            fn newline(&self) -> Self::Cost {
                (BigUint::from(0u8), 1)
            }

            fn penalty(&self, _amount: u32) -> Self::Cost {
                (BigUint::from(0u8), 0)
            }
        }

        impl sealed::Sealed for OverflowThenHeight {}
        impl LawfulCostModel for OverflowThenHeight {}

        /// Cost used by the high-level consumer renderer.
        ///
        /// Overflow is lexicographically dominant. `burden` combines line breaks and
        /// explicit branch-local penalties in documented burden units.
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct ConsumerCost {
            /// Exact, unbounded squared-overflow area.
            pub overflow: BigUint,
            pub burden: u64,
        }

        /// Built-in practical cost model used by [`super::render()`].
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct ConsumerCostModel {
            width: u32,
            newline_cost: u32,
        }

        impl ConsumerCostModel {
            pub fn new(width: u32) -> Self {
                Self {
                    width,
                    newline_cost: 1,
                }
            }

            pub fn with_newline_cost(mut self, newline_cost: u32) -> Self {
                self.newline_cost = newline_cost;
                self
            }

            pub fn width(&self) -> u32 {
                self.width
            }

            pub fn newline_cost(&self) -> u32 {
                self.newline_cost
            }

            fn over(&self, col: u32) -> u64 {
                u64::from(col.saturating_sub(self.width))
            }
        }

        impl CostModel for ConsumerCostModel {
            type Cost = ConsumerCost;

            fn zero(&self) -> Self::Cost {
                ConsumerCost {
                    overflow: BigUint::from(0u8),
                    burden: 0,
                }
            }

            fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost {
                ConsumerCost {
                    overflow: &a.overflow + &b.overflow,
                    burden: a.burden.saturating_add(b.burden),
                }
            }

            fn text(&self, col: u32, width: u32) -> Self::Cost {
                let start = self.over(col);
                let end = self.over(col.checked_add(width).expect("cost-model column overflow"));
                ConsumerCost {
                    overflow: BigUint::from(end * end - start * start),
                    burden: 0,
                }
            }

            fn newline(&self) -> Self::Cost {
                ConsumerCost {
                    overflow: BigUint::from(0u8),
                    burden: u64::from(self.newline_cost),
                }
            }

            fn penalty(&self, amount: u32) -> Self::Cost {
                ConsumerCost {
                    overflow: BigUint::from(0u8),
                    burden: u64::from(amount),
                }
            }
        }

        impl sealed::Sealed for ConsumerCostModel {}
        impl LawfulCostModel for ConsumerCostModel {}

        /// Terminal display width of a text run according to Unicode width rules.
        pub fn display_width(s: &str) -> u32 {
            u32::try_from(UnicodeWidthStr::width(s)).expect("text display width exceeds u32::MAX")
        }

        #[cfg(any())]
        mod tests {
            use num_bigint::BigUint;

            use super::{display_width, ConsumerCost, ConsumerCostModel, CostModel};

            #[test]
            fn display_width_counts_terminal_columns() {
                assert_eq!(display_width("ascii"), 5);
                assert_eq!(display_width("界"), 2);
                assert_eq!(display_width("e\u{301}"), 1);
            }

            #[test]
            fn dominant_accumulation_preserves_lexicographic_order_past_u64_max() {
                let model = ConsumerCostModel::new(80);
                let less = ConsumerCost {
                    overflow: BigUint::from(0u8),
                    burden: u64::MAX,
                };
                let greater = ConsumerCost {
                    overflow: BigUint::from(1u8),
                    burden: 0,
                };
                let increment = ConsumerCost {
                    overflow: BigUint::from(u64::MAX),
                    burden: 0,
                };

                assert!(less < greater);
                assert!(model.add(&less, &increment) < model.add(&greater, &increment));

                let research = super::OverflowThenHeight { width: 80 };
                let less = (BigUint::from(0u8), u32::MAX);
                let greater = (BigUint::from(1u8), 0);
                let increment = (BigUint::from(u64::MAX), 0);
                assert!(less < greater);
                assert!(research.add(&less, &increment) < research.add(&greater, &increment));
            }
        }
    }
    pub mod render {
        //! Shared output events and the high-level consumer rendering facade.
        //!
        //! Engines preserve annotations and penalties structurally in [`Out`]. One
        //! iterative materialization walk produces UTF-8 text, nested byte spans, and
        //! reconstructed cost, keeping the consumer path independent of rope shape.

        use std::ops::Range;
        use std::rc::Rc;

        use super::cost::{ConsumerCost, ConsumerCostModel, CostModel, LawfulCostModel};
        use super::doc::{Doc, TagId, TextRun};
        use super::frontier::{best_with_limits, RenderError, SolveLimits, SolveStats};

        #[non_exhaustive]
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum Out {
            Empty,
            Text(TextRun),
            Newline(u32),
            Cat(Rc<Out>, Rc<Out>),
            Tagged(TagId, Rc<Out>),
            Penalty(u32),
        }

        pub fn out_empty() -> Rc<Out> {
            Rc::new(Out::Empty)
        }

        pub fn out_text(run: TextRun) -> Rc<Out> {
            Rc::new(Out::Text(run))
        }

        pub fn out_newline(indent: u32) -> Rc<Out> {
            Rc::new(Out::Newline(indent))
        }

        pub fn out_cat(a: Rc<Out>, b: Rc<Out>) -> Rc<Out> {
            match (&*a, &*b) {
                (Out::Empty, _) => b,
                (_, Out::Empty) => a,
                _ => Rc::new(Out::Cat(a, b)),
            }
        }

        pub fn out_tagged(tag: TagId, out: Rc<Out>) -> Rc<Out> {
            Rc::new(Out::Tagged(tag, out))
        }

        pub fn out_penalty(amount: u32) -> Rc<Out> {
            Rc::new(Out::Penalty(amount))
        }

        /// One lossless annotation in document preorder.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct AnnotationSpan {
            pub tag: TagId,
            pub range: Range<usize>,
            pub parent: Option<usize>,
        }

        /// A rendered text slice plus its active outer-to-inner annotation stack.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct AnnotatedRun<'a> {
            pub text: &'a str,
            pub range: Range<usize>,
            pub tags: Vec<TagId>,
        }

        /// Consumer-owned rendering independent of the source document and solver.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct Rendered<C> {
            pub text: String,
            pub spans: Vec<AnnotationSpan>,
            pub cost: C,
            pub stats: SolveStats,
        }

        impl<C> Rendered<C> {
            /// Split rendered text at annotation boundaries.
            ///
            /// Runs are nonempty and cover the text exactly once. Tag IDs are ordered
            /// from the outermost active annotation to the innermost.
            pub fn annotated_runs(&self) -> Vec<AnnotatedRun<'_>> {
                if self.text.is_empty() {
                    return Vec::new();
                }

                let mut boundaries = Vec::with_capacity(self.spans.len() * 2 + 2);
                boundaries.push(0);
                boundaries.push(self.text.len());
                for span in &self.spans {
                    boundaries.push(span.range.start);
                    boundaries.push(span.range.end);
                }
                boundaries.sort_unstable();
                boundaries.dedup();

                boundaries
                    .windows(2)
                    .filter_map(|boundary| {
                        let range = boundary[0]..boundary[1];
                        if range.is_empty() {
                            return None;
                        }
                        let tags = self
                            .spans
                            .iter()
                            .filter(|span| {
                                span.range.start <= range.start && range.end <= span.range.end
                            })
                            .map(|span| span.tag)
                            .collect();
                        Some(AnnotatedRun {
                            text: &self.text[range.clone()],
                            range,
                            tags,
                        })
                    })
                    .collect()
            }
        }

        /// Options for the built-in exact consumer renderer.
        #[non_exhaustive]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct RenderOptions {
            width: u32,
            newline_cost: u32,
            limits: SolveLimits,
        }

        impl RenderOptions {
            pub fn new(width: u32) -> Self {
                Self {
                    width,
                    newline_cost: 1,
                    limits: SolveLimits::default(),
                }
            }

            pub fn with_newline_cost(mut self, newline_cost: u32) -> Self {
                self.newline_cost = newline_cost;
                self
            }

            pub fn with_limits(mut self, limits: SolveLimits) -> Self {
                self.limits = limits;
                self
            }

            pub fn width(&self) -> u32 {
                self.width
            }

            pub fn newline_cost(&self) -> u32 {
                self.newline_cost
            }

            pub fn limits(&self) -> SolveLimits {
                self.limits
            }
        }

        /// Render with the practical built-in cost model and exact frontier solver.
        pub fn render(
            doc: &Rc<Doc>,
            options: &RenderOptions,
        ) -> Result<Rendered<ConsumerCost>, RenderError> {
            let model =
                ConsumerCostModel::new(options.width).with_newline_cost(options.newline_cost);
            render_with(doc, &model, options.limits)
        }

        /// Render exactly with a cost model whose frontier laws are proved.
        pub fn render_with<M: LawfulCostModel>(
            doc: &Rc<Doc>,
            model: &M,
            limits: SolveLimits,
        ) -> Result<Rendered<M::Cost>, RenderError> {
            let solved = best_with_limits(model, doc, limits)?;
            let rendered = materialize(model, &solved.candidate.out, solved.stats);
            debug_assert_eq!(solved.candidate.cost, rendered.cost);
            Ok(rendered)
        }

        enum Walk<'a> {
            Visit(&'a Out),
            CloseTag(usize),
        }

        pub(crate) fn materialize<M: CostModel>(
            cm: &M,
            out: &Out,
            stats: SolveStats,
        ) -> Rendered<M::Cost> {
            let mut text = String::new();
            let mut spans: Vec<AnnotationSpan> = Vec::new();
            let mut open_tags: Vec<usize> = Vec::new();
            let mut cost = cm.zero();
            let mut col = 0u32;
            let mut stack = vec![Walk::Visit(out)];

            while let Some(action) = stack.pop() {
                match action {
                    Walk::Visit(Out::Empty) => {}
                    Walk::Visit(Out::Text(run)) => {
                        cost = cm.add(&cost, &cm.text(col, run.columns()));
                        col = col
                            .checked_add(run.columns())
                            .expect("verified output column overflow");
                        text.push_str(run.value());
                    }
                    Walk::Visit(Out::Newline(indent)) => {
                        cost = cm.add(&cost, &cm.newline());
                        cost = cm.add(&cost, &cm.text(0, *indent));
                        text.push('\n');
                        for _ in 0..*indent {
                            text.push(' ');
                        }
                        col = *indent;
                    }
                    Walk::Visit(Out::Cat(left, right)) => {
                        stack.push(Walk::Visit(right));
                        stack.push(Walk::Visit(left));
                    }
                    Walk::Visit(Out::Tagged(tag, inner)) => {
                        let index = spans.len();
                        spans.push(AnnotationSpan {
                            tag: *tag,
                            range: text.len()..text.len(),
                            parent: open_tags.last().copied(),
                        });
                        open_tags.push(index);
                        stack.push(Walk::CloseTag(index));
                        stack.push(Walk::Visit(inner));
                    }
                    Walk::Visit(Out::Penalty(amount)) => {
                        cost = cm.add(&cost, &cm.penalty(*amount));
                    }
                    Walk::CloseTag(index) => {
                        let popped = open_tags.pop();
                        debug_assert_eq!(popped, Some(index));
                        spans[index].range.end = text.len();
                    }
                }
            }

            debug_assert!(open_tags.is_empty());
            debug_assert!(spans.iter().all(|span| {
                span.range.start <= span.range.end
                    && span.range.end <= text.len()
                    && text.is_char_boundary(span.range.start)
                    && text.is_char_boundary(span.range.end)
                    && span.parent.is_none_or(|parent| {
                        parent < spans.len()
                            && spans[parent].range.start <= span.range.start
                            && span.range.end <= spans[parent].range.end
                    })
            }));

            Rendered {
                text,
                spans,
                cost,
                stats,
            }
        }

        /// Canonical reconstruction of every semantic cost event in an output rope.
        pub fn cost_of_out<M: CostModel>(cm: &M, out: &Out) -> M::Cost {
            materialize(cm, out, SolveStats::default()).cost
        }

        /// Flatten a rope into lines. Each line already includes its indentation.
        pub fn to_lines(out: &Out) -> Vec<String> {
            to_string(out).split('\n').map(str::to_owned).collect()
        }

        pub fn to_string(out: &Out) -> String {
            let mut text = String::new();
            let mut stack = vec![out];
            while let Some(event) = stack.pop() {
                match event {
                    Out::Empty | Out::Penalty(_) => {}
                    Out::Text(run) => text.push_str(run.value()),
                    Out::Newline(indent) => {
                        text.push('\n');
                        for _ in 0..*indent {
                            text.push(' ');
                        }
                    }
                    Out::Cat(left, right) => {
                        stack.push(right);
                        stack.push(left);
                    }
                    Out::Tagged(_, inner) => stack.push(inner),
                }
            }
            text
        }

        /// The whitespace-insensitive content of a rendering: every engine and every
        /// layout of the same document must agree on this.
        pub fn words(s: &str) -> Vec<String> {
            s.split_whitespace().map(str::to_owned).collect()
        }
    }
    pub mod frontier {
        //! Exact memoized preference-aware Pareto-frontier search.
        //!
        //! `solve(doc, col, indent)` returns layouts of `doc` starting at `col`,
        //! pruned by `(cost, last-column)` dominance while retaining equal-cost states
        //! needed to preserve left-branch ties. Documents are structurally
        //! interned, and memo keys contain only document identity, column, and
        //! indentation. Tags and penalties remain ordinary structural output events.

        use std::collections::HashMap;
        use std::error::Error;
        use std::fmt;
        use std::hash::{Hash, Hasher};
        use std::rc::Rc;

        use super::cost::LawfulCostModel;
        use super::doc::{Doc, TagId};
        use super::render::{
            out_cat, out_empty, out_newline, out_penalty, out_tagged, out_text, Out,
        };

        #[derive(Clone, Debug)]
        pub struct Cand<C> {
            pub cost: C,
            pub last: u32,
            pub out: Rc<Out>,
        }

        /// Deterministic work counters for an exact solve.
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct SolveStats {
            /// Every subproblem invocation, including memo hits.
            pub solve_calls: u64,
            /// Subproblem invocations satisfied from a completed memo entry.
            pub memo_hits: u64,
            /// Subproblem invocations that required evaluation.
            pub memo_misses: u64,
            /// Candidates emitted by a node before that node's pruning pass.
            pub candidates_generated: u64,
            /// Emitted candidates absent from the retained preference-aware frontier.
            pub candidates_pruned: u64,
            /// Largest retained frontier for one completed subproblem.
            pub peak_frontier: usize,
            /// Completed memo entries retained at success or failure.
            pub memo_entries: usize,
            /// Structurally distinct documents interned at success or failure.
            pub interned_documents: usize,
        }

        /// Deterministic exact-solver resource limits. `None` means unlimited.
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct SolveLimits {
            /// Maximum permitted solve invocations, including memo hits.
            pub max_solve_calls: Option<u64>,
            /// Maximum permitted node-emitted candidates before pruning.
            pub max_candidates: Option<u64>,
            /// Maximum permitted completed memo entries.
            pub max_memo_entries: Option<usize>,
        }

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum SolveLimitKind {
            SolveCalls,
            Candidates,
            MemoEntries,
        }

        #[non_exhaustive]
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum RenderError {
            LimitExceeded {
                kind: SolveLimitKind,
                configured: u64,
                stats: SolveStats,
            },
            ColumnOverflow {
                column: u32,
                width: u32,
                stats: SolveStats,
            },
        }

        impl fmt::Display for RenderError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::LimitExceeded {
                        kind,
                        configured,
                        stats,
                    } => write!(
                        f,
                        "exact render exceeded {kind:?} limit {configured}; current statistics: {stats:?}"
                    ),
                    Self::ColumnOverflow {
                        column,
                        width,
                        stats,
                    } => write!(
                        f,
                        "exact render column overflow while adding width {width} at column {column}; current statistics: {stats:?}"
                    ),
                }
            }
        }

        impl Error for RenderError {}

        #[derive(Clone, Debug)]
        pub struct Solved<C> {
            pub candidate: Cand<C>,
            pub stats: SolveStats,
        }

        #[derive(Clone, Eq, Hash, PartialEq)]
        struct Key {
            doc_id: usize,
            col: u32,
            indent: u32,
        }

        #[derive(Clone)]
        struct DocKey(Rc<Doc>);

        impl PartialEq for DocKey {
            fn eq(&self, other: &Self) -> bool {
                if Rc::ptr_eq(&self.0, &other.0) {
                    return true;
                }
                let mut stack = vec![(self.0.as_ref(), other.0.as_ref())];
                while let Some((left, right)) = stack.pop() {
                    match (left, right) {
                        (Doc::Empty, Doc::Empty) => {}
                        (Doc::Text(left), Doc::Text(right)) if left == right => {}
                        (Doc::Line { flat: left }, Doc::Line { flat: right }) if left == right => {}
                        (Doc::Concat(la, lb), Doc::Concat(ra, rb))
                        | (Doc::Choice(la, lb), Doc::Choice(ra, rb)) => {
                            stack.push((lb, rb));
                            stack.push((la, ra));
                        }
                        (Doc::Nest(ln, left), Doc::Nest(rn, right)) if ln == rn => {
                            stack.push((left, right));
                        }
                        (Doc::Align(left), Doc::Align(right)) => stack.push((left, right)),
                        (Doc::Tag(lt, left), Doc::Tag(rt, right)) if lt == rt => {
                            stack.push((left, right));
                        }
                        (
                            Doc::Penalty {
                                amount: la,
                                doc: left,
                            },
                            Doc::Penalty {
                                amount: ra,
                                doc: right,
                            },
                        ) if la == ra => stack.push((left, right)),
                        _ => return false,
                    }
                }
                true
            }
        }

        impl Eq for DocKey {}

        impl Hash for DocKey {
            fn hash<H: Hasher>(&self, state: &mut H) {
                let mut stack = vec![self.0.as_ref()];
                while let Some(doc) = stack.pop() {
                    match doc {
                        Doc::Empty => 0u8.hash(state),
                        Doc::Text(run) => {
                            1u8.hash(state);
                            run.hash(state);
                        }
                        Doc::Line { flat } => {
                            2u8.hash(state);
                            flat.hash(state);
                        }
                        Doc::Concat(left, right) => {
                            3u8.hash(state);
                            stack.push(right);
                            stack.push(left);
                        }
                        Doc::Nest(amount, inner) => {
                            4u8.hash(state);
                            amount.hash(state);
                            stack.push(inner);
                        }
                        Doc::Align(inner) => {
                            5u8.hash(state);
                            stack.push(inner);
                        }
                        Doc::Choice(left, right) => {
                            6u8.hash(state);
                            stack.push(right);
                            stack.push(left);
                        }
                        Doc::Tag(tag, inner) => {
                            7u8.hash(state);
                            tag.hash(state);
                            stack.push(inner);
                        }
                        Doc::Penalty { amount, doc } => {
                            8u8.hash(state);
                            amount.hash(state);
                            stack.push(doc);
                        }
                    }
                }
            }
        }

        #[derive(Clone, Copy)]
        enum Unary {
            Pass,
            Tag(TagId),
            Penalty(u32),
        }

        enum WorkFor<C> {
            Eval {
                doc: Rc<Doc>,
                col: u32,
                indent: u32,
            },
            CompleteUnary {
                key: Key,
                child: Key,
                kind: Unary,
            },
            AfterConcatLeft {
                key: Key,
                left: Key,
                right: Rc<Doc>,
                indent: u32,
            },
            CompleteConcat {
                key: Key,
                left: Rc<Vec<Cand<C>>>,
                right: Rc<Doc>,
                indent: u32,
            },
            CompleteChoice {
                key: Key,
                preferred: Key,
                alternative: Key,
            },
        }

        pub struct Engine<'a, M: LawfulCostModel> {
            cm: &'a M,
            limits: SolveLimits,
            stats: SolveStats,
            doc_ids: HashMap<DocKey, usize>,
            next_doc_id: usize,
            memo: HashMap<Key, Rc<Vec<Cand<M::Cost>>>>,
        }

        impl<'a, M: LawfulCostModel> Engine<'a, M> {
            pub fn new(cm: &'a M) -> Self {
                Self::with_limits(cm, SolveLimits::default())
            }

            pub fn with_limits(cm: &'a M, limits: SolveLimits) -> Self {
                Self {
                    cm,
                    limits,
                    stats: SolveStats::default(),
                    doc_ids: HashMap::new(),
                    next_doc_id: 0,
                    memo: HashMap::new(),
                }
            }

            pub fn stats(&self) -> SolveStats {
                self.stats
            }

            fn limit_error(&self, kind: SolveLimitKind, configured: u64) -> RenderError {
                RenderError::LimitExceeded {
                    kind,
                    configured,
                    stats: self.stats,
                }
            }

            fn add_columns(&self, column: u32, width: u32) -> Result<u32, RenderError> {
                column
                    .checked_add(width)
                    .ok_or(RenderError::ColumnOverflow {
                        column,
                        width,
                        stats: self.stats,
                    })
            }

            fn record_solve_call(&mut self) -> Result<(), RenderError> {
                self.stats.solve_calls = self.stats.solve_calls.saturating_add(1);
                if let Some(limit) = self.limits.max_solve_calls {
                    if self.stats.solve_calls > limit {
                        return Err(self.limit_error(SolveLimitKind::SolveCalls, limit));
                    }
                }
                Ok(())
            }

            fn record_candidate(&mut self) -> Result<(), RenderError> {
                self.stats.candidates_generated = self.stats.candidates_generated.saturating_add(1);
                if let Some(limit) = self.limits.max_candidates {
                    if self.stats.candidates_generated > limit {
                        return Err(self.limit_error(SolveLimitKind::Candidates, limit));
                    }
                }
                Ok(())
            }

            fn intern(&mut self, doc: &Rc<Doc>) -> usize {
                let lookup = DocKey(doc.clone());
                if let Some(id) = self.doc_ids.get(&lookup) {
                    return *id;
                }
                let id = self.next_doc_id;
                self.next_doc_id += 1;
                self.doc_ids.insert(lookup, id);
                self.stats.interned_documents = self.doc_ids.len();
                id
            }

            fn key(&mut self, doc: &Rc<Doc>, col: u32, indent: u32) -> Key {
                Key {
                    doc_id: self.intern(doc),
                    col,
                    indent,
                }
            }

            fn complete(
                &mut self,
                key: Key,
                candidates: Vec<Cand<M::Cost>>,
            ) -> Result<(), RenderError> {
                let before = candidates.len();
                let mut kept: Vec<Cand<M::Cost>> = Vec::with_capacity(before);
                for candidate in candidates {
                    // Keep generation order so equal total costs preserve the
                    // document algebra's left/preferred bias. A strictly cheaper
                    // candidate ending no farther right dominates. At the exact same
                    // last column, equality also dominates because the earlier
                    // candidate has identical continuation behavior.
                    let dominated = kept.iter().any(|previous| {
                        (previous.last == candidate.last && previous.cost <= candidate.cost)
                            || (previous.last < candidate.last && previous.cost < candidate.cost)
                    });
                    if dominated {
                        continue;
                    }

                    kept.retain(|previous| {
                        !(candidate.last <= previous.last && candidate.cost < previous.cost)
                    });
                    kept.push(candidate);
                }
                self.stats.candidates_pruned = self
                    .stats
                    .candidates_pruned
                    .saturating_add(u64::try_from(before - kept.len()).unwrap_or(u64::MAX));
                self.stats.peak_frontier = self.stats.peak_frontier.max(kept.len());

                if let Some(limit) = self.limits.max_memo_entries {
                    if self.memo.len() >= limit {
                        return Err(self.limit_error(
                            SolveLimitKind::MemoEntries,
                            u64::try_from(limit).unwrap_or(u64::MAX),
                        ));
                    }
                }
                self.memo.insert(key, Rc::new(kept));
                self.stats.memo_entries = self.memo.len();
                Ok(())
            }

            /// Solve one exact subproblem, accumulating deterministic engine stats.
            pub fn solve(
                &mut self,
                doc: &Rc<Doc>,
                col: u32,
                indent: u32,
            ) -> Result<Rc<Vec<Cand<M::Cost>>>, RenderError> {
                let root = self.key(doc, col, indent);
                let mut work = vec![WorkFor::Eval {
                    doc: doc.clone(),
                    col,
                    indent,
                }];

                while let Some(item) = work.pop() {
                    match item {
                        WorkFor::Eval { doc, col, indent } => {
                            self.record_solve_call()?;
                            let key = self.key(&doc, col, indent);
                            if self.memo.contains_key(&key) {
                                self.stats.memo_hits = self.stats.memo_hits.saturating_add(1);
                                continue;
                            }
                            self.stats.memo_misses = self.stats.memo_misses.saturating_add(1);

                            match &*doc {
                                Doc::Empty => {
                                    self.record_candidate()?;
                                    self.complete(
                                        key,
                                        vec![Cand {
                                            cost: self.cm.zero(),
                                            last: col,
                                            out: out_empty(),
                                        }],
                                    )?;
                                }
                                Doc::Text(run) => {
                                    self.record_candidate()?;
                                    let last = self.add_columns(col, run.columns())?;
                                    self.complete(
                                        key,
                                        vec![Cand {
                                            cost: self.cm.text(col, run.columns()),
                                            last,
                                            out: out_text(run.clone()),
                                        }],
                                    )?;
                                }
                                Doc::Line { .. } => {
                                    self.record_candidate()?;
                                    let cost =
                                        self.cm.add(&self.cm.newline(), &self.cm.text(0, indent));
                                    self.complete(
                                        key,
                                        vec![Cand {
                                            cost,
                                            last: indent,
                                            out: out_newline(indent),
                                        }],
                                    )?;
                                }
                                Doc::Concat(left, right) => {
                                    let left_key = self.key(left, col, indent);
                                    work.push(WorkFor::AfterConcatLeft {
                                        key,
                                        left: left_key,
                                        right: right.clone(),
                                        indent,
                                    });
                                    work.push(WorkFor::Eval {
                                        doc: left.clone(),
                                        col,
                                        indent,
                                    });
                                }
                                Doc::Nest(amount, inner) => {
                                    let child_indent =
                                        self.add_columns(indent, u32::from(*amount))?;
                                    let child = self.key(inner, col, child_indent);
                                    work.push(WorkFor::CompleteUnary {
                                        key,
                                        child,
                                        kind: Unary::Pass,
                                    });
                                    work.push(WorkFor::Eval {
                                        doc: inner.clone(),
                                        col,
                                        indent: child_indent,
                                    });
                                }
                                Doc::Align(inner) => {
                                    let child = self.key(inner, col, col);
                                    work.push(WorkFor::CompleteUnary {
                                        key,
                                        child,
                                        kind: Unary::Pass,
                                    });
                                    work.push(WorkFor::Eval {
                                        doc: inner.clone(),
                                        col,
                                        indent: col,
                                    });
                                }
                                Doc::Choice(preferred, alternative) => {
                                    let preferred_key = self.key(preferred, col, indent);
                                    let alternative_key = self.key(alternative, col, indent);
                                    work.push(WorkFor::CompleteChoice {
                                        key,
                                        preferred: preferred_key,
                                        alternative: alternative_key,
                                    });
                                    work.push(WorkFor::Eval {
                                        doc: alternative.clone(),
                                        col,
                                        indent,
                                    });
                                    work.push(WorkFor::Eval {
                                        doc: preferred.clone(),
                                        col,
                                        indent,
                                    });
                                }
                                Doc::Tag(tag, inner) => {
                                    let child = self.key(inner, col, indent);
                                    work.push(WorkFor::CompleteUnary {
                                        key,
                                        child,
                                        kind: Unary::Tag(*tag),
                                    });
                                    work.push(WorkFor::Eval {
                                        doc: inner.clone(),
                                        col,
                                        indent,
                                    });
                                }
                                Doc::Penalty { amount, doc } => {
                                    let child = self.key(doc, col, indent);
                                    work.push(WorkFor::CompleteUnary {
                                        key,
                                        child,
                                        kind: Unary::Penalty(*amount),
                                    });
                                    work.push(WorkFor::Eval {
                                        doc: doc.clone(),
                                        col,
                                        indent,
                                    });
                                }
                            }
                        }
                        WorkFor::CompleteUnary { key, child, kind } => {
                            let child = self
                                .memo
                                .get(&child)
                                .expect("completed unary child is memoized")
                                .clone();
                            let mut candidates = Vec::with_capacity(child.len());
                            for candidate in child.iter() {
                                self.record_candidate()?;
                                let (cost, out) = match kind {
                                    Unary::Pass => (candidate.cost.clone(), candidate.out.clone()),
                                    Unary::Tag(tag) => (
                                        candidate.cost.clone(),
                                        out_tagged(tag, candidate.out.clone()),
                                    ),
                                    Unary::Penalty(amount) => (
                                        self.cm.add(&self.cm.penalty(amount), &candidate.cost),
                                        out_cat(out_penalty(amount), candidate.out.clone()),
                                    ),
                                };
                                candidates.push(Cand {
                                    cost,
                                    last: candidate.last,
                                    out,
                                });
                            }
                            self.complete(key, candidates)?;
                        }
                        WorkFor::AfterConcatLeft {
                            key,
                            left,
                            right,
                            indent,
                        } => {
                            let left = self
                                .memo
                                .get(&left)
                                .expect("completed concat left child is memoized")
                                .clone();
                            work.push(WorkFor::CompleteConcat {
                                key,
                                left: left.clone(),
                                right: right.clone(),
                                indent,
                            });
                            for candidate in left.iter().rev() {
                                work.push(WorkFor::Eval {
                                    doc: right.clone(),
                                    col: candidate.last,
                                    indent,
                                });
                            }
                        }
                        WorkFor::CompleteConcat {
                            key,
                            left,
                            right,
                            indent,
                        } => {
                            let mut combined = Vec::new();
                            for left_candidate in left.iter() {
                                let right_key = self.key(&right, left_candidate.last, indent);
                                let right_candidates = self
                                    .memo
                                    .get(&right_key)
                                    .expect("completed concat right child is memoized")
                                    .clone();
                                for right_candidate in right_candidates.iter() {
                                    self.record_candidate()?;
                                    combined.push(Cand {
                                        cost: self
                                            .cm
                                            .add(&left_candidate.cost, &right_candidate.cost),
                                        last: right_candidate.last,
                                        out: out_cat(
                                            left_candidate.out.clone(),
                                            right_candidate.out.clone(),
                                        ),
                                    });
                                }
                            }
                            self.complete(key, combined)?;
                        }
                        WorkFor::CompleteChoice {
                            key,
                            preferred,
                            alternative,
                        } => {
                            let preferred = self
                                .memo
                                .get(&preferred)
                                .expect("completed preferred child is memoized")
                                .clone();
                            let alternative = self
                                .memo
                                .get(&alternative)
                                .expect("completed alternative child is memoized")
                                .clone();
                            let mut candidates =
                                Vec::with_capacity(preferred.len() + alternative.len());
                            for candidate in preferred.iter().chain(alternative.iter()) {
                                self.record_candidate()?;
                                candidates.push(candidate.clone());
                            }
                            self.complete(key, candidates)?;
                        }
                    }
                }

                Ok(self
                    .memo
                    .get(&root)
                    .expect("completed root is memoized")
                    .clone())
            }
        }

        /// Exact minimum-cost layout with deterministic resource limits.
        pub fn best_with_limits<M: LawfulCostModel>(
            cm: &M,
            doc: &Rc<Doc>,
            limits: SolveLimits,
        ) -> Result<Solved<M::Cost>, RenderError> {
            let mut engine = Engine::with_limits(cm, limits);
            let frontier = engine.solve(doc, 0, 0)?;
            let mut candidates = frontier.iter();
            let mut candidate = candidates
                .next()
                .expect("a document always has at least one layout")
                .clone();
            for next in candidates {
                if next.cost < candidate.cost {
                    candidate = next.clone();
                }
            }
            Ok(Solved {
                candidate,
                stats: engine.stats(),
            })
        }

        /// Exact minimum-cost layout with unlimited deterministic work.
        pub fn best<M: LawfulCostModel>(cm: &M, doc: &Rc<Doc>) -> Cand<M::Cost> {
            best_with_limits(cm, doc, SolveLimits::default())
                .expect("unlimited exact solving cannot exceed a resource limit")
                .candidate
        }

        #[cfg(any())]
        mod tests {
            use super::cost::OverflowThenHeight;
            use super::doc::{concat2, line, text};
            use crate::table::{table, Column};

            use super::{Engine, RenderError, SolveLimitKind, SolveLimits};

            #[test]
            fn structurally_equal_rebuilt_subtrees_share_memo_entries() {
                let left = concat2(text("same"), line());
                let rebuilt = concat2(text("same"), line());
                assert!(!std::rc::Rc::ptr_eq(&left, &rebuilt));

                let cm = OverflowThenHeight { width: 20 };
                let mut engine = Engine::new(&cm);
                engine.solve(&left, 0, 0).unwrap();
                let after_first = engine.memo.len();
                let docs_after_first = engine.doc_ids.len();
                engine.solve(&rebuilt, 0, 0).unwrap();

                assert_eq!(engine.memo.len(), after_first);
                assert_eq!(engine.doc_ids.len(), docs_after_first);
            }

            #[test]
            fn rebuilt_equal_tables_share_interned_documents_and_memo_entries() {
                let build = || {
                    table([Column::new(), Column::new()])
                        .row([text("left"), text("right")])
                        .row([text("up"), text("down")])
                        .build()
                        .unwrap()
                };
                let left = build();
                let rebuilt = build();
                assert!(!std::rc::Rc::ptr_eq(&left, &rebuilt));
                assert_eq!(left, rebuilt);

                let cm = OverflowThenHeight { width: 20 };
                let mut engine = Engine::new(&cm);
                engine.solve(&left, 0, 0).unwrap();
                let after_first = engine.memo.len();
                let docs_after_first = engine.doc_ids.len();
                engine.solve(&rebuilt, 0, 0).unwrap();

                assert_eq!(engine.memo.len(), after_first);
                assert_eq!(engine.doc_ids.len(), docs_after_first);
            }

            #[test]
            fn limit_abort_never_caches_the_incomplete_root() {
                let doc = concat2(text("left"), text("right"));
                let cm = OverflowThenHeight { width: 20 };
                let mut engine = Engine::with_limits(
                    &cm,
                    SolveLimits {
                        max_candidates: Some(1),
                        ..SolveLimits::default()
                    },
                );
                let root = engine.key(&doc, 0, 0);

                let error = engine.solve(&doc, 0, 0).unwrap_err();
                assert!(matches!(
                    error,
                    RenderError::LimitExceeded {
                        kind: SolveLimitKind::Candidates,
                        configured: 1,
                        ..
                    }
                ));
                assert!(!engine.memo.contains_key(&root));
                assert_eq!(engine.stats.memo_entries, 1);
            }

            #[test]
            fn column_overflow_is_typed_instead_of_clamped() {
                let doc = text("xx");
                let cm = OverflowThenHeight { width: u32::MAX };
                let mut engine = Engine::new(&cm);

                assert!(matches!(
                    engine.solve(&doc, u32::MAX - 1, 0),
                    Err(RenderError::ColumnOverflow {
                        column,
                        width: 2,
                        ..
                    }) if column == u32::MAX - 1
                ));
            }
        }
    }
}

use std::rc::Rc;

use crate::harness_v1::{BenchmarkAdapter, RunResult, Workload};
use frozen::doc::{choice, concat, concat2, empty, group, line, penalize, text, Doc};
use frozen::render::{render, RenderOptions};

pub struct BaselineAdapter;

pub struct BaselineCase {
    doc: Rc<Doc>,
    options: RenderOptions,
}

impl BenchmarkAdapter for BaselineAdapter {
    type Prepared = BaselineCase;

    fn implementation(&self) -> &'static str {
        "laidout-0.1-baseline"
    }

    fn prepare(&self, workload: Workload) -> Result<Self::Prepared, String> {
        let doc = match workload.name {
            "json-20" | "json-40" => json_doc(),
            "unique-sequence-2000" | "unique-sequence-4000" => unique_sequence(workload.size),
            "frontier-64" | "frontier-128" => wide_frontier(workload.size),
            "zero-width-2000" | "zero-width-4000" => zero_width(workload.size),
            other => return Err(format!("unknown workload {other}")),
        };
        Ok(BaselineCase {
            doc,
            options: RenderOptions::new(workload.width),
        })
    }

    fn run(&self, prepared: &mut Self::Prepared) -> Result<RunResult, String> {
        let rendered =
            render(&prepared.doc, &prepared.options).map_err(|error| error.to_string())?;
        Ok(RunResult {
            fingerprint: fingerprint(rendered.text.as_bytes()),
        })
    }
}

fn json_doc() -> Rc<Doc> {
    let comma = concat2(text(","), line());
    group(concat([
        text("{"),
        frozen::doc::nest(
            2,
            concat([
                frozen::doc::softline(),
                frozen::doc::join(
                    comma,
                    [
                        concat([text("\\\"name\\\":"), line(), text("\\\"laidout\\\"")]),
                        concat([text("\\\"unicode\\\":"), line(), text("\\\"界\\\"")]),
                        concat([
                            text("\\\"values\\\":"),
                            line(),
                            group(concat([
                                text("[1,"),
                                line(),
                                text("2,"),
                                line(),
                                text("3]"),
                            ])),
                        ]),
                    ],
                ),
            ]),
        ),
        frozen::doc::softline(),
        text("}"),
    ]))
}

fn unique_sequence(count: usize) -> Rc<Doc> {
    (0..count)
        .map(|index| text(index.to_string()))
        .fold(empty(), concat2)
}

fn wide_frontier(count: usize) -> Rc<Doc> {
    let alternative = |index: usize| {
        penalize(
            u32::try_from(index).expect("benchmark index fits u32"),
            text("x".repeat(count - index)),
        )
    };
    let mut doc = alternative(0);
    for index in 1..count {
        doc = choice(doc, alternative(index));
    }
    doc
}

fn zero_width(count: usize) -> Rc<Doc> {
    concat((0..count).map(|_| group(concat([text("\u{200b}"), line(), text("x"), line()]))))
}

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
