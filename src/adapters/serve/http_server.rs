//! `tiny_http`-backed listener for FA Local's one read-only serving route.
//!
//! Authorized by `BDS-FAL-DAEMON-v0.1`'s implementation scoping packet,
//! "Route contract" and "Startup and configuration". Routing, request
//! parsing, and response writing only -- no business logic beyond what is
//! needed to call `app::serve_service::ServeService::lookup` and translate
//! its answer into the exact response shape the packet freezes. This module
//! never calls `domain::capabilities::CapabilityRegistryLoader::admit_execution_request`
//! and never accepts a write.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::json;
use tiny_http::{Header as HttpHeader, Method, Request, Response, Server};
use uuid::Uuid;

use crate::app::serve_service::{ServeLookupResult, ServeService};
use crate::domain::shared::CapabilityId;
use crate::errors::{FaLocalError, FaLocalResult};

use super::token_verify::verify_bearer_token;

/// Default port this daemon listens on when `--port` is not given -- claimed
/// in the ecosystem's canonical `PORT_REGISTRY.md` (Agent Layer) ahead of
/// this packet's acceptance, per that file's own "claim before coding" rule.
pub const DEFAULT_SERVE_PORT: u16 = 8011;

/// Default environment variable name carrying the `kid -> public key` map,
/// overridable with `--public-keys-env`.
pub const DEFAULT_PUBLIC_KEYS_ENV: &str = "FA_LOCAL_SERVE_PUBLIC_KEYS";

const CAPABILITIES_PATH_PREFIX: &str = "/api/v1/capabilities/";

/// Poll interval for `Server::recv_timeout` between checks of the `SIGHUP`
/// reload flag -- short enough that a reload lands promptly, long enough
/// not to spin the process.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);

extern "C" fn request_reload(_signum: libc::c_int) {
    // Signal-safe: only sets an atomic flag. The actual reload (file I/O,
    // schema validation) happens on the main serve loop, never inside the
    // handler itself.
    RELOAD_REQUESTED.store(true, Ordering::SeqCst);
}

/// Installs the `SIGHUP` handler that requests a registry reload. Safe to
/// call more than once -- `libc::signal` simply replaces the prior handler
/// with an identical one.
fn install_sighup_handler() {
    unsafe {
        libc::signal(
            libc::SIGHUP,
            request_reload as *const () as libc::sighandler_t,
        );
    }
}

/// Parses `FA_LOCAL_SERVE_PUBLIC_KEYS` (or the `--public-keys-env`-named
/// variable) into a `kid -> public key` map.
///
/// Unset or blank is not an error -- it is the fail-closed default (an
/// empty map denies every request, see
/// [`token_verify::verify_bearer_token`](super::token_verify::verify_bearer_token)).
/// Present-but-malformed JSON is a startup configuration error, reported
/// loudly rather than silently downgraded to "no keys" -- mirroring
/// `dataforge-Local`'s own `RunTokenVerifier::from_env`, which raises
/// rather than degrading to a deny-everything posture the operator never
/// hears about.
pub fn load_public_keys_from_env(var_name: &str) -> Result<HashMap<String, String>, String> {
    let raw = std::env::var(var_name).unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(HashMap::new());
    }
    serde_json::from_str(raw)
        .map_err(|e| format!("{var_name} is not a JSON object of kid -> public key: {e}"))
}

/// Runs the blocking `serve` listener until the process is killed.
///
/// Binds `127.0.0.1:<port>`, answers exactly
/// `GET /api/v1/capabilities/{capability_id}`, and reloads `service`'s
/// registry on `SIGHUP` (a failed reload logs and keeps serving the
/// previous good registry -- see `app::serve_service::ServeService::reload`).
pub fn run_server(
    service: &ServeService,
    port: u16,
    public_keys: &HashMap<String, String>,
) -> FaLocalResult<()> {
    install_sighup_handler();

    let address = format!("127.0.0.1:{port}");
    let server = Server::http(&address)
        .map_err(|e| FaLocalError::InternalInvariant(format!("could not bind {address}: {e}")))?;

    loop {
        if RELOAD_REQUESTED.swap(false, Ordering::SeqCst) {
            match service.reload() {
                Ok(()) => eprintln!("info: fa-local-run serve: registry reloaded"),
                Err(e) => eprintln!(
                    "warning: fa-local-run serve: registry reload failed, continuing to serve the previous registry: {e}"
                ),
            }
        }

        match server.recv_timeout(POLL_INTERVAL) {
            Ok(Some(request)) => handle_request(request, service, public_keys),
            Ok(None) => {}
            Err(e) => eprintln!("warning: fa-local-run serve: could not receive a request: {e}"),
        }
    }
}

fn handle_request(request: Request, service: &ServeService, public_keys: &HashMap<String, String>) {
    let response = build_response(&request, service, public_keys);
    if let Err(e) = request.respond(response) {
        eprintln!("warning: fa-local-run serve: could not write a response: {e}");
    }
}

fn build_response(
    request: &Request,
    service: &ServeService,
    public_keys: &HashMap<String, String>,
) -> Response<Cursor<Vec<u8>>> {
    if *request.method() != Method::Get {
        return json_response(404, json!({ "error": "not_found" }));
    }

    let url = request.url();
    let path = url.split('?').next().unwrap_or(url);
    let Some(id_segment) = path.strip_prefix(CAPABILITIES_PATH_PREFIX) else {
        return json_response(404, json!({ "error": "not_found" }));
    };
    if id_segment.is_empty() || id_segment.contains('/') {
        return json_response(404, json!({ "error": "not_found" }));
    }

    let Some(capability_id) = parse_capability_id_segment(id_segment) else {
        return json_response(400, json!({ "error": "malformed_capability_id" }));
    };

    let authorization_header = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Authorization"))
        .map(|header| header.value.as_str());

    if verify_bearer_token(authorization_header, public_keys).is_err() {
        return json_response(401, json!({ "error": "unauthorized" }));
    }

    match service.lookup(capability_id) {
        ServeLookupResult::Found(record) => json_response(
            200,
            serde_json::to_value(record).expect("CapabilityRecord always serializes"),
        ),
        ServeLookupResult::NotFound => {
            json_response(404, json!({ "error": "capability_not_found" }))
        }
        ServeLookupResult::RegistryUnavailable => {
            json_response(503, json!({ "error": "registry_unavailable" }))
        }
    }
}

/// Validates `segment` against the same strict `uuid_string` shape
/// `schemas/capability-registry.schema.json`'s `$def` requires
/// (`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)
/// before ever asking `uuid::Uuid` to parse it -- `Uuid::parse_str` alone is
/// looser than that shape (it also accepts bare hex, braces, and `urn:uuid:`
/// forms), and the packet's 400 case is specifically "does not match
/// `uuid_string`", not "`uuid::Uuid` cannot parse it".
fn parse_capability_id_segment(segment: &str) -> Option<CapabilityId> {
    let bytes = segment.as_bytes();
    if bytes.len() != 36 {
        return None;
    }
    for (index, &byte) in bytes.iter().enumerate() {
        let must_be_hyphen = matches!(index, 8 | 13 | 18 | 23);
        if must_be_hyphen {
            if byte != b'-' {
                return None;
            }
        } else if !byte.is_ascii_hexdigit() {
            return None;
        }
    }
    Uuid::parse_str(segment).ok().map(CapabilityId::from_uuid)
}

fn json_response(status: u16, body: serde_json::Value) -> Response<Cursor<Vec<u8>>> {
    let content_type = HttpHeader::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("static content-type header is valid");
    Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(content_type)
}
