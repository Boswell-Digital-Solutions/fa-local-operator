use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use uuid::Uuid;

use fa_local::integrations::cortex::{
    CortexSubprocessGnatShardAdapter, CortexSubprocessGnatShardAdapterConfig,
    GnatShardDeliveryAdapter, GnatShardDispatchRequest, GnatShardDispatchResult,
    GnatSourceFingerprint, GnatWorkerType,
};

fn temp_file(label: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fa-local-gnat-dispatch-test-{label}-{}.{extension}",
        Uuid::new_v4()
    ))
}

/// Writes an executable shell script standing in for `python_binary`: it
/// ignores every argument (so it works regardless of how the adapter
/// invokes it) and just echoes canned stdout, exiting with the given code --
/// enough to prove the adapter's *own* spawn/parse/result-mapping logic
/// without needing a real Python interpreter or the COR checkout. A live,
/// real cross-repo run (real `python3`, real `cortex_runtime.gnats.shard_cli`)
/// is exercised manually, not as part of this automated suite, since CI
/// environments are not guaranteed to have the COR sibling checkout.
fn fake_python_binary(label: &str, stdout: &str, exit_code: i32) -> PathBuf {
    let script_path = temp_file(label, "sh");
    fs::write(
        &script_path,
        format!("#!/bin/sh\ncat <<'EOF'\n{stdout}\nEOF\nexit {exit_code}\n"),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).unwrap();
    script_path
}

fn base_request(local_path: PathBuf) -> GnatShardDispatchRequest {
    GnatShardDispatchRequest {
        run_id: "gnat-run-dispatch-test".to_owned(),
        shard_id: "shard-0001".to_owned(),
        ordinal: 0,
        worker_type: GnatWorkerType::MarkdownSyntax,
        source_ref: "chapter-01".to_owned(),
        source_path_token: "token-abc123".to_owned(),
        media_type: "text/markdown".to_owned(),
        source_fingerprint: GnatSourceFingerprint {
            algorithm: "sha256".to_owned(),
            digest: "a".repeat(64),
            byte_count: 42,
            modified_at: "2030-01-01T00:00:00Z".to_owned(),
        },
        deadline_ms: 30000,
        max_bytes: 20 * 1024 * 1024,
        local_path,
    }
}

#[test]
fn dispatch_refuses_a_worker_type_outside_the_proving_slice_without_spawning() {
    let local_path = temp_file("unused-source", "md");
    fs::write(&local_path, "# hello\n").unwrap();

    let mut request = base_request(local_path.clone());
    request.worker_type = GnatWorkerType::PdfTextSyntax;

    // An intentionally-nonexistent interpreter: if the adapter ever tried
    // to spawn it, this would fail with a spawn error, not the worker-type
    // rejection this test actually wants -- proving the check happens
    // before any process is spawned.
    let adapter =
        CortexSubprocessGnatShardAdapter::new(CortexSubprocessGnatShardAdapterConfig::new(
            PathBuf::from("/nonexistent/python-binary-should-never-be-invoked"),
            PathBuf::from("/nonexistent/cortex-repo-root"),
        ));

    let result = adapter.deliver_shard(&request);
    match result {
        GnatShardDispatchResult::DispatchUnavailable { summary } => {
            assert!(summary.contains("markdown_syntax"));
            assert!(summary.contains("PdfTextSyntax") || summary.contains("pdf"));
        }
        other => panic!("expected DispatchUnavailable, got {other:?}"),
    }

    fs::remove_file(&local_path).ok();
}

#[test]
fn dispatch_reports_unavailable_when_the_source_file_is_missing() {
    let request = base_request(PathBuf::from("/nonexistent/source-file.md"));

    let adapter =
        CortexSubprocessGnatShardAdapter::new(CortexSubprocessGnatShardAdapterConfig::new(
            PathBuf::from("/nonexistent/python-binary-should-never-be-invoked"),
            PathBuf::from("/nonexistent/cortex-repo-root"),
        ));

    let result = adapter.deliver_shard(&request);
    assert!(matches!(
        result,
        GnatShardDispatchResult::DispatchUnavailable { .. }
    ));
}

#[test]
fn dispatch_maps_a_complete_receipt_to_completed() {
    let local_path = temp_file("complete-source", "md");
    fs::write(&local_path, "# hello\n").unwrap();
    let request = base_request(local_path.clone());

    let fake_python = fake_python_binary(
        "complete",
        r#"{"contract_version": "GnatWorkerReceipt.v1", "state": "complete", "shard_id": "shard-0001"}"#,
        0,
    );

    let adapter = CortexSubprocessGnatShardAdapter::new(
        CortexSubprocessGnatShardAdapterConfig::new(fake_python.clone(), std::env::temp_dir()),
    );

    let result = adapter.deliver_shard(&request);
    match result {
        GnatShardDispatchResult::Completed { receipt } => {
            assert_eq!(receipt["state"], "complete");
            assert_eq!(receipt["shard_id"], "shard-0001");
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    fs::remove_file(&local_path).ok();
    fs::remove_file(&fake_python).ok();
}

#[test]
fn dispatch_maps_a_denied_receipt_to_not_completed_even_with_a_nonzero_exit() {
    let local_path = temp_file("denied-source", "txt");
    fs::write(&local_path, "").unwrap();
    let request = base_request(local_path.clone());

    let fake_python = fake_python_binary(
        "denied",
        r#"{"contract_version": "GnatWorkerReceipt.v1", "state": "denied", "shard_id": "shard-0001", "error_reason_code": "source_ineligible"}"#,
        1,
    );

    let adapter = CortexSubprocessGnatShardAdapter::new(
        CortexSubprocessGnatShardAdapterConfig::new(fake_python.clone(), std::env::temp_dir()),
    );

    let result = adapter.deliver_shard(&request);
    match result {
        GnatShardDispatchResult::NotCompleted { receipt } => {
            assert_eq!(receipt["state"], "denied");
        }
        other => panic!("expected NotCompleted, got {other:?}"),
    }

    fs::remove_file(&local_path).ok();
    fs::remove_file(&fake_python).ok();
}

#[test]
fn dispatch_reports_unavailable_for_unparseable_output() {
    let local_path = temp_file("garbage-source", "md");
    fs::write(&local_path, "# hello\n").unwrap();
    let request = base_request(local_path.clone());

    let fake_python = fake_python_binary("garbage", "not json at all", 1);

    let adapter = CortexSubprocessGnatShardAdapter::new(
        CortexSubprocessGnatShardAdapterConfig::new(fake_python.clone(), std::env::temp_dir()),
    );

    let result = adapter.deliver_shard(&request);
    assert!(matches!(
        result,
        GnatShardDispatchResult::DispatchUnavailable { .. }
    ));

    fs::remove_file(&local_path).ok();
    fs::remove_file(&fake_python).ok();
}

#[test]
fn dispatch_reports_unavailable_when_the_interpreter_cannot_be_spawned() {
    let local_path = temp_file("spawn-fail-source", "md");
    fs::write(&local_path, "# hello\n").unwrap();
    let request = base_request(local_path.clone());

    let adapter =
        CortexSubprocessGnatShardAdapter::new(CortexSubprocessGnatShardAdapterConfig::new(
            PathBuf::from("/nonexistent/python-binary"),
            std::env::temp_dir(),
        ));

    let result = adapter.deliver_shard(&request);
    match result {
        GnatShardDispatchResult::DispatchUnavailable { summary } => {
            assert!(summary.contains("could not spawn"));
        }
        other => panic!("expected DispatchUnavailable, got {other:?}"),
    }

    fs::remove_file(&local_path).ok();
}
