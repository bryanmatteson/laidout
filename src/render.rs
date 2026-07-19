//! Allocating convenience rendering built on the reusable prepared kernel.

use std::collections::TryReserveError;
use std::fmt;
use std::hash::Hash;
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
pub enum OwnedOutputResource {
    Text,
    Spans,
    Annotations,
}

#[derive(Debug, thiserror::Error)]
pub enum OwnedRenderError {
    #[error("document preparation failed")]
    Prepare(#[source] PrepareError),
    #[error("workspace reservation failed")]
    Reserve(#[source] ReserveError),
    #[error("document rendering failed")]
    Render(#[source] RenderError),
    #[error("failed to reserve {requested} entries for owned {resource:?} output")]
    OwnedOutputReserve {
        resource: OwnedOutputResource,
        requested: usize,
        #[source]
        source: TryReserveError,
    },
}

pub struct Rendered<A> {
    text: String,
    spans: Vec<AnnotationSpan>,
    annotations: Vec<Arc<A>>,
    cost: ConsumerCost,
    stats: SolveStats,
}

impl<A> Rendered<A> {
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn spans(&self) -> &[AnnotationSpan] {
        &self.spans
    }
    pub const fn cost(&self) -> ConsumerCost {
        self.cost
    }
    pub const fn stats(&self) -> SolveStats {
        self.stats
    }

    pub fn resolved_spans(&self) -> OwnedResolvedSpans<'_, A> {
        OwnedResolvedSpans {
            spans: self.spans.iter(),
            annotations: &self.annotations,
        }
    }

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

pub struct AnnotatedRun<'a, A> {
    pub text: &'a str,
    pub range: Range<usize>,
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

#[allow(clippy::result_large_err)]
pub fn render<A>(doc: &Doc<A>, options: RenderOptions) -> Result<Rendered<A>, OwnedRenderError>
where
    A: Eq + Hash,
{
    let prepared = doc.prepare().map_err(OwnedRenderError::Prepare)?;
    let mut workspace = RenderWorkspace::growable();
    workspace
        .reserve(seed_capacity(prepared.bounds(), options.strategy()))
        .map_err(OwnedRenderError::Reserve)?;
    let borrowed =
        render_into(&prepared, options, &mut workspace).map_err(OwnedRenderError::Render)?;

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
