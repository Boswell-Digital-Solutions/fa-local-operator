//! Full-stack `adapters::serve::http_server` cases from
//! `docs/plans/active/BDS_FAL_DAEMON_v0.1/02_IMPLEMENTATION_SCOPING_PACKET.md`'s
//! "Test allowlist and negative cases": case 1 (200 success), case 2 (404
//! not found), case 3 (400 malformed id, before any registry lookup), and
//! case 9 (`FA_LOCAL_SERVE_ENABLED` unset means no listener at all, not
//! just a 404 on the route).
//!
//! Each HTTP-level test spawns a real `adapters::serve::run_server` on its
//! own fixed loopback port and drives it with real `ureq` requests -- the
//! same HTTP client this crate already uses against `dataforge-Local` and
//! `neuronforge-local-operator` (`integrations::df_local`,
//! `integrations::neuronforge_local`).

mod support;

use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

use fa_local::adapters::serve::{REQUIRED_SCOPE, run_server};
use fa_local::app::serve_service::ServeService;
use support::{TempRegistryFile, mint_serve_test_token};

fn spawn_test_server(
    registry_file: &TempRegistryFile,
    port: u16,
    public_keys: HashMap<String, String>,
) {
    let registry_path = registry_file.path().to_path_buf();
    std::thread::spawn(move || {
        let service =
            ServeService::load(&registry_path).expect("fixture registry loads for test server");
        run_server(&service, port, &public_keys).expect("test server runs");
    });
    wait_for_port(port);
}

fn wait_for_port(port: u16) {
    for _ in 0..200 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("serve test server on port {port} did not start in time");
}

/// Sends one `GET` to the test server and returns `(status, body)`,
/// regardless of whether `ureq` treated the response as success or error --
/// matching how `integrations::df_local`/`integrations::neuronforge_local`
/// already unwrap `ureq::Error::Status` in this crate.
fn get(port: u16, path: &str, bearer_token: Option<&str>) -> (u16, serde_json::Value) {
    let url = format!("http://127.0.0.1:{port}{path}");
    let mut request = ureq::get(&url);
    if let Some(token) = bearer_token {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    match request.call() {
        Ok(response) => {
            let status = response.status();
            let body = response.into_json().unwrap_or(serde_json::Value::Null);
            (status, body)
        }
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_json().unwrap_or(serde_json::Value::Null);
            (status, body)
        }
        Err(ureq::Error::Transport(transport)) => {
            panic!("transport error talking to test server: {transport}")
        }
    }
}

#[test]
fn a_well_formed_capability_id_present_in_the_registry_returns_200_with_the_record() {
    let registry_json = support::load_fixture_json("valid", "capability-registry-basic.json");
    let registry_file = TempRegistryFile::write(&registry_json);
    let minted = mint_serve_test_token("kid-http-200", REQUIRED_SCOPE, 3600);
    let mut public_keys = HashMap::new();
    public_keys.insert(minted.kid.clone(), minted.public_key_pem.clone());

    let port = 18111;
    spawn_test_server(&registry_file, port, public_keys);

    let (status, body) = get(
        port,
        "/api/v1/capabilities/44444444-4444-4444-8444-444444444444",
        Some(&minted.token),
    );

    assert_eq!(status, 200);
    assert_eq!(
        body["capability_id"],
        "44444444-4444-4444-8444-444444444444"
    );
    assert_eq!(body["owner_service"], "fa-local");
}

#[test]
fn a_well_formed_capability_id_absent_from_the_registry_returns_404() {
    let registry_json = support::load_fixture_json("valid", "capability-registry-basic.json");
    let registry_file = TempRegistryFile::write(&registry_json);
    let minted = mint_serve_test_token("kid-http-404", REQUIRED_SCOPE, 3600);
    let mut public_keys = HashMap::new();
    public_keys.insert(minted.kid.clone(), minted.public_key_pem.clone());

    let port = 18112;
    spawn_test_server(&registry_file, port, public_keys);

    let (status, body) = get(
        port,
        "/api/v1/capabilities/99999999-9999-4999-8999-999999999999",
        Some(&minted.token),
    );

    assert_eq!(status, 404);
    assert_eq!(body["error"], "capability_not_found");
}

#[test]
fn a_malformed_capability_id_returns_400_before_any_registry_lookup_or_auth_check() {
    let registry_json = support::load_fixture_json("valid", "capability-registry-basic.json");
    let registry_file = TempRegistryFile::write(&registry_json);
    // No public keys configured at all -- if this request reached the auth
    // or lookup stage, it would deny for a different reason (401) rather
    // than reporting the malformed id, proving the 400 short-circuits
    // before either.
    let public_keys = HashMap::new();

    let port = 18113;
    spawn_test_server(&registry_file, port, public_keys);

    let (status, body) = get(
        port,
        "/api/v1/capabilities/not-a-uuid-at-all",
        None, // no Authorization header either
    );

    assert_eq!(status, 400);
    assert_eq!(body["error"], "malformed_capability_id");
}

#[test]
fn serve_enabled_env_var_unset_means_the_daemon_never_starts_a_listener() {
    let registry_json = support::load_fixture_json("valid", "capability-registry-basic.json");
    let registry_file = TempRegistryFile::write(&registry_json);
    let port: u16 = 18114;

    let binary = env!("CARGO_BIN_EXE_fa-local-run");
    let output = Command::new(binary)
        .args([
            "serve",
            "--registry-file",
            registry_file.path().to_str().unwrap(),
            "--port",
            &port.to_string(),
        ])
        .env_remove("FA_LOCAL_SERVE_ENABLED")
        .output()
        .expect("fa-local-run spawns");

    assert!(
        !output.status.success(),
        "serve must refuse to start without FA_LOCAL_SERVE_ENABLED, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("FA_LOCAL_SERVE_ENABLED"),
        "refusal must name the missing gate, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Defense in depth: no listener was ever bound on the port it would
    // have used.
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", port)).is_err(),
        "no listener should be bound on {port} when FA_LOCAL_SERVE_ENABLED was never set"
    );
}
