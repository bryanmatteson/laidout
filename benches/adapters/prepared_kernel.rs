//! Prepared Laidout 0.2 consumer-kernel benchmark adapter.

use std::num::NonZeroU32;

use crate::harness_v1::{BenchmarkAdapter, LogicalBytes, RunResult, Workload};
use laidout::{render_into, Doc, LayoutStrategy, RenderOptions, RenderWorkspace, WorkspaceMode};

pub struct PreparedAdapter;

pub struct PreparedCase {
    prepared: laidout::PreparedDoc,
    workspace: RenderWorkspace,
    options: RenderOptions,
}

impl BenchmarkAdapter for PreparedAdapter {
    type Prepared = PreparedCase;

    fn implementation(&self) -> &'static str {
        "prepared-kernel"
    }

    fn prepare(&self, workload: Workload) -> Result<Self::Prepared, String> {
        let source = match workload.name {
            "json-20" | "json-40" => json_doc(),
            "unique-sequence-2000" | "unique-sequence-4000" => unique_sequence(workload.size),
            "frontier-64" | "frontier-128" => wide_frontier(workload.size),
            "zero-width-2000" | "zero-width-4000" => zero_width(workload.size),
            other => return Err(format!("unknown workload {other}")),
        };
        let prepared = source.prepare().map_err(|error| error.to_string())?;
        drop(source);
        let strategy = if workload.name.starts_with("zero-width-") {
            LayoutStrategy::Fast
        } else {
            LayoutStrategy::Exact
        };
        let options = RenderOptions::new(
            NonZeroU32::new(workload.width).ok_or_else(|| "benchmark width is zero".to_owned())?,
        )
        .with_strategy(strategy);
        let mut workspace = RenderWorkspace::growable();
        drop(render_into(&prepared, options, &mut workspace).map_err(|error| error.to_string())?);
        workspace.set_mode(WorkspaceMode::Fixed);
        Ok(PreparedCase {
            prepared,
            workspace,
            options,
        })
    }

    fn run(&self, case: &mut Self::Prepared) -> Result<RunResult, String> {
        let rendered = render_into(&case.prepared, case.options, &mut case.workspace)
            .map_err(|error| error.to_string())?;
        Ok(RunResult {
            fingerprint: fingerprint(rendered.text().as_bytes()),
        })
    }

    fn logical_bytes(&self, case: &Self::Prepared) -> LogicalBytes {
        let prepared = laidout::measure::prepared_footprint(&case.prepared).total;
        let workspace = laidout::measure::workspace_footprint(&case.workspace).logical_total;
        LogicalBytes {
            prepared: u64::try_from(prepared).unwrap_or(u64::MAX),
            workspace: u64::try_from(workspace).unwrap_or(u64::MAX),
        }
    }
}

fn json_doc() -> Doc {
    let comma = Doc::concat([Doc::text(","), Doc::line()]);
    Doc::group(Doc::concat([
        Doc::text("{"),
        Doc::nest(
            2,
            Doc::concat([
                Doc::soft_line(),
                Doc::join(
                    comma,
                    [
                        Doc::concat([
                            Doc::text("\\\"name\\\":"),
                            Doc::line(),
                            Doc::text("\\\"laidout\\\""),
                        ]),
                        Doc::concat([
                            Doc::text("\\\"unicode\\\":"),
                            Doc::line(),
                            Doc::text("\\\"界\\\""),
                        ]),
                        Doc::concat([
                            Doc::text("\\\"values\\\":"),
                            Doc::line(),
                            Doc::group(Doc::concat([
                                Doc::text("[1,"),
                                Doc::line(),
                                Doc::text("2,"),
                                Doc::line(),
                                Doc::text("3]"),
                            ])),
                        ]),
                    ],
                ),
            ]),
        ),
        Doc::soft_line(),
        Doc::text("}"),
    ]))
}

fn unique_sequence(count: usize) -> Doc {
    Doc::concat((0..count).map(|index| Doc::text(index.to_string())))
}

fn wide_frontier(count: usize) -> Doc {
    let alternative = |index: usize| {
        Doc::penalize(
            u32::try_from(index).expect("benchmark index fits u32"),
            Doc::text("x".repeat(count - index)),
        )
    };
    (1..count).fold(alternative(0), |doc, index| {
        Doc::choice(doc, alternative(index))
    })
}

fn zero_width(count: usize) -> Doc {
    Doc::concat((0..count).map(|_| {
        Doc::group(Doc::concat([
            Doc::text("\u{200b}"),
            Doc::line(),
            Doc::text("x"),
            Doc::line(),
        ]))
    }))
}

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
