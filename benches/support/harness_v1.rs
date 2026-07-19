//! Immutable benchmark measurement and artifact-schema kernel.
//!
//! This file is captured before the prepared kernel changes production code.
//! Role adapters may change independently; clocks, counters, workload dispatch,
//! schema writing, and comparison rules live only here.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::fs;
use std::hint::black_box;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use serde_json::Value;

pub struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

fn record_growth(bytes: usize) {
    ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            record_growth(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            record_growth(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, old: Layout, new_size: usize) -> *mut u8 {
        let replacement = unsafe { System.realloc(pointer, old, new_size) };
        if !replacement.is_null() {
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            if new_size >= old.size() {
                record_growth(new_size - old.size());
            } else {
                LIVE_BYTES.fetch_sub(old.size() - new_size, Ordering::Relaxed);
            }
        }
        replacement
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy, Debug)]
pub struct Workload {
    pub name: &'static str,
    pub size: usize,
    pub width: u32,
}

pub const WORKLOADS: &[Workload] = &[
    Workload {
        name: "json-20",
        size: 1,
        width: 20,
    },
    Workload {
        name: "json-40",
        size: 1,
        width: 40,
    },
    Workload {
        name: "unique-sequence-2000",
        size: 2_000,
        width: u32::MAX,
    },
    Workload {
        name: "unique-sequence-4000",
        size: 4_000,
        width: u32::MAX,
    },
    Workload {
        name: "frontier-64",
        size: 64,
        width: 128,
    },
    Workload {
        name: "frontier-128",
        size: 128,
        width: 256,
    },
    Workload {
        name: "zero-width-2000",
        size: 2_000,
        width: 2,
    },
    Workload {
        name: "zero-width-4000",
        size: 4_000,
        width: 2,
    },
];

#[derive(Clone, Copy, Debug, Default)]
pub struct LogicalBytes {
    pub prepared: u64,
    pub workspace: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct RunResult {
    pub fingerprint: u64,
}

pub trait BenchmarkAdapter {
    type Prepared;

    fn implementation(&self) -> &'static str;
    fn prepare(&self, workload: Workload) -> Result<Self::Prepared, String>;
    fn run(&self, prepared: &mut Self::Prepared) -> Result<RunResult, String>;
    fn logical_bytes(&self, _prepared: &Self::Prepared) -> LogicalBytes {
        LogicalBytes::default()
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct AllocationSample {
    allocations: u64,
    reallocations: u64,
    deallocations: u64,
    allocated_bytes: u64,
    retained_before: u64,
    peak_delta: u64,
    peak_total: u64,
}

#[derive(Debug)]
struct CaseArtifact {
    workload: Workload,
    samples_ns: Vec<u64>,
    median_ns: u64,
    p95_ns: u64,
    allocations: u64,
    reallocations: u64,
    deallocations: u64,
    allocated_bytes: u64,
    retained_before: u64,
    peak_delta: u64,
    peak_total: u64,
    prepared_logical_bytes: u64,
    workspace_logical_bytes: u64,
    fingerprint: u64,
    coefficient_of_variation: f64,
}

#[derive(Debug)]
struct Identity {
    production_source: String,
    plan: String,
    harness: String,
    role_adapter: String,
    baseline_adapter: String,
    workload: String,
    dependency_graph_digest: String,
    dependency_graph: String,
    rustc: String,
    target: String,
    operating_system: String,
    cpu: String,
}

pub fn entry<A: BenchmarkAdapter>(adapter: &A) -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match arguments.first().map(String::as_str) {
        Some("capture") => capture(adapter, &arguments[1..]),
        Some("compare") => compare(&arguments[1..]),
        _ => Err("expected `capture` or `compare`".to_owned()),
    }
}

fn capture<A: BenchmarkAdapter>(adapter: &A, arguments: &[String]) -> Result<(), String> {
    let options = options(arguments)?;
    let role = required(&options, "role")?;
    let implementation = required(&options, "implementation")?;
    let output = PathBuf::from(required(&options, "output")?);
    if implementation != adapter.implementation() {
        return Err(format!(
            "implementation argument {implementation:?} does not match registered adapter {:?}",
            adapter.implementation()
        ));
    }
    if role != "baseline" && role != "final" {
        return Err(format!("unsupported role {role:?}"));
    }
    if role == "baseline" {
        verify_laidout_baseline_identity()?;
    }

    let identity = identity(role)?;
    let mut cases = Vec::with_capacity(WORKLOADS.len());
    for workload in WORKLOADS {
        cases.push(measure_case(adapter, *workload)?);
    }
    let artifact = artifact_json(role, implementation, &identity, &cases);
    write_new_or_identical(&output, artifact.as_bytes())?;
    println!("wrote {}", output.display());
    Ok(())
}

fn measure_case<A: BenchmarkAdapter>(
    adapter: &A,
    workload: Workload,
) -> Result<CaseArtifact, String> {
    let mut prepared = adapter.prepare(workload)?;
    let warm = adapter.run(&mut prepared)?;
    black_box(warm.fingerprint);

    let logical = adapter.logical_bytes(&prepared);
    let mut best_group: Option<(Vec<u64>, Vec<AllocationSample>, u64, f64)> = None;
    for _ in 0..3 {
        let mut samples = Vec::with_capacity(30);
        let mut allocations = Vec::with_capacity(30);
        let mut fingerprint = None;
        for _ in 0..30 {
            let retained_before = LIVE_BYTES.load(Ordering::Relaxed);
            ALLOCATIONS.store(0, Ordering::Relaxed);
            REALLOCATIONS.store(0, Ordering::Relaxed);
            DEALLOCATIONS.store(0, Ordering::Relaxed);
            ALLOCATED_BYTES.store(0, Ordering::Relaxed);
            PEAK_BYTES.store(retained_before, Ordering::Relaxed);

            let started = Instant::now();
            let result = adapter.run(&mut prepared)?;
            let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            black_box(result.fingerprint);
            if let Some(previous) = fingerprint {
                if previous != result.fingerprint {
                    return Err(format!("{} produced unstable output", workload.name));
                }
            } else {
                fingerprint = Some(result.fingerprint);
            }
            samples.push(elapsed);
            let peak_total = PEAK_BYTES.load(Ordering::Relaxed);
            allocations.push(AllocationSample {
                allocations: ALLOCATIONS.load(Ordering::Relaxed),
                reallocations: REALLOCATIONS.load(Ordering::Relaxed),
                deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
                allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
                retained_before: retained_before as u64,
                peak_delta: peak_total.saturating_sub(retained_before) as u64,
                peak_total: peak_total as u64,
            });
        }
        let cv = coefficient_of_variation(&samples);
        let candidate = (samples, allocations, fingerprint.unwrap_or_default(), cv);
        if best_group.as_ref().is_none_or(|current| cv < current.3) {
            best_group = Some(candidate);
        }
        if cv <= 0.05 {
            break;
        }
    }

    let (mut samples, allocations, fingerprint, cv) = best_group.expect("three sample groups");
    samples.sort_unstable();
    let median_ns = samples[samples.len() / 2];
    let p95_ns = samples[(samples.len() * 95).div_ceil(100).saturating_sub(1)];
    let allocation_field = |field: fn(&AllocationSample) -> u64| {
        let mut values = allocations.iter().map(field).collect::<Vec<_>>();
        values.sort_unstable();
        values[values.len() / 2]
    };
    Ok(CaseArtifact {
        workload,
        samples_ns: samples,
        median_ns,
        p95_ns,
        allocations: allocation_field(|sample| sample.allocations),
        reallocations: allocation_field(|sample| sample.reallocations),
        deallocations: allocation_field(|sample| sample.deallocations),
        allocated_bytes: allocation_field(|sample| sample.allocated_bytes),
        retained_before: allocation_field(|sample| sample.retained_before),
        peak_delta: allocation_field(|sample| sample.peak_delta),
        peak_total: allocation_field(|sample| sample.peak_total),
        prepared_logical_bytes: logical.prepared,
        workspace_logical_bytes: logical.workspace,
        fingerprint,
        coefficient_of_variation: cv,
    })
}

fn coefficient_of_variation(samples: &[u64]) -> f64 {
    let count = samples.len() as f64;
    let mean = samples.iter().map(|value| *value as f64).sum::<f64>() / count;
    if mean == 0.0 {
        return 0.0;
    }
    let variance = samples
        .iter()
        .map(|value| {
            let delta = *value as f64 - mean;
            delta * delta
        })
        .sum::<f64>()
        / count;
    variance.sqrt() / mean
}

fn verify_laidout_baseline_identity() -> Result<(), String> {
    let src = command("git", &["rev-parse", "HEAD:src"])?;
    let manifest = command("git", &["rev-parse", "HEAD:Cargo.toml"])?;
    if src.trim() != "8a6ba8b5d7ad618e409118d37d8731d0ca319302" {
        return Err(format!("unexpected pinned Laidout src tree: {src}"));
    }
    if manifest.trim() != "e679d0b11b963edc27ef243fa4ee8c10a2f4b442" {
        return Err(format!(
            "unexpected pinned Laidout manifest blob: {manifest}"
        ));
    }
    let status = Command::new("git")
        .args(["diff", "--quiet", "--", "src"])
        .status()
        .map_err(|error| format!("failed to inspect source tree: {error}"))?;
    if !status.success() {
        return Err("production source changed before baseline capture".to_owned());
    }
    Ok(())
}

fn identity(role: &str) -> Result<Identity, String> {
    let adapter = if role == "baseline" {
        "benches/adapters/laidout_0_1_baseline.rs"
    } else {
        "benches/adapters/prepared_kernel.rs"
    };
    let dependency_graph = normalized_dependency_graph("laidout", &["--features", "research"])?;
    Ok(Identity {
        production_source: production_source_digest(role)?,
        plan: sha256_path(Path::new("docs/prepared-layout-kernel-plan.md"))?,
        harness: sha256_path(Path::new("benches/support/harness_v1.rs"))?,
        role_adapter: sha256_path(Path::new(adapter))?,
        baseline_adapter: sha256_path(Path::new("benches/adapters/laidout_0_1_baseline.rs"))?,
        workload: sha256_path(Path::new("benches/workloads/prepared-kernel-v1.toml"))?,
        dependency_graph_digest: sha256_bytes(dependency_graph.as_bytes())?,
        dependency_graph,
        rustc: command("rustc", &["-vV"])?,
        target: command("rustc", &["-vV"])?
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .unwrap_or("unknown")
            .to_owned(),
        operating_system: command("uname", &["-a"])?,
        cpu: command(
            "sh",
            &[
                "-c",
                "sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m",
            ],
        )?,
    })
}

fn artifact_json(
    role: &str,
    implementation: &str,
    identity: &Identity,
    cases: &[CaseArtifact],
) -> String {
    let mut output = String::new();
    output.push_str("{\n");
    field_number(&mut output, "schema_version", 1, true);
    field_string(&mut output, "role", role, true);
    field_string(&mut output, "repository", "laidout", true);
    field_string(&mut output, "implementation", implementation, true);
    field_string(
        &mut output,
        "pinned_baseline",
        "38c55341e02fd9e4869e060d36e88886c7ed1ea3",
        true,
    );
    field_string(
        &mut output,
        "production_src_digest",
        &identity.production_source,
        true,
    );
    field_string(&mut output, "plan_digest", &identity.plan, true);
    field_string(
        &mut output,
        "immutable_harness_digest",
        &identity.harness,
        true,
    );
    field_string(
        &mut output,
        "role_adapter_digest",
        &identity.role_adapter,
        true,
    );
    field_string(
        &mut output,
        "baseline_adapter_digest",
        &identity.baseline_adapter,
        true,
    );
    field_string(
        &mut output,
        "workload_manifest_digest",
        &identity.workload,
        true,
    );
    field_string(
        &mut output,
        "dependency_graph_digest",
        &identity.dependency_graph_digest,
        true,
    );
    field_string(
        &mut output,
        "dependency_graph",
        &identity.dependency_graph,
        true,
    );
    field_string(&mut output, "rustc", &identity.rustc, true);
    field_string(&mut output, "target", &identity.target, true);
    field_string(
        &mut output,
        "operating_system",
        &identity.operating_system,
        true,
    );
    field_string(&mut output, "cpu", &identity.cpu, true);
    field_string(&mut output, "features", "research", true);
    field_string(&mut output, "allocator_mode", "counting-system", true);
    output.push_str("  \"cases\": [\n");
    for (index, case) in cases.iter().enumerate() {
        output.push_str("    {");
        inline_string(&mut output, "name", case.workload.name, true);
        inline_number(&mut output, "size", case.workload.size as u64, true);
        inline_number(&mut output, "width", u64::from(case.workload.width), true);
        inline_number(&mut output, "median_ns", case.median_ns, true);
        inline_number(&mut output, "p95_ns", case.p95_ns, true);
        inline_number(&mut output, "allocations", case.allocations, true);
        inline_number(&mut output, "reallocations", case.reallocations, true);
        inline_number(&mut output, "deallocations", case.deallocations, true);
        inline_number(&mut output, "allocated_bytes", case.allocated_bytes, true);
        inline_number(&mut output, "retained_before", case.retained_before, true);
        inline_number(&mut output, "peak_live_delta", case.peak_delta, true);
        inline_number(&mut output, "peak_total_live", case.peak_total, true);
        inline_number(
            &mut output,
            "prepared_logical_bytes",
            case.prepared_logical_bytes,
            true,
        );
        inline_number(
            &mut output,
            "workspace_logical_bytes",
            case.workspace_logical_bytes,
            true,
        );
        inline_number(&mut output, "fingerprint", case.fingerprint, true);
        output.push_str(&format!(
            "\"coefficient_of_variation\":{:.8},\"samples_ns\":[",
            case.coefficient_of_variation
        ));
        for (sample_index, sample) in case.samples_ns.iter().enumerate() {
            if sample_index > 0 {
                output.push(',');
            }
            output.push_str(&sample.to_string());
        }
        output.push_str("]}");
        if index + 1 != cases.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("  ]\n}\n");
    output
}

fn compare(arguments: &[String]) -> Result<(), String> {
    let options = options(arguments)?;
    let baseline_path = PathBuf::from(required(&options, "baseline")?);
    let candidate_path = PathBuf::from(required(&options, "candidate")?);
    let transition = PathBuf::from(required(&options, "dependency-transition")?);
    if !transition.is_file() {
        return Err(format!(
            "dependency transition manifest is missing: {}",
            transition.display()
        ));
    }
    let baseline = fs::read_to_string(&baseline_path)
        .map_err(|error| format!("failed to read {}: {error}", baseline_path.display()))?;
    let candidate = fs::read_to_string(&candidate_path)
        .map_err(|error| format!("failed to read {}: {error}", candidate_path.display()))?;
    validate_current_evidence(&baseline, &candidate)?;
    validate_dependency_transition(&baseline, &candidate, &transition)?;
    for key in [
        "schema_version",
        "repository",
        "pinned_baseline",
        "plan_digest",
        "immutable_harness_digest",
        "baseline_adapter_digest",
        "workload_manifest_digest",
        "rustc",
        "target",
        "operating_system",
        "cpu",
        "features",
        "allocator_mode",
    ] {
        if raw_field(&baseline, key) != raw_field(&candidate, key) {
            return Err(format!("benchmark identity mismatch for {key}"));
        }
    }
    if string_field(&baseline, "role").as_deref() != Some("baseline")
        || string_field(&candidate, "role").as_deref() != Some("final")
    {
        return Err("benchmark artifacts have incorrect roles".to_owned());
    }
    let current_baseline = sha256_path(Path::new("benches/adapters/laidout_0_1_baseline.rs"))?;
    if string_field(&baseline, "baseline_adapter_digest").as_deref()
        != Some(current_baseline.as_str())
    {
        return Err("baseline adapter changed after capture".to_owned());
    }
    let baseline_cases = case_metrics(&baseline)?;
    let candidate_cases = case_metrics(&candidate)?;
    if baseline_cases.keys().collect::<Vec<_>>() != candidate_cases.keys().collect::<Vec<_>>() {
        return Err("benchmark case-set mismatch".to_owned());
    }
    for name in ["json-20", "json-40"] {
        let old = baseline_cases
            .get(name)
            .ok_or_else(|| format!("missing {name}"))?;
        let new = candidate_cases
            .get(name)
            .ok_or_else(|| format!("missing {name}"))?;
        require_ratio(name, "median_ns", new.0, old.0, 1.15)?;
        require_ratio(name, "peak_total_live", new.1, old.1, 1.10)?;
    }
    require_pair_ratio(
        &candidate_cases,
        "unique-sequence-2000",
        "unique-sequence-4000",
        2.5,
        2.25,
    )?;
    require_pair_ratio(&candidate_cases, "frontier-64", "frontier-128", 3.0, 2.25)?;
    require_pair_ratio(
        &candidate_cases,
        "zero-width-2000",
        "zero-width-4000",
        2.5,
        2.25,
    )?;
    println!("prepared-kernel benchmark comparison passed");
    Ok(())
}

fn validate_current_evidence(
    baseline_artifact: &str,
    candidate_artifact: &str,
) -> Result<(), String> {
    let baseline: Value = serde_json::from_str(baseline_artifact)
        .map_err(|error| format!("invalid baseline artifact JSON: {error}"))?;
    let candidate: Value = serde_json::from_str(candidate_artifact)
        .map_err(|error| format!("invalid candidate artifact JSON: {error}"))?;
    for (key, current) in [
        (
            "plan_digest",
            sha256_path(Path::new("docs/prepared-layout-kernel-plan.md"))?,
        ),
        (
            "immutable_harness_digest",
            sha256_path(Path::new("benches/support/harness_v1.rs"))?,
        ),
        (
            "baseline_adapter_digest",
            sha256_path(Path::new("benches/adapters/laidout_0_1_baseline.rs"))?,
        ),
        (
            "workload_manifest_digest",
            sha256_path(Path::new("benches/workloads/prepared-kernel-v1.toml"))?,
        ),
    ] {
        if baseline[key].as_str() != Some(current.as_str())
            || candidate[key].as_str() != Some(current.as_str())
        {
            return Err(format!("stale benchmark evidence for {key}"));
        }
    }
    let final_adapter = sha256_path(Path::new("benches/adapters/prepared_kernel.rs"))?;
    if candidate["role_adapter_digest"].as_str() != Some(final_adapter.as_str()) {
        return Err("stale final role adapter evidence".to_owned());
    }
    let baseline_source = production_source_digest("baseline")?;
    let candidate_source = production_source_digest("final")?;
    if baseline["production_src_digest"].as_str() != Some(baseline_source.as_str())
        || candidate["production_src_digest"].as_str() != Some(candidate_source.as_str())
    {
        return Err("stale production source evidence".to_owned());
    }
    let graph = normalized_dependency_graph("laidout", &["--features", "research"])?;
    if candidate["dependency_graph"].as_str() != Some(graph.as_str()) {
        return Err("stale candidate dependency graph evidence".to_owned());
    }
    Ok(())
}

#[derive(Default)]
struct GraphData {
    root: String,
    requirements: BTreeMap<(String, String, String), String>,
    packages: std::collections::BTreeSet<String>,
    edges: BTreeMap<String, std::collections::BTreeSet<String>>,
}

fn validate_dependency_transition(
    baseline_artifact: &str,
    candidate_artifact: &str,
    transition_path: &Path,
) -> Result<(), String> {
    let baseline_json: Value = serde_json::from_str(baseline_artifact)
        .map_err(|error| format!("invalid baseline artifact JSON: {error}"))?;
    let candidate_json: Value = serde_json::from_str(candidate_artifact)
        .map_err(|error| format!("invalid candidate artifact JSON: {error}"))?;
    let baseline_text = baseline_json["dependency_graph"]
        .as_str()
        .ok_or("baseline artifact omitted dependency_graph")?;
    let candidate_text = candidate_json["dependency_graph"]
        .as_str()
        .ok_or("candidate artifact omitted dependency_graph")?;
    for (role, artifact, graph) in [
        ("baseline", &baseline_json, baseline_text),
        ("candidate", &candidate_json, candidate_text),
    ] {
        let expected = artifact["dependency_graph_digest"]
            .as_str()
            .ok_or_else(|| format!("{role} artifact omitted dependency_graph_digest"))?;
        let actual = sha256_bytes(graph.as_bytes())?;
        if expected != actual {
            return Err(format!("{role} dependency graph digest is invalid"));
        }
    }

    let baseline = parse_dependency_graph(baseline_text)?;
    let candidate = parse_dependency_graph(candidate_text)?;
    let transition_text = fs::read_to_string(transition_path)
        .map_err(|error| format!("failed to read {}: {error}", transition_path.display()))?;
    let transition: toml::Value = toml::from_str(&transition_text)
        .map_err(|error| format!("invalid dependency transition manifest: {error}"))?;
    if transition
        .get("schema_version")
        .and_then(toml::Value::as_integer)
        != Some(1)
    {
        return Err("unsupported dependency transition schema".to_owned());
    }
    let baseline_version = package_version(&baseline.root)?;
    let candidate_version = package_version(&candidate.root)?;
    if transition
        .get("baseline_version")
        .and_then(toml::Value::as_str)
        != Some(baseline_version)
        || transition
            .get("candidate_version")
            .and_then(toml::Value::as_str)
            != Some(candidate_version)
    {
        return Err("dependency transition root versions do not match artifacts".to_owned());
    }

    let mut declared = std::collections::BTreeSet::new();
    let mut permits_feature_boundary = false;
    for section in ["direct_change", "dev_change"] {
        for change in transition
            .get(section)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let package = change
                .get("package")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("{section} entry omitted package"))?;
            declared.insert(package.to_owned());
            permits_feature_boundary |= change.get("kind").and_then(toml::Value::as_str)
                == Some("feature-boundary")
                && package == package_name(&candidate.root)?;
            if let Some(version) = change.get("version").and_then(toml::Value::as_str) {
                let resolved = candidate
                    .packages
                    .iter()
                    .filter(|key| package_name(key).ok() == Some(package))
                    .map(|key| package_version(key))
                    .collect::<Result<Vec<_>, _>>()?;
                if !resolved
                    .iter()
                    .any(|resolved| transition_version_matches(version, resolved))
                {
                    return Err(format!(
                        "declared package {package} version {version} is not resolved"
                    ));
                }
            }
        }
    }

    let requirement_keys = baseline
        .requirements
        .keys()
        .chain(candidate.requirements.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    for key in requirement_keys {
        if baseline.requirements.get(&key) != candidate.requirements.get(&key)
            && !declared.contains(&key.0)
            && !permits_feature_boundary
        {
            return Err(format!("undeclared direct dependency change for {}", key.0));
        }
    }

    let baseline_allowed = declared_closure(&baseline, &declared);
    let candidate_allowed = declared_closure(&candidate, &declared);
    for package in candidate.packages.difference(&baseline.packages) {
        if package != &candidate.root && !candidate_allowed.contains(package) {
            return Err(format!(
                "unclassified candidate dependency {}",
                package_name(package)?
            ));
        }
    }
    for package in baseline.packages.difference(&candidate.packages) {
        if package != &baseline.root && !baseline_allowed.contains(package) {
            return Err(format!(
                "unclassified removed dependency {}",
                package_name(package)?
            ));
        }
    }
    for package in baseline.packages.intersection(&candidate.packages) {
        if package == &baseline.root
            || baseline_allowed.contains(package)
            || candidate_allowed.contains(package)
        {
            continue;
        }
        if baseline.edges.get(package) != candidate.edges.get(package) {
            return Err(format!(
                "unclassified dependency-edge change for {}",
                package_name(package)?
            ));
        }
    }
    Ok(())
}

fn parse_dependency_graph(input: &str) -> Result<GraphData, String> {
    let mut graph = GraphData::default();
    for line in input.lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        match fields.as_slice() {
            ["ROOT", root] => graph.root = (*root).to_owned(),
            ["REQ", name, requirement, kind, optional, defaults, features, target] => {
                graph.requirements.insert(
                    ((*name).to_owned(), (*kind).to_owned(), (*target).to_owned()),
                    [*requirement, *optional, *defaults, *features].join("\t"),
                );
            }
            ["PKG", package] => {
                graph.packages.insert((*package).to_owned());
            }
            ["EDGE", from, _kind, _target, to] => {
                graph
                    .edges
                    .entry((*from).to_owned())
                    .or_default()
                    .insert((*to).to_owned());
            }
            _ => return Err(format!("malformed dependency graph line {line:?}")),
        }
    }
    if graph.root.is_empty() || !graph.packages.contains(&graph.root) {
        return Err("dependency graph has no resolved root".to_owned());
    }
    Ok(graph)
}

fn declared_closure(
    graph: &GraphData,
    declared: &std::collections::BTreeSet<String>,
) -> std::collections::BTreeSet<String> {
    let mut closure = std::collections::BTreeSet::new();
    let mut pending = graph
        .edges
        .get(&graph.root)
        .into_iter()
        .flatten()
        .filter(|package| package_name(package).is_ok_and(|name| declared.contains(name)))
        .cloned()
        .collect::<Vec<_>>();
    while let Some(package) = pending.pop() {
        if !closure.insert(package.clone()) {
            continue;
        }
        pending.extend(graph.edges.get(&package).into_iter().flatten().cloned());
    }
    closure
}

fn package_name(key: &str) -> Result<&str, String> {
    key.split_once('@')
        .map(|(name, _)| name)
        .ok_or_else(|| format!("malformed package key {key:?}"))
}

fn package_version(key: &str) -> Result<&str, String> {
    key.split_once('@')
        .and_then(|(_, rest)| rest.split_once('['))
        .map(|(version, _)| version)
        .ok_or_else(|| format!("malformed package key {key:?}"))
}

fn transition_version_matches(expected: &str, resolved: &str) -> bool {
    resolved == expected
        || (!expected.contains('.') && resolved.split('.').next() == Some(expected))
}

fn require_pair_ratio(
    cases: &BTreeMap<String, (u64, u64, u64)>,
    small: &str,
    large: &str,
    time_limit: f64,
    memory_limit: f64,
) -> Result<(), String> {
    let small = cases.get(small).ok_or_else(|| format!("missing {small}"))?;
    let large = cases.get(large).ok_or_else(|| format!("missing {large}"))?;
    require_ratio(
        large.2.to_string().as_str(),
        "median_ns",
        large.0,
        small.0,
        time_limit,
    )?;
    require_ratio(
        large.2.to_string().as_str(),
        "logical_bytes",
        large.1,
        small.1,
        memory_limit,
    )
}

fn require_ratio(
    name: &str,
    metric: &str,
    value: u64,
    base: u64,
    limit: f64,
) -> Result<(), String> {
    if base == 0 {
        return Err(format!("{name} has zero baseline {metric}"));
    }
    let ratio = value as f64 / base as f64;
    if ratio > limit {
        return Err(format!(
            "{name} {metric} ratio {ratio:.3} exceeds {limit:.3}"
        ));
    }
    Ok(())
}

fn case_metrics(input: &str) -> Result<BTreeMap<String, (u64, u64, u64)>, String> {
    let mut cases = BTreeMap::new();
    let mut rest = input;
    while let Some(position) = rest.find("\"name\":\"") {
        rest = &rest[position + 8..];
        let end = rest.find('"').ok_or("malformed case name")?;
        let name = rest[..end].to_owned();
        let object_end = rest.find('}').ok_or("malformed case object")?;
        let object = &rest[..object_end];
        let median = number_field(object, "median_ns").ok_or("missing median_ns")?;
        let peak = number_field(object, "peak_total_live").ok_or("missing peak_total_live")?;
        let prepared = number_field(object, "prepared_logical_bytes").unwrap_or(0);
        let workspace = number_field(object, "workspace_logical_bytes").unwrap_or(0);
        cases.insert(
            name,
            (median, prepared.saturating_add(workspace).max(peak), peak),
        );
        rest = &rest[object_end + 1..];
    }
    Ok(cases)
}

fn options(arguments: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut parsed = BTreeMap::new();
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--bench" {
            index += 1;
            continue;
        }
        let key = arguments[index]
            .strip_prefix("--")
            .ok_or_else(|| format!("expected option, found {:?}", arguments[index]))?;
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| format!("missing value for --{key}"))?;
        parsed.insert(key.to_owned(), value.clone());
        index += 2;
    }
    Ok(parsed)
}

fn required<'a>(options: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    options
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("missing --{key}"))
}

fn command(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("failed to run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("{program} emitted non-UTF-8 output: {error}"))
}

fn normalized_dependency_graph(
    root_name: &str,
    feature_arguments: &[&str],
) -> Result<String, String> {
    let mut arguments = vec!["metadata", "--format-version", "1"];
    arguments.extend_from_slice(feature_arguments);
    let metadata: Value = serde_json::from_str(&command("cargo", &arguments)?)
        .map_err(|error| format!("failed to parse cargo metadata: {error}"))?;
    let workspace_members = metadata["workspace_members"]
        .as_array()
        .ok_or("cargo metadata omitted workspace_members")?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    let mut packages = BTreeMap::<String, (String, Value)>::new();
    for package in metadata["packages"]
        .as_array()
        .ok_or("cargo metadata omitted packages")?
    {
        let id = package["id"].as_str().ok_or("package without id")?;
        let name = package["name"].as_str().ok_or("package without name")?;
        let version = package["version"]
            .as_str()
            .ok_or("package without version")?;
        let source = package["source"].as_str().unwrap_or_else(|| {
            if workspace_members.contains(id) {
                "workspace"
            } else {
                "path"
            }
        });
        let key = format!("{name}@{version}[{source}]");
        packages.insert(id.to_owned(), (key, package.clone()));
    }
    let root = packages
        .iter()
        .find_map(|(id, (_, package))| {
            (package["name"].as_str() == Some(root_name) && workspace_members.contains(id))
                .then(|| id.clone())
        })
        .ok_or_else(|| format!("cargo metadata omitted workspace root {root_name}"))?;
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("cargo metadata omitted resolve nodes")?;
    let node_by_id = nodes
        .iter()
        .map(|node| {
            Ok((
                node["id"]
                    .as_str()
                    .ok_or("resolve node without id")?
                    .to_owned(),
                node,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let mut reachable = std::collections::BTreeSet::new();
    let mut pending = vec![root.clone()];
    while let Some(id) = pending.pop() {
        if !reachable.insert(id.clone()) {
            continue;
        }
        let node = node_by_id
            .get(&id)
            .ok_or_else(|| format!("missing resolve node {id}"))?;
        for dependency in node["deps"].as_array().ok_or("resolve node without deps")? {
            pending.push(
                dependency["pkg"]
                    .as_str()
                    .ok_or("resolved dependency without package id")?
                    .to_owned(),
            );
        }
    }

    let mut lines = Vec::new();
    lines.push(format!("ROOT\t{}", packages[&root].0));
    let root_package = &packages[&root].1;
    for dependency in root_package["dependencies"]
        .as_array()
        .ok_or("root package without dependency declarations")?
    {
        let features = dependency["features"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(",");
        lines.push(format!(
            "REQ\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            dependency["name"]
                .as_str()
                .ok_or("dependency without name")?,
            dependency["req"]
                .as_str()
                .ok_or("dependency without requirement")?,
            dependency["kind"].as_str().unwrap_or("normal"),
            dependency["optional"].as_bool().unwrap_or(false),
            dependency["uses_default_features"]
                .as_bool()
                .unwrap_or(true),
            features,
            dependency["target"].as_str().unwrap_or(""),
        ));
    }
    for id in &reachable {
        let (key, _) = &packages[id];
        lines.push(format!("PKG\t{key}"));
        let node = node_by_id[id];
        for dependency in node["deps"].as_array().ok_or("resolve node without deps")? {
            let target = &packages[dependency["pkg"]
                .as_str()
                .ok_or("resolved dependency without package id")?]
            .0;
            for kind in dependency["dep_kinds"]
                .as_array()
                .ok_or("resolved dependency without dependency kinds")?
            {
                lines.push(format!(
                    "EDGE\t{key}\t{}\t{}\t{target}",
                    kind["kind"].as_str().unwrap_or("normal"),
                    kind["target"].as_str().unwrap_or(""),
                ));
            }
        }
    }
    lines.sort();
    Ok(lines.join("\n"))
}

fn production_source_digest(role: &str) -> Result<String, String> {
    if role == "baseline" {
        let source = command(
            "git",
            &["rev-parse", "38c55341e02fd9e4869e060d36e88886c7ed1ea3:src"],
        )?;
        let manifest = command(
            "git",
            &[
                "rev-parse",
                "38c55341e02fd9e4869e060d36e88886c7ed1ea3:Cargo.toml",
            ],
        )?;
        return sha256_bytes(format!("src\t{source}\nmanifest\t{manifest}\n").as_bytes());
    }
    let mut files = vec![PathBuf::from("Cargo.toml")];
    let mut pending = vec![PathBuf::from("src")];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            for entry in fs::read_dir(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?
            {
                pending.push(
                    entry
                        .map_err(|error| format!("failed to read {}: {error}", path.display()))?
                        .path(),
                );
            }
        } else if path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    let mut bytes = Vec::new();
    for path in files {
        let name = path.to_string_lossy();
        let contents = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        bytes.extend_from_slice(&(name.len() as u64).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&(contents.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&contents);
    }
    sha256_bytes(&bytes)
}

fn sha256_path(path: &Path) -> Result<String, String> {
    let path = path
        .to_str()
        .ok_or_else(|| format!("non-UTF-8 path: {}", path.display()))?;
    let output = command("shasum", &["-a", "256", path])?;
    output
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| format!("missing digest for {path}"))
}

fn sha256_bytes(bytes: &[u8]) -> Result<String, String> {
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to run shasum: {error}"))?;
    child
        .stdin
        .as_mut()
        .ok_or("missing shasum stdin")?
        .write_all(bytes)
        .map_err(|error| format!("failed to hash bytes: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for shasum: {error}"))?;
    if !output.status.success() {
        return Err("shasum failed".to_owned());
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("shasum emitted non-UTF-8 output: {error}"))?
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| "shasum emitted no digest".to_owned())
}

fn write_new_or_identical(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Ok(existing) = fs::read(path) {
        if existing == bytes {
            return Ok(());
        }
        return Err(format!(
            "refusing to overwrite differing artifact {}",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("failed to install {}: {error}", path.display()))
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn field_string(output: &mut String, key: &str, value: &str, comma: bool) {
    output.push_str(&format!(
        "  \"{}\": \"{}\"{}\n",
        key,
        escape_json(value),
        if comma { "," } else { "" }
    ));
}

fn field_number(output: &mut String, key: &str, value: u64, comma: bool) {
    output.push_str(&format!(
        "  \"{}\": {}{}\n",
        key,
        value,
        if comma { "," } else { "" }
    ));
}

fn inline_string(output: &mut String, key: &str, value: &str, comma: bool) {
    output.push_str(&format!(
        "\"{}\":\"{}\"{}",
        key,
        escape_json(value),
        if comma { "," } else { "" }
    ));
}

fn inline_number(output: &mut String, key: &str, value: u64, comma: bool) {
    output.push_str(&format!(
        "\"{}\":{}{}",
        key,
        value,
        if comma { "," } else { "" }
    ));
}

fn raw_field<'a>(input: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("\"{key}\":");
    let rest = input
        .find(&marker)
        .map(|position| &input[position + marker.len()..])?;
    let end = rest.find([',', '\n', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn string_field(input: &str, key: &str) -> Option<String> {
    raw_field(input, key).map(|value| value.trim_matches('"').to_owned())
}

fn number_field(input: &str, key: &str) -> Option<u64> {
    raw_field(input, key)?.trim().parse().ok()
}
