//! FA Local — minimal diagnostic and intake CLI
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
//! # Check FA Local contract posture and emit a structured status report
//! fa-local-run status
//!
//! # Emit FA Local's status projected onto forge-local-systems-runtime's
//! # canonical service-status schema (FC-LTA-P007)
//! fa-local-run canonical-status
//! ```
//!
//! Exit codes:
//! - 0 — operation succeeded
//! - 1 — validation failed or operational error

use std::io::{self, Read};
use std::process;

use fa_local::app::intake_service::IntakeService;
use fa_local::domain::service_status;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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

        Some("status") => {
            let facts = service_status::operational_facts();
            println!("{{");
            println!("  \"service\": \"fa-local-operator\",");
            println!("  \"version\": \"{VERSION}\",");
            println!("  \"execution_enabled\": {},", facts.execution_enabled);
            println!("  \"writeback_wired\": {},", facts.writeback_wired);
            println!(
                "  \"note\": \"bounded local execution consumer — execution bridge v1 pending Phase X4 wiring\""
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
                "  status            Emit a structured FA Local posture and readiness report"
            );
            eprintln!(
                "  canonical-status  Emit FA Local's status on forge-local-systems-runtime's canonical schema (FC-LTA-P007)"
            );
            eprintln!("");
            eprintln!("OPTIONS FOR validate:");
            eprintln!("  --request <FILE>   Read request JSON from file (default: stdin)");
            eprintln!("");
            eprintln!("EXIT CODES:");
            eprintln!("  0   Success");
            eprintln!("  1   Validation failure or error");
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
