//! FA Local — minimal diagnostic, intake, and route-decision CLI
//!
//! Usage:
//!
//! ```bash
//! # Validate an execution request from a file
//! fa-local-run validate --request request.json
//!
//! # Validate an execution request from stdin
//! cat request.json | fa-local-run validate
//!
//! # Resolve a bounded route decision from a request, a requester-trust
//! # envelope, a policy artifact, and a capability registry
//! fa-local-run route \
//!   --request request.json \
//!   --requester-trust requester-trust.json \
//!   --policy policy-artifact.json \
//!   --capability-registry capability-registry.json
//!
//! # Resolve a route decision and, if it admits execution, validate the
//! # plan, dispatch through an adapter, and record forensic evidence --
//! # all in one bounded run
//! fa-local-run execute \
//!   --request request.json \
//!   --requester-trust requester-trust.json \
//!   --policy policy-artifact.json \
//!   --capability-registry capability-registry.json \
//!   --plan execution-plan.json \
//!   --local-file-write-root ./delivery \
//!   --forensic-sqlite ./forensics.sqlite3
//!
//! # Query the recorded forensic events back out
//! fa-local-run forensics-query --sqlite ./forensics.sqlite3 --event-type execution_status_observed
//!
//! # Check FA Local contract posture and emit a structured status report
//! fa-local-run status
//!
//! # Emit FA Local's status projected onto forge-local-systems-runtime's
//! # canonical service-status schema (FC-LTA-P007)
//! fa-local-run canonical-status
//! ```
//!
//! Exit codes:
//! - 0 — operation succeeded (for `route`, the resolved posture admits execution)
//! - 1 — validation failed, the route decision does not admit execution, or an operational error occurred

use std::io::{self, Read};
use std::process;

use fa_local::ExecutionState;
use fa_local::adapters::exports::ForensicEventExportAdapter;
use fa_local::adapters::exports::jsonl_forensic_export::{
    JsonlForensicExportAdapter, JsonlForensicExportAdapterConfig,
};
use fa_local::adapters::exports::sqlite_forensic_store::SqliteForensicStore;
use fa_local::app::decision_service::DecisionService;
use fa_local::app::execution_pipeline_service::{
    AdapterSelection, CapabilityScopedAdapterSelection, ExecutionPipelineInputs,
    ExecutionPipelineService,
};
use fa_local::app::intake_service::IntakeService;
use fa_local::domain::posture::RouteResolutionContext;
use fa_local::domain::service_status;
use serde_json::Value;

const VERSION: &str = env!("CARGO_PKG_VERSION");

enum ForensicSink {
    Jsonl(JsonlForensicExportAdapter),
    Sqlite(SqliteForensicStore),
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("validate") => {
            let request_path = args
                .windows(2)
                .find(|w| w[0] == "--request")
                .map(|w| w[1].as_str());

            let bytes = match request_path {
                Some(path) => match std::fs::read(path) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("error: could not read request file {path:?}: {e}");
                        process::exit(1);
                    }
                },
                None => {
                    let mut buf = Vec::new();
                    if let Err(e) = io::stdin().read_to_end(&mut buf) {
                        eprintln!("error: could not read from stdin: {e}");
                        process::exit(1);
                    }
                    buf
                }
            };

            let service = IntakeService::default();
            match service.validate_request_bytes(&bytes) {
                Ok(result) => {
                    println!("{{");
                    println!("  \"status\": \"valid\",");
                    println!("  \"request_id\": \"{}\",", result.request.request_id);
                    println!(
                        "  \"correlation_id\": \"{}\",",
                        result.request.correlation_id
                    );
                    println!(
                        "  \"environment_mode\": \"{:?}\"",
                        result.request.environment_mode
                    );
                    println!("}}");
                    process::exit(0);
                }
                Err(e) => {
                    eprintln!("{{");
                    eprintln!("  \"status\": \"invalid\",");
                    eprintln!("  \"error\": \"{e}\"");
                    eprintln!("}}");
                    process::exit(1);
                }
            }
        }

        Some("route") => {
            let flag_path = |flag: &str| -> Option<&str> {
                args.windows(2)
                    .find(|w| w[0] == flag)
                    .map(|w| w[1].as_str())
            };

            let paths = (
                flag_path("--request"),
                flag_path("--requester-trust"),
                flag_path("--policy"),
                flag_path("--capability-registry"),
            );
            let (request_path, requester_trust_path, policy_path, capability_registry_path) =
                match paths {
                    (Some(r), Some(t), Some(p), Some(c)) => (r, t, p, c),
                    _ => {
                        eprintln!(
                            "error: route requires --request, --requester-trust, --policy, and --capability-registry"
                        );
                        process::exit(1);
                    }
                };

            let request = read_json_file(request_path);
            let requester_trust = read_json_file(requester_trust_path);
            let policy = read_json_file(policy_path);
            let capability_registry = read_json_file(capability_registry_path);

            match DecisionService.resolve_route_decision(
                &request,
                &requester_trust,
                &policy,
                &capability_registry,
                RouteResolutionContext::default(),
            ) {
                Ok(route_decision) => {
                    let exit_code = if route_decision.execution_allowed {
                        0
                    } else {
                        1
                    };
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&route_decision)
                            .expect("route decision serializes")
                    );
                    process::exit(exit_code);
                }
                Err(e) => {
                    eprintln!("{{");
                    eprintln!("  \"status\": \"error\",");
                    eprintln!("  \"error\": \"{e}\"");
                    eprintln!("}}");
                    process::exit(1);
                }
            }
        }

        Some("execute") => {
            let flag_path = |flag: &str| -> Option<&str> {
                args.windows(2)
                    .find(|w| w[0] == flag)
                    .map(|w| w[1].as_str())
            };

            let paths = (
                flag_path("--request"),
                flag_path("--requester-trust"),
                flag_path("--policy"),
                flag_path("--capability-registry"),
            );
            let (request_path, requester_trust_path, policy_path, capability_registry_path) =
                match paths {
                    (Some(r), Some(t), Some(p), Some(c)) => (r, t, p, c),
                    _ => {
                        eprintln!(
                            "error: execute requires --request, --requester-trust, --policy, and --capability-registry"
                        );
                        process::exit(1);
                    }
                };

            let request = read_json_file(request_path);
            let requester_trust = read_json_file(requester_trust_path);
            let policy = read_json_file(policy_path);
            let capability_registry = read_json_file(capability_registry_path);
            let plan = flag_path("--plan").map(read_json_file);

            let local_file_write_root = flag_path("--local-file-write-root");
            let nmap_binary = flag_path("--nmap-binary");
            let adapter_selection = match (local_file_write_root, nmap_binary) {
                (Some(root), None) => Some(AdapterSelection::LocalFileWrite {
                    delivery_root: root.into(),
                }),
                (None, Some(binary)) => {
                    let scan_profile = match flag_path("--nmap-profile") {
                        None | Some("loopback") => {
                            fa_local::adapters::execution_delivery::nmap_preflight::NmapScanProfile::LoopbackTcpConnectV1
                        }
                        Some("private-subnet") => {
                            fa_local::adapters::execution_delivery::nmap_preflight::NmapScanProfile::AuthorizedPrivateSubnetTcpConnectV1
                        }
                        Some(other) => {
                            eprintln!(
                                "error: unknown --nmap-profile {other:?} (expected loopback or private-subnet)"
                            );
                            process::exit(1);
                        }
                    };
                    Some(AdapterSelection::NmapPreflight {
                        nmap_binary: binary.into(),
                        scan_profile,
                    })
                }
                (None, None) => None,
                (Some(_), Some(_)) => {
                    eprintln!(
                        "error: --local-file-write-root and --nmap-binary are mutually exclusive"
                    );
                    process::exit(1);
                }
            };

            let additional_adapters: Vec<CapabilityScopedAdapterSelection> = args
                .windows(2)
                .filter(|w| w[0] == "--adapter")
                .map(|w| parse_capability_scoped_adapter(&w[1]))
                .collect::<Result<Vec<_>, String>>()
                .unwrap_or_else(|e| {
                    eprintln!("error: {e}");
                    process::exit(1);
                });

            let forensic_export_path = flag_path("--forensic-export");
            let forensic_sqlite_path = flag_path("--forensic-sqlite");
            let forensic_sink = match (forensic_export_path, forensic_sqlite_path) {
                (Some(_), Some(_)) => {
                    eprintln!(
                        "error: --forensic-export and --forensic-sqlite are mutually exclusive"
                    );
                    process::exit(1);
                }
                (Some(path), None) => Some(ForensicSink::Jsonl(JsonlForensicExportAdapter::new(
                    JsonlForensicExportAdapterConfig::new(path.into()),
                ))),
                (None, Some(path)) => Some(ForensicSink::Sqlite(
                    SqliteForensicStore::open(std::path::Path::new(path)).unwrap_or_else(|e| {
                        eprintln!("error: could not open forensic sqlite store {path:?}: {e}");
                        process::exit(1);
                    }),
                )),
                (None, None) => None,
            };
            let forensic_export_adapter: Option<&dyn ForensicEventExportAdapter> =
                match &forensic_sink {
                    Some(ForensicSink::Jsonl(adapter)) => Some(adapter),
                    Some(ForensicSink::Sqlite(store)) => Some(store),
                    None => None,
                };

            let dispatch_mode = if args.iter().any(|arg| arg == "--per-step-dispatch") {
                fa_local::app::execution_pipeline_service::DispatchMode::PerStep
            } else {
                fa_local::app::execution_pipeline_service::DispatchMode::WholeRoute
            };

            match ExecutionPipelineService.run(
                ExecutionPipelineInputs {
                    request: &request,
                    requester_trust: &requester_trust,
                    policy: &policy,
                    capability_registry: &capability_registry,
                    execution_plan: plan.as_ref(),
                },
                adapter_selection,
                additional_adapters,
                dispatch_mode,
                forensic_export_adapter,
                RouteResolutionContext::default(),
            ) {
                Ok(outcome) => {
                    let mut output =
                        serde_json::json!({ "route_decision": outcome.route_decision });

                    if let Some(trace) = &outcome.execution_trace {
                        let statuses: Vec<_> =
                            trace.statuses.iter().map(|status| &status.status).collect();
                        output["execution_trace"] =
                            serde_json::to_value(statuses).expect("execution trace serializes");
                    }

                    let exit_code = if outcome.plan_denial.is_some() {
                        1
                    } else if let Some(trace) = &outcome.execution_trace {
                        i32::from(!matches!(
                            trace.final_status().status.state,
                            ExecutionState::Completed
                                | ExecutionState::CompletedWithConstraints
                                | ExecutionState::PartialSuccess
                        ))
                    } else {
                        i32::from(!outcome.route_decision.execution_allowed)
                    };

                    if let Some(plan_denial) = &outcome.plan_denial {
                        output["plan_denial"] =
                            serde_json::to_value(plan_denial).expect("denial guard serializes");
                    }

                    output["forensic_records"] = serde_json::Value::Array(
                        outcome
                            .forensic_records
                            .iter()
                            .map(|record| {
                                serde_json::json!({
                                    "event": record.event.event,
                                    "export_reference": record.export_reference,
                                })
                            })
                            .collect(),
                    );

                    println!(
                        "{}",
                        serde_json::to_string_pretty(&output).expect("output serializes")
                    );
                    process::exit(exit_code);
                }
                Err(e) => {
                    eprintln!("{{");
                    eprintln!("  \"status\": \"error\",");
                    eprintln!("  \"error\": \"{e}\"");
                    eprintln!("}}");
                    process::exit(1);
                }
            }
        }

        Some("forensics-query") => {
            let flag_path = |flag: &str| -> Option<&str> {
                args.windows(2)
                    .find(|w| w[0] == flag)
                    .map(|w| w[1].as_str())
            };

            let sqlite_path = flag_path("--sqlite").unwrap_or_else(|| {
                eprintln!("error: forensics-query requires --sqlite");
                process::exit(1);
            });
            let store = SqliteForensicStore::open(std::path::Path::new(sqlite_path))
                .unwrap_or_else(|e| {
                    eprintln!("error: could not open forensic sqlite store {sqlite_path:?}: {e}");
                    process::exit(1);
                });

            let correlation_id = flag_path("--correlation-id");
            let event_type = flag_path("--event-type");

            let result = match (correlation_id, event_type) {
                (Some(value), None) => {
                    let correlation_id = value
                        .parse::<uuid::Uuid>()
                        .map(fa_local::CorrelationId::from_uuid)
                        .unwrap_or_else(|e| {
                            eprintln!("error: invalid --correlation-id {value:?}: {e}");
                            process::exit(1);
                        });
                    store.query_by_correlation_id(correlation_id)
                }
                (None, Some(value)) => {
                    let event_type: fa_local::domain::forensics::ForensicEventType =
                        serde_json::from_value(serde_json::Value::String(value.to_owned()))
                            .unwrap_or_else(|e| {
                                eprintln!("error: invalid --event-type {value:?}: {e}");
                                process::exit(1);
                            });
                    store.query_by_event_type(event_type)
                }
                _ => {
                    eprintln!(
                        "error: forensics-query requires exactly one of --correlation-id or --event-type"
                    );
                    process::exit(1);
                }
            };

            match result {
                Ok(events) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&events).expect("events serialize")
                    );
                    process::exit(0);
                }
                Err(e) => {
                    eprintln!("{{");
                    eprintln!("  \"status\": \"error\",");
                    eprintln!("  \"error\": \"{e}\"");
                    eprintln!("}}");
                    process::exit(1);
                }
            }
        }

        Some("gnat-dispatch") => {
            let flag_path = |flag: &str| -> Option<&str> {
                args.windows(2)
                    .find(|w| w[0] == flag)
                    .map(|w| w[1].as_str())
            };

            let envelope_path = flag_path("--envelope").unwrap_or_else(|| {
                eprintln!("error: gnat-dispatch requires --envelope");
                process::exit(1);
            });
            let shards_path = flag_path("--shards").unwrap_or_else(|| {
                eprintln!("error: gnat-dispatch requires --shards");
                process::exit(1);
            });
            let cortex_repo_root = flag_path("--cortex-repo-root").unwrap_or_else(|| {
                eprintln!("error: gnat-dispatch requires --cortex-repo-root");
                process::exit(1);
            });
            let cortex_python = flag_path("--cortex-python").unwrap_or("python3");

            let envelope_value = read_json_file(envelope_path);
            let envelope =
                fa_local::integrations::cortex::GnatDispatchEnvelope::load_contract_value(
                    &envelope_value,
                )
                .unwrap_or_else(|e| {
                    eprintln!("error: invalid --envelope: {e}");
                    process::exit(1);
                });

            let shards_value = read_json_file(shards_path);
            let shard_requests: Vec<fa_local::integrations::cortex::GnatShardDispatchRequest> =
                serde_json::from_value(shards_value).unwrap_or_else(|e| {
                    eprintln!("error: invalid --shards: {e}");
                    process::exit(1);
                });

            // Hardcoded rather than CLI-configurable: there is no persistent
            // Gnat-capability config yet, so this proving slice always
            // reports FA Local's fixed default capability state.
            let capabilities =
                fa_local::integrations::cortex::GnatFaLocalCapabilityState::ready_default();
            let adapter = fa_local::integrations::cortex::CortexSubprocessGnatShardAdapter::new(
                fa_local::integrations::cortex::CortexSubprocessGnatShardAdapterConfig::new(
                    cortex_python.into(),
                    cortex_repo_root.into(),
                ),
            );

            match fa_local::app::gnat_dispatch_pipeline_service::GnatDispatchPipelineService.run(
                &envelope,
                &capabilities,
                &shard_requests,
                &adapter,
            ) {
                Ok(outcome) => {
                    let (output, exit_code) = gnat_dispatch_outcome_to_json(outcome);
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&output).expect("output serializes")
                    );
                    process::exit(exit_code);
                }
                Err(e) => {
                    eprintln!("{{");
                    eprintln!("  \"status\": \"error\",");
                    eprintln!("  \"error\": \"{e}\"");
                    eprintln!("}}");
                    process::exit(1);
                }
            }
        }

        Some("status") => {
            let facts = service_status::operational_facts();
            println!("{{");
            println!("  \"service\": \"fa-local-operator\",");
            println!("  \"version\": \"{VERSION}\",");
            println!("  \"execution_enabled\": {},", facts.execution_enabled);
            println!("  \"writeback_wired\": {},", facts.writeback_wired);
            println!(
                "  \"note\": \"bounded local execution consumer — execution enabled; DataForge Local writeback pending Phase X4 wiring\""
            );
            println!("}}");
            process::exit(0);
        }

        Some("canonical-status") => {
            match service_status::build_canonical_service_status_envelope() {
                Ok(envelope) => {
                    let exit_code = if envelope["state"] == "ready" { 0 } else { 1 };
                    println!(
                        "{}",
                        serde_json::to_string(&envelope).expect("envelope serializes")
                    );
                    process::exit(exit_code);
                }
                Err(e) => {
                    eprintln!("{{");
                    eprintln!("  \"status\": \"error\",");
                    eprintln!("  \"error\": \"{e}\"");
                    eprintln!("}}");
                    process::exit(1);
                }
            }
        }

        Some("--version") | Some("-V") => {
            println!("fa-local-run {VERSION}");
            process::exit(0);
        }

        Some("--help") | Some("-h") | None => {
            eprintln!("FA Local — bounded local execution control service");
            eprintln!("");
            eprintln!("USAGE:");
            eprintln!("  fa-local-run <COMMAND>");
            eprintln!("");
            eprintln!("COMMANDS:");
            eprintln!(
                "  validate          Validate a bounded execution request against FA Local contract schema"
            );
            eprintln!(
                "  route             Resolve a bounded route decision from a request, requester-trust envelope, policy artifact, and capability registry"
            );
            eprintln!(
                "  execute           Resolve a route decision and, if admitted, validate the plan, dispatch through an adapter, and record forensic evidence"
            );
            eprintln!(
                "  forensics-query   Query a SQLite forensic store by correlation-id or event-type"
            );
            eprintln!(
                "  gnat-dispatch     Negotiate a Cortex Gnat dispatch envelope and, if admitted, dispatch every declared shard to Cortex"
            );
            eprintln!(
                "  status            Emit a structured FA Local posture and readiness report"
            );
            eprintln!(
                "  canonical-status  Emit FA Local's status on forge-local-systems-runtime's canonical schema (FC-LTA-P007)"
            );
            eprintln!("");
            eprintln!("OPTIONS FOR validate:");
            eprintln!("  --request <FILE>   Read request JSON from file (default: stdin)");
            eprintln!("");
            eprintln!("OPTIONS FOR route (all required):");
            eprintln!("  --request <FILE>              Execution request JSON");
            eprintln!("  --requester-trust <FILE>      Requester-trust envelope JSON");
            eprintln!("  --policy <FILE>               Policy artifact JSON");
            eprintln!("  --capability-registry <FILE>  Capability registry JSON");
            eprintln!("");
            eprintln!("OPTIONS FOR execute (same required flags as route, plus):");
            eprintln!(
                "  --plan <FILE>                    Execution plan JSON (required if the route admits execution)"
            );
            eprintln!(
                "  --local-file-write-root <DIR>    Dispatch through the local-file-write adapter"
            );
            eprintln!(
                "  --nmap-binary <FILE>             Dispatch through the nmap-preflight adapter"
            );
            eprintln!(
                "  --nmap-profile <loopback|private-subnet>  Nmap scan profile (default: loopback)"
            );
            eprintln!(
                "  --forensic-export <FILE>         Append every recorded forensic event to this JSONL file"
            );
            eprintln!(
                "  --forensic-sqlite <FILE>         Record every forensic event into a queryable SQLite store (mutually exclusive with --forensic-export)"
            );
            eprintln!(
                "  --per-step-dispatch              Dispatch each declared plan step to its own capability-scoped adapter, one call per step, instead of one call for the whole plan"
            );
            eprintln!(
                "  --adapter <CAP_UUID>:<KIND>:<PARAMS>  Register an additional adapter for an explicit capability (repeatable); most useful with --per-step-dispatch on a heterogeneous plan"
            );
            eprintln!("      KIND=local-file-write:  PARAMS=<DIR>");
            eprintln!(
                "      KIND=nmap-preflight:    PARAMS=<BINARY>[:<loopback|private-subnet>] (default: loopback)"
            );
            eprintln!(
                "      DIR and BINARY must not themselves contain ':' -- the value is split on ':'"
            );
            eprintln!("");
            eprintln!("OPTIONS FOR forensics-query:");
            eprintln!("  --sqlite <FILE>              SQLite forensic store to query (required)");
            eprintln!(
                "  --correlation-id <UUID>      Return events for this correlation id, oldest first"
            );
            eprintln!(
                "  --event-type <TYPE>          Return events of this type, oldest first (denial_issued, route_decision_resolved, review_package_prepared, execution_status_observed)"
            );
            eprintln!("  (exactly one of --correlation-id or --event-type is required)");
            eprintln!("");
            eprintln!("OPTIONS FOR gnat-dispatch (all required except --cortex-python):");
            eprintln!(
                "  --envelope <FILE>            A GnatDispatchEnvelope.v1 JSON file (Cortex-constructed)"
            );
            eprintln!(
                "  --shards <FILE>              A JSON array of full shard descriptors, one per shard declared in the envelope --"
            );
            eprintln!(
                "                               each needs every GnatShard.v1 field plus local_path, which the envelope"
            );
            eprintln!(
                "                               deliberately never carries; every descriptor must agree with the"
            );
            eprintln!(
                "                               envelope's own run_id/shard_id/worker_type/source_ref"
            );
            eprintln!(
                "  --cortex-repo-root <DIR>     The Cortex (COR) checkout root -- cortex_runtime.gnats.shard_cli must resolve from here"
            );
            eprintln!(
                "  --cortex-python <BINARY>     Python interpreter to spawn Cortex's CLI with (default: python3)"
            );
            eprintln!("");
            eprintln!("EXIT CODES:");
            eprintln!(
                "  0   Success (for route/execute: the resolved posture admits execution and, for execute, it completed)"
            );
            eprintln!(
                "  1   Validation failure, a non-admitting or non-completing outcome, or an operational error"
            );
            let code = if args.get(1).map(String::as_str) == Some("--help")
                || args.get(1).map(String::as_str) == Some("-h")
            {
                0
            } else {
                1
            };
            process::exit(code);
        }

        Some(unknown) => {
            eprintln!("error: unknown command {unknown:?}");
            eprintln!("Run 'fa-local-run --help' for usage.");
            process::exit(1);
        }
    }
}

fn read_json_file(path: &str) -> Value {
    let bytes = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("error: could not read file {path:?}: {e}");
        process::exit(1);
    });
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        eprintln!("error: could not parse {path:?} as JSON: {e}");
        process::exit(1);
    })
}

/// Renders a [`GnatDispatchRunOutcome`](fa_local::app::gnat_dispatch_pipeline_service::GnatDispatchRunOutcome)
/// as the `gnat-dispatch` command's JSON output and exit code. Exit 0 only
/// for a fully dispatched run where every shard's receipt reports
/// `state: "complete"`; a denial, a serial-fallback report, or any
/// not-completed or unavailable shard exits 1 -- the full truthful detail
/// is still always printed, never traded away for a clean exit code.
fn gnat_dispatch_outcome_to_json(
    outcome: fa_local::app::gnat_dispatch_pipeline_service::GnatDispatchRunOutcome,
) -> (Value, i32) {
    use fa_local::app::gnat_dispatch_pipeline_service::GnatDispatchRunOutcome;
    use fa_local::integrations::cortex::GnatShardDispatchResult;

    match outcome {
        GnatDispatchRunOutcome::Denied(denial) => (
            serde_json::json!({
                "outcome": "denied",
                "denial": denial,
            }),
            1,
        ),
        GnatDispatchRunOutcome::SerialFallbackPermitted(admission) => (
            serde_json::json!({
                "outcome": "serial_fallback_permitted",
                "admission": admission,
            }),
            1,
        ),
        GnatDispatchRunOutcome::Dispatched {
            admission,
            shard_results,
        } => {
            let mut all_completed = true;
            let shard_results_json: Vec<Value> = shard_results
                .into_iter()
                .map(|(shard_id, result)| match result {
                    GnatShardDispatchResult::Completed { receipt } => serde_json::json!({
                        "shard_id": shard_id,
                        "outcome": "completed",
                        "receipt": receipt,
                    }),
                    GnatShardDispatchResult::NotCompleted { receipt } => {
                        all_completed = false;
                        serde_json::json!({
                            "shard_id": shard_id,
                            "outcome": "not_completed",
                            "receipt": receipt,
                        })
                    }
                    GnatShardDispatchResult::DispatchUnavailable { summary } => {
                        all_completed = false;
                        serde_json::json!({
                            "shard_id": shard_id,
                            "outcome": "dispatch_unavailable",
                            "summary": summary,
                        })
                    }
                })
                .collect();

            (
                serde_json::json!({
                    "outcome": "dispatched",
                    "admission": admission,
                    "shard_results": shard_results_json,
                }),
                i32::from(!all_completed),
            )
        }
    }
}

/// Parses one `--adapter <CAP_UUID>:<KIND>:<PARAMS>` value. `PARAMS` is
/// `<DIR>` for `local-file-write` or `<BINARY>[:<loopback|private-subnet>]`
/// for `nmap-preflight`; the value is split on `:`, so neither `DIR` nor
/// `BINARY` may themselves contain a `:`.
fn parse_capability_scoped_adapter(
    value: &str,
) -> Result<CapabilityScopedAdapterSelection, String> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() < 3 {
        return Err(format!(
            "invalid --adapter value {value:?} (expected <CAPABILITY_UUID>:<local-file-write|nmap-preflight>:<PARAMS>)"
        ));
    }

    let capability_id = parts[0]
        .parse::<uuid::Uuid>()
        .map(fa_local::CapabilityId::from_uuid)
        .map_err(|e| format!("invalid --adapter capability id {:?}: {e}", parts[0]))?;

    let selection = match parts[1] {
        "local-file-write" => {
            if parts.len() != 3 {
                return Err(format!(
                    "invalid --adapter value {value:?}: local-file-write takes exactly one parameter, the delivery directory"
                ));
            }
            AdapterSelection::LocalFileWrite {
                delivery_root: parts[2].into(),
            }
        }
        "nmap-preflight" => {
            if parts.len() < 3 || parts.len() > 4 {
                return Err(format!(
                    "invalid --adapter value {value:?}: nmap-preflight takes <BINARY>[:<loopback|private-subnet>]"
                ));
            }
            let scan_profile = match parts.get(3) {
                None | Some(&"loopback") => {
                    fa_local::adapters::execution_delivery::nmap_preflight::NmapScanProfile::LoopbackTcpConnectV1
                }
                Some(&"private-subnet") => {
                    fa_local::adapters::execution_delivery::nmap_preflight::NmapScanProfile::AuthorizedPrivateSubnetTcpConnectV1
                }
                Some(other) => {
                    return Err(format!(
                        "invalid --adapter nmap profile {other:?} (expected loopback or private-subnet)"
                    ));
                }
            };
            AdapterSelection::NmapPreflight {
                nmap_binary: parts[2].into(),
                scan_profile,
            }
        }
        other => {
            return Err(format!(
                "unknown --adapter kind {other:?} (expected local-file-write or nmap-preflight)"
            ));
        }
    };

    Ok(CapabilityScopedAdapterSelection {
        capability_id,
        selection,
    })
}
