//! Dispatches one admitted Cortex Gnat shard to a real Cortex process and
//! reports back its truthful outcome.
//!
//! [`GnatDispatchValidator::negotiate`](super::GnatDispatchValidator::negotiate)
//! only ever decides whether FA Local *admits* a Cortex-initiated Gnat run;
//! it has no onward path to actually running a shard. Cortex's own
//! `run_serial_gnat_plan`/`run_parallel_gnat_plan` run a shard through its
//! registered worker and produce a receipt, but only ever in-process, inside
//! Cortex's own Python runtime -- neither is a real process-boundary entry
//! point FA Local could reach. `DECISIONS/0019` (fa-local-owns-gnat-execution-routing,
//! in the COR repo) names exactly this gap: "Bounded parallel execution
//! requires a later FA-Local dispatch adapter." This module is that adapter.
//!
//! [`CortexSubprocessGnatShardAdapter`] spawns Cortex's
//! `cortex_runtime.gnats.shard_cli` module (one new bounded CLI entry point
//! added in COR alongside this) as a subprocess, one shard per call, and
//! parses back the `GnatWorkerReceipt.v1` it prints. Bounded to the two
//! worker types `DECISIONS/0018` (COR) authorizes for this proving slice
//! (`markdown_syntax`, `plain_text_syntax`); every other worker type is
//! refused here too, before ever spawning a process, not left for Cortex's
//! own CLI to reject.
//!
//! Deadline enforcement is not yet implemented: the subprocess call blocks
//! until Cortex's CLI exits on its own. `DECISIONS/0019` assigns FA Local
//! "scheduling, cancellation, concurrency limits, and retry decisions in
//! integrated mode" -- this first slice proves the dispatch boundary itself
//! works; a hard per-shard timeout is a real, disclosed gap for a later
//! slice, not something silently assumed away.

use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::integrations::cortex::GnatWorkerType;

/// The two worker types [`DECISIONS/0018`](https://github.com/Boswell-Digital-Solutions/COR/blob/master/DECISIONS/0018-gnat-bounded-parallel-worker-authorization.md)
/// authorizes for this proving slice. Checked before ever spawning Cortex's
/// CLI, not left for that process to reject.
pub const AUTHORIZED_WORKER_TYPES: [GnatWorkerType; 2] = [
    GnatWorkerType::MarkdownSyntax,
    GnatWorkerType::PlainTextSyntax,
];

/// A Cortex Gnat source fingerprint, matching COR's own `SourceFingerprint`
/// (`schemas/gnat-shard.schema.json`'s `source_fingerprint` object).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GnatSourceFingerprint {
    pub algorithm: String,
    pub digest: String,
    pub byte_count: u64,
    pub modified_at: String,
}

/// Everything needed to actually dispatch one already-planned Cortex Gnat
/// shard -- the full `GnatShard.v1` contract fields, plus the real
/// `local_path` that contract deliberately excludes (see
/// `source_path_token` in COR's `gnat-shard.schema.json`; the local
/// filesystem path never crosses a schema-validated contract boundary, by
/// design). This is caller-supplied, complete input: this module does not
/// derive a runnable shard from
/// [`GnatDispatchShard`](super::GnatDispatchShard) (the negotiation-time
/// envelope's shard summary), which lacks several of these fields --
/// bridging admission negotiation into a full runnable shard descriptor is
/// a separate, later concern.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GnatShardDispatchRequest {
    pub run_id: String,
    pub shard_id: String,
    pub ordinal: u32,
    pub worker_type: GnatWorkerType,
    pub source_ref: String,
    pub source_path_token: String,
    pub media_type: String,
    pub source_fingerprint: GnatSourceFingerprint,
    pub deadline_ms: u64,
    pub max_bytes: u64,
    pub local_path: PathBuf,
}

impl GnatShardDispatchRequest {
    /// Builds the `gnat-shard.schema.json`-shaped JSON Cortex's CLI expects
    /// as input -- everything except `local_path`, which travels as a
    /// separate CLI argument, never as part of this schema-validated
    /// payload.
    fn to_shard_contract_json(&self) -> Value {
        json!({
            "contract_version": "GnatShard.v1",
            "run_id": self.run_id,
            "shard_id": self.shard_id,
            "ordinal": self.ordinal,
            "worker_type": self.worker_type,
            "source_ref": self.source_ref,
            "source_path_token": self.source_path_token,
            "media_type": self.media_type,
            "source_fingerprint": {
                "algorithm": self.source_fingerprint.algorithm,
                "digest": self.source_fingerprint.digest,
                "byte_count": self.source_fingerprint.byte_count,
                "modified_at": self.source_fingerprint.modified_at,
            },
            "operation": "syntax_extract",
            "limits": {
                "deadline_ms": self.deadline_ms,
                "max_bytes": self.max_bytes,
            },
            "output_contract": "extraction-result.schema.json",
        })
    }
}

/// Truthful outcome of dispatching one Gnat shard to Cortex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GnatShardDispatchResult {
    /// Cortex ran the shard and its receipt reports `state: "complete"`.
    Completed { receipt: Value },
    /// Cortex produced a real, schema-shaped receipt, but the shard did not
    /// complete (`denied`, `stale`, or `failed`) -- a truthful outcome, not
    /// a dispatch failure.
    NotCompleted { receipt: Value },
    /// No receipt could be obtained at all: the shard's worker type is
    /// outside this proving slice, the local source file is unavailable,
    /// the subprocess could not be spawned, or it produced no parseable
    /// receipt.
    DispatchUnavailable { summary: String },
}

pub trait GnatShardDeliveryAdapter {
    fn adapter_id(&self) -> &'static str;

    fn deliver_shard(&self, request: &GnatShardDispatchRequest) -> GnatShardDispatchResult;
}

/// Config for [`CortexSubprocessGnatShardAdapter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CortexSubprocessGnatShardAdapterConfig {
    /// The Python interpreter to spawn Cortex's CLI with (e.g. `python3`).
    pub python_binary: PathBuf,
    /// The Cortex (COR) repo root -- `cortex_runtime.gnats.shard_cli` must
    /// resolve as an importable module from this directory.
    pub cortex_repo_root: PathBuf,
}

impl CortexSubprocessGnatShardAdapterConfig {
    pub fn new(python_binary: PathBuf, cortex_repo_root: PathBuf) -> Self {
        Self {
            python_binary,
            cortex_repo_root,
        }
    }
}

/// Dispatches a Gnat shard by spawning Cortex's `cortex_runtime.gnats.shard_cli`
/// as a subprocess and parsing the `GnatWorkerReceipt.v1` it prints to stdout.
#[derive(Debug, Clone)]
pub struct CortexSubprocessGnatShardAdapter {
    config: CortexSubprocessGnatShardAdapterConfig,
}

impl CortexSubprocessGnatShardAdapter {
    pub fn new(config: CortexSubprocessGnatShardAdapterConfig) -> Self {
        Self { config }
    }
}

impl GnatShardDeliveryAdapter for CortexSubprocessGnatShardAdapter {
    fn adapter_id(&self) -> &'static str {
        "cortex-subprocess-gnat-shard-delivery"
    }

    fn deliver_shard(&self, request: &GnatShardDispatchRequest) -> GnatShardDispatchResult {
        if !AUTHORIZED_WORKER_TYPES.contains(&request.worker_type) {
            return GnatShardDispatchResult::DispatchUnavailable {
                summary: format!(
                    "this proving slice only dispatches markdown_syntax or plain_text_syntax shards, not {:?}",
                    request.worker_type
                ),
            };
        }

        if !request.local_path.is_file() {
            return GnatShardDispatchResult::DispatchUnavailable {
                summary: "shard source file is unavailable for Cortex dispatch".to_owned(),
            };
        }

        let shard_json = request.to_shard_contract_json();
        let shard_path = std::env::temp_dir().join(format!(
            "fa-local-gnat-shard-{}-{}.json",
            request.shard_id,
            Uuid::new_v4()
        ));
        if let Err(error) = std::fs::write(&shard_path, shard_json.to_string()) {
            return GnatShardDispatchResult::DispatchUnavailable {
                summary: format!("could not write shard descriptor for Cortex dispatch: {error}"),
            };
        }

        let output = Command::new(&self.config.python_binary)
            .current_dir(&self.config.cortex_repo_root)
            .args(["-m", "cortex_runtime.gnats.shard_cli"])
            .arg(&shard_path)
            .arg("--local-path")
            .arg(&request.local_path)
            .output();

        let _ = std::fs::remove_file(&shard_path);

        let output = match output {
            Ok(output) => output,
            Err(error) => {
                return GnatShardDispatchResult::DispatchUnavailable {
                    summary: format!("could not spawn Cortex Gnat shard runner: {error}"),
                };
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let receipt: Value = match serde_json::from_str(stdout.trim()) {
            Ok(value) => value,
            Err(_) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return GnatShardDispatchResult::DispatchUnavailable {
                    summary: format!(
                        "Cortex Gnat shard runner produced no parseable receipt (exit {:?}): {}",
                        output.status.code(),
                        stderr.trim()
                    ),
                };
            }
        };

        match receipt.get("state").and_then(Value::as_str) {
            Some("complete") => GnatShardDispatchResult::Completed { receipt },
            Some(_) => GnatShardDispatchResult::NotCompleted { receipt },
            None => GnatShardDispatchResult::DispatchUnavailable {
                summary: "Cortex Gnat shard runner produced a receipt with no state field"
                    .to_owned(),
            },
        }
    }
}
