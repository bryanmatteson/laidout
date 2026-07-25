//! Owned and sink rendering built on the reusable prepared kernel.

use std::collections::TryReserveError;
use std::fmt;
use std::io;
use std::ops::Range;
use std::sync::Arc;

use crate::cost::ConsumerCost;
use crate::doc::Doc;
use crate::kernel::{
    render_into, seed_capacity, AnnotationSpan, RenderError, RenderOptions, RenderWorkspace,
    ReserveError, SolveStats,
};
use crate::prepare::PrepareError;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
/// An owned output buffer that failed allocation.
pub enum OwnedOutputResource {
    /// The rendered UTF-8 text.
    Text,
    /// The annotation span table.
    Spans,
    /// The retained annotation values.
    Annotations,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
/// Failure from an allocating convenience renderer.
pub enum OwnedRenderError {
    /// Preparing the input document failed.
    #[error("document preparation failed")]
    Prepare(#[source] PrepareError),
    /// Reserving reusable workspace failed.
    #[error("workspace reservation failed")]
    Reserve(#[source] ReserveError),
    /// Solving or materializing the prepared document failed.
    #[error("document rendering failed")]
    Render(#[source] RenderError),
    /// Reserving an owned result buffer failed.
    #[error("failed to reserve {requested} entries for owned {resource:?} output")]
    OwnedOutputReserve {
        /// The buffer being reserved.
        resource: OwnedOutputResource,
        /// The requested number of bytes or entries.
        requested: usize,
        #[source]
        /// The allocator error.
        source: TryReserveError,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
/// Stable category for an [`OwnedRenderError`].
pub enum OwnedRenderErrorKind {
    /// Document preparation failed.
    Prepare,
    /// Workspace reservation failed.
    Reserve,
    /// Kernel rendering failed.
    Render,
    /// An owned result buffer failed reservation.
    OwnedOutputReserve,
}

impl OwnedRenderError {
    /// Return the stable category of this error.
    pub const fn kind(&self) -> OwnedRenderErrorKind {
        match self {
            Self::Prepare(_) => OwnedRenderErrorKind::Prepare,
            Self::Reserve(_) => OwnedRenderErrorKind::Reserve,
            Self::Render(_) => OwnedRenderErrorKind::Render,
            Self::OwnedOutputReserve { .. } => OwnedRenderErrorKind::OwnedOutputReserve,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Cost and solver telemetry returned by a sink write.
pub struct RenderSummary {
    /// Cost of the selected layout.
    pub cost: ConsumerCost,
    /// Counters collected by the selected solver.
    pub stats: SolveStats,
}

#[derive(Debug)]
#[non_exhaustive]
/// Failure from rendering directly into a caller-provided sink.
pub enum WriteError<E> {
    /// The layout kernel failed before the write completed.
    Render(RenderError),
    /// The destination rejected the rendered text.
    Sink(E),
}

impl<E: fmt::Display> fmt::Display for WriteError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Render(error) => error.fmt(formatter),
            Self::Sink(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for WriteError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Render(error) => Some(error),
            Self::Sink(error) => Some(error),
        }
    }
}

/// An owned rendering, including annotation spans and solver telemetry.
pub struct Rendered<A> {
    text: String,
    spans: Vec<AnnotationSpan>,
    annotations: Vec<Arc<A>>,
    cost: ConsumerCost,
    stats: SolveStats,
}

/// Independently owned fields extracted from a [`Rendered`] value.
pub struct RenderedParts<A> {
    /// Rendered UTF-8 text.
    pub text: String,
    /// Annotation ranges into `text`.
    pub spans: Vec<AnnotationSpan>,
    /// Annotation values addressed by `spans`.
    pub annotations: Vec<Arc<A>>,
    /// Cost of the selected layout.
    pub cost: ConsumerCost,
    /// Solver counters.
    pub stats: SolveStats,
}

impl<A> Rendered<A> {
    /// Return the rendered text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Return annotation spans into [`Self::text`].
    pub fn spans(&self) -> &[AnnotationSpan] {
        &self.spans
    }
    /// Return the cost of the selected layout.
    pub const fn cost(&self) -> ConsumerCost {
        self.cost
    }
    /// Return counters collected while solving.
    pub const fn stats(&self) -> SolveStats {
        self.stats
    }

    /// Iterate over spans paired with their annotation values.
    pub fn resolved_spans(&self) -> OwnedResolvedSpans<'_, A> {
        OwnedResolvedSpans {
            spans: self.spans.iter(),
            annotations: &self.annotations,
        }
    }

    /// Split the text at annotation boundaries and resolve active annotations.
    pub fn annotated_runs(&self) -> Vec<AnnotatedRun<'_, A>> {
        if self.text.is_empty() {
            return Vec::new();
        }
        let mut boundaries =
            Vec::with_capacity(self.spans.len().saturating_mul(2).saturating_add(2));
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
            .filter_map(|pair| {
                let range = pair[0]..pair[1];
                if range.is_empty() {
                    return None;
                }
                let annotations = self
                    .spans
                    .iter()
                    .filter(|span| span.range.start <= range.start && range.end <= span.range.end)
                    .map(|span| self.annotations[span.annotation.0 as usize].as_ref())
                    .collect();
                Some(AnnotatedRun {
                    text: &self.text[range.clone()],
                    range,
                    annotations,
                })
            })
            .collect()
    }

    /// Consume the rendering and return only its text.
    pub fn into_text(self) -> String {
        self.text
    }

    /// Consume the rendering and expose all owned fields.
    pub fn into_parts(self) -> RenderedParts<A> {
        RenderedParts {
            text: self.text,
            spans: self.spans,
            annotations: self.annotations,
            cost: self.cost,
            stats: self.stats,
        }
    }
}

impl<A> Clone for Rendered<A> {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            spans: self.spans.clone(),
            annotations: self.annotations.clone(),
            cost: self.cost,
            stats: self.stats,
        }
    }
}

impl<A: fmt::Debug> fmt::Debug for Rendered<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Rendered")
            .field("text", &self.text)
            .field("spans", &self.spans)
            .field("annotations", &self.annotations)
            .field("cost", &self.cost)
            .field("stats", &self.stats)
            .finish()
    }
}

impl<A: PartialEq> PartialEq for Rendered<A> {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.spans == other.spans
            && self.annotations == other.annotations
            && self.cost == other.cost
            && self.stats == other.stats
    }
}

impl<A: Eq> Eq for Rendered<A> {}

/// A contiguous text run and the annotations active over it.
pub struct AnnotatedRun<'a, A> {
    /// Text covered by this run.
    pub text: &'a str,
    /// Byte range of this run in the complete rendered text.
    pub range: Range<usize>,
    /// Annotation values active over the entire run.
    pub annotations: Vec<&'a A>,
}

pub struct OwnedResolvedSpans<'a, A> {
    spans: std::slice::Iter<'a, AnnotationSpan>,
    annotations: &'a [Arc<A>],
}

impl<'a, A> Iterator for OwnedResolvedSpans<'a, A> {
    type Item = (&'a AnnotationSpan, &'a A);
    fn next(&mut self) -> Option<Self::Item> {
        let span = self.spans.next()?;
        Some((span, self.annotations[span.annotation.0 as usize].as_ref()))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.spans.size_hint()
    }
}

impl<A> ExactSizeIterator for OwnedResolvedSpans<'_, A> {}

/// Stateful owned and sink renderer with reusable workspace.
///
/// Reuse one renderer across documents to amortize workspace allocation.
pub struct Renderer {
    workspace: RenderWorkspace,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    /// Create a renderer backed by a growable workspace.
    pub fn new() -> Self {
        Self {
            workspace: RenderWorkspace::growable(),
        }
    }

    /// Create a renderer that owns the supplied workspace.
    pub fn with_workspace(workspace: RenderWorkspace) -> Self {
        Self { workspace }
    }

    /// Borrow the reusable workspace.
    pub const fn workspace(&self) -> &RenderWorkspace {
        &self.workspace
    }

    /// Mutably borrow the reusable workspace.
    pub fn workspace_mut(&mut self) -> &mut RenderWorkspace {
        &mut self.workspace
    }

    /// Consume the renderer and return its workspace.
    pub fn into_workspace(self) -> RenderWorkspace {
        self.workspace
    }

    /// Prepare and render a document into owned output.
    #[allow(clippy::result_large_err)]
    pub fn render<A>(
        &mut self,
        doc: &Doc<A>,
        options: RenderOptions,
    ) -> Result<Rendered<A>, OwnedRenderError> {
        let prepared = doc.prepare().map_err(OwnedRenderError::Prepare)?;
        self.render_prepared(&prepared, options)
    }

    /// Render an already prepared document into owned output.
    #[allow(clippy::result_large_err)]
    pub fn render_prepared<A>(
        &mut self,
        prepared: &crate::PreparedDoc<A>,
        options: RenderOptions,
    ) -> Result<Rendered<A>, OwnedRenderError> {
        render_prepared_into_owned(prepared, options, &mut self.workspace)
    }

    /// Render a prepared document into a [`fmt::Write`] sink.
    #[allow(clippy::result_large_err)]
    pub fn write_fmt<A>(
        &mut self,
        prepared: &crate::PreparedDoc<A>,
        options: RenderOptions,
        sink: &mut impl fmt::Write,
    ) -> Result<RenderSummary, WriteError<fmt::Error>> {
        let rendered =
            render_into(prepared, options, &mut self.workspace).map_err(WriteError::Render)?;
        sink.write_str(rendered.text()).map_err(WriteError::Sink)?;
        Ok(RenderSummary {
            cost: rendered.cost(),
            stats: rendered.stats(),
        })
    }

    /// Render a prepared document into an [`io::Write`] sink.
    #[allow(clippy::result_large_err)]
    pub fn write_io<A>(
        &mut self,
        prepared: &crate::PreparedDoc<A>,
        options: RenderOptions,
        sink: &mut impl io::Write,
    ) -> Result<RenderSummary, WriteError<io::Error>> {
        let rendered =
            render_into(prepared, options, &mut self.workspace).map_err(WriteError::Render)?;
        sink.write_all(rendered.text().as_bytes())
            .map_err(WriteError::Sink)?;
        Ok(RenderSummary {
            cost: rendered.cost(),
            stats: rendered.stats(),
        })
    }
}

/// Prepare and render a document using a temporary growable renderer.
#[allow(clippy::result_large_err)]
pub fn render<A>(doc: &Doc<A>, options: RenderOptions) -> Result<Rendered<A>, OwnedRenderError> {
    Renderer::new().render(doc, options)
}

/// Render a prepared document using a temporary growable renderer.
#[allow(clippy::result_large_err)]
pub fn render_prepared<A>(
    prepared: &crate::PreparedDoc<A>,
    options: RenderOptions,
) -> Result<Rendered<A>, OwnedRenderError> {
    Renderer::new().render_prepared(prepared, options)
}

/// Render a prepared document into owned output using caller-owned workspace.
///
/// Use [`Renderer`] to retain the workspace with its rendering methods.
#[allow(clippy::result_large_err)]
pub fn render_prepared_into_owned<A>(
    prepared: &crate::PreparedDoc<A>,
    options: RenderOptions,
    workspace: &mut RenderWorkspace,
) -> Result<Rendered<A>, OwnedRenderError> {
    workspace
        .reserve(seed_capacity(prepared.bounds(), options.strategy()))
        .map_err(OwnedRenderError::Reserve)?;
    let borrowed = render_into(prepared, options, workspace).map_err(OwnedRenderError::Render)?;

    let mut text = String::new();
    text.try_reserve_exact(borrowed.text().len())
        .map_err(|source| OwnedRenderError::OwnedOutputReserve {
            resource: OwnedOutputResource::Text,
            requested: borrowed.text().len(),
            source,
        })?;
    text.push_str(borrowed.text());

    let mut spans = Vec::new();
    spans
        .try_reserve_exact(borrowed.spans().len())
        .map_err(|source| OwnedRenderError::OwnedOutputReserve {
            resource: OwnedOutputResource::Spans,
            requested: borrowed.spans().len(),
            source,
        })?;
    spans.extend_from_slice(borrowed.spans());
    let cost = borrowed.cost();
    let stats = borrowed.stats();
    drop(borrowed);

    let mut annotations = Vec::new();
    annotations
        .try_reserve_exact(prepared.0.annotations.len())
        .map_err(|source| OwnedRenderError::OwnedOutputReserve {
            resource: OwnedOutputResource::Annotations,
            requested: prepared.0.annotations.len(),
            source,
        })?;
    annotations.extend(prepared.0.annotations.iter().cloned());
    Ok(Rendered {
        text,
        spans,
        annotations,
        cost,
        stats,
    })
}
