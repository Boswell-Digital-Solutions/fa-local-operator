use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use serde_json::json;

use fa_local::integrations::neuronforge_local::{
    ADMITTED_TASK_ID, HttpNeuronForgeLocalAdapter, HttpNeuronForgeLocalAdapterConfig,
    ModelResourceDisclosure, NeuronForgeTaskDeliveryAdapter, NeuronForgeTaskDispatchRequest,
    NeuronForgeTaskDispatchResult,
};

fn disclosure() -> ModelResourceDisclosure {
    ModelResourceDisclosure {
        route_class: "WORKHORSE_LOCAL".to_owned(),
        model_id: "qwen2.5:14b".to_owned(),
        resource_budget_class: "workhorse_local".to_owned(),
        execution_mode: "local_model".to_owned(),
    }
}

fn request(task_id: &str) -> NeuronForgeTaskDispatchRequest {
    NeuronForgeTaskDispatchRequest {
        dispatch_id: "dispatch-001".to_owned(),
        request_id: "request-001".to_owned(),
        task_id: task_id.to_owned(),
        scene_text: "Rawn crossed the ford without speaking.".to_owned(),
        model_resource_disclosure: disclosure(),
        operator_visible_message: "test dispatch".to_owned(),
    }
}

/// Reads one HTTP request off `stream` (headers + declared Content-Length
/// body, if any) and writes back `body` as a `200 OK` JSON response with
/// `Connection: close` so the client never blocks waiting for more.
fn serve_one(stream: &mut TcpStream, status_line: &str, body: &str) {
    let mut buf = [0u8; 8192];
    let mut received = Vec::new();
    loop {
        let n = stream.read(&mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        received.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&received);
        if let Some(header_end) = text.find("\r\n\r\n") {
            let headers = &text[..header_end];
            let content_length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("Content-Length: "))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            let body_so_far = received.len() - (header_end + 4);
            if body_so_far >= content_length {
                break;
            }
        }
    }

    let response = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Spawns a one-shot mock HTTP server on an OS-assigned local port, returns
/// its base URL. The server answers exactly one request then exits.
fn spawn_mock_server(status_line: &'static str, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("mock server local addr");
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            serve_one(&mut stream, status_line, body);
        }
    });
    format!("http://{addr}")
}

#[test]
fn an_unadmitted_task_is_refused_before_any_network_call() {
    let adapter = HttpNeuronForgeLocalAdapter::new(HttpNeuronForgeLocalAdapterConfig::new(Some(
        // A port nothing listens on -- proves no connection was ever attempted.
        "http://127.0.0.1:1".to_owned(),
    )));

    let result = adapter.dispatch_task(&request("analyze.something.else.v1"));

    match result {
        NeuronForgeTaskDispatchResult::DispatchUnavailable { summary } => {
            assert!(summary.contains("not admitted"));
            assert!(summary.contains(ADMITTED_TASK_ID));
        }
        other => panic!("expected DispatchUnavailable, got {other:?}"),
    }
}

#[test]
fn a_valid_receipt_maps_to_completed() {
    let body = json!({
        "receipt_version": "NeuronForgeFaLocalTaskDispatchReceipt.v1",
        "schema_validation_status": "valid",
        "semantic_result_posture": "non_canonical_candidate",
        "output_payload": {"summary": "ok"},
    })
    .to_string();
    let url = spawn_mock_server("200 OK", Box::leak(body.into_boxed_str()));

    let adapter =
        HttpNeuronForgeLocalAdapter::new(HttpNeuronForgeLocalAdapterConfig::new(Some(url)));
    let result = adapter.dispatch_task(&request(ADMITTED_TASK_ID));

    match result {
        NeuronForgeTaskDispatchResult::Completed { receipt } => {
            assert_eq!(receipt["schema_validation_status"], "valid");
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn a_failed_receipt_maps_to_not_completed_not_a_dispatch_failure() {
    let body = json!({
        "schema_validation_status": "failed",
        "output_payload": null,
    })
    .to_string();
    let url = spawn_mock_server("200 OK", Box::leak(body.into_boxed_str()));

    let adapter =
        HttpNeuronForgeLocalAdapter::new(HttpNeuronForgeLocalAdapterConfig::new(Some(url)));
    let result = adapter.dispatch_task(&request(ADMITTED_TASK_ID));

    match result {
        NeuronForgeTaskDispatchResult::NotCompleted { receipt } => {
            assert_eq!(receipt["schema_validation_status"], "failed");
        }
        other => panic!("expected NotCompleted, got {other:?}"),
    }
}

#[test]
fn a_degraded_receipt_maps_to_not_completed() {
    let body = json!({ "schema_validation_status": "degraded" }).to_string();
    let url = spawn_mock_server("200 OK", Box::leak(body.into_boxed_str()));

    let adapter =
        HttpNeuronForgeLocalAdapter::new(HttpNeuronForgeLocalAdapterConfig::new(Some(url)));
    let result = adapter.dispatch_task(&request(ADMITTED_TASK_ID));

    assert!(matches!(
        result,
        NeuronForgeTaskDispatchResult::NotCompleted { .. }
    ));
}

#[test]
fn a_non_2xx_response_is_dispatch_unavailable() {
    let url = spawn_mock_server("500 Internal Server Error", "{\"error\": \"boom\"}");

    let adapter =
        HttpNeuronForgeLocalAdapter::new(HttpNeuronForgeLocalAdapterConfig::new(Some(url)));
    let result = adapter.dispatch_task(&request(ADMITTED_TASK_ID));

    match result {
        NeuronForgeTaskDispatchResult::DispatchUnavailable { summary } => {
            assert!(summary.contains("500"));
        }
        other => panic!("expected DispatchUnavailable, got {other:?}"),
    }
}

#[test]
fn an_unparseable_response_body_is_dispatch_unavailable() {
    let url = spawn_mock_server("200 OK", "not json at all");

    let adapter =
        HttpNeuronForgeLocalAdapter::new(HttpNeuronForgeLocalAdapterConfig::new(Some(url)));
    let result = adapter.dispatch_task(&request(ADMITTED_TASK_ID));

    assert!(matches!(
        result,
        NeuronForgeTaskDispatchResult::DispatchUnavailable { .. }
    ));
}

#[test]
fn a_missing_schema_validation_status_is_dispatch_unavailable() {
    let body = json!({ "receipt_version": "NeuronForgeFaLocalTaskDispatchReceipt.v1" }).to_string();
    let url = spawn_mock_server("200 OK", Box::leak(body.into_boxed_str()));

    let adapter =
        HttpNeuronForgeLocalAdapter::new(HttpNeuronForgeLocalAdapterConfig::new(Some(url)));
    let result = adapter.dispatch_task(&request(ADMITTED_TASK_ID));

    match result {
        NeuronForgeTaskDispatchResult::DispatchUnavailable { summary } => {
            assert!(summary.contains("schema_validation_status"));
        }
        other => panic!("expected DispatchUnavailable, got {other:?}"),
    }
}

#[test]
fn an_unreachable_server_is_dispatch_unavailable() {
    // Bind then immediately drop the listener, freeing the port but leaving
    // nothing listening on it -- guarantees a connection-refused, not a
    // flaky "might still be open" race.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);

    let adapter = HttpNeuronForgeLocalAdapter::new(HttpNeuronForgeLocalAdapterConfig::new(Some(
        format!("http://{addr}"),
    )));
    let result = adapter.dispatch_task(&request(ADMITTED_TASK_ID));

    match result {
        NeuronForgeTaskDispatchResult::DispatchUnavailable { summary } => {
            assert!(summary.contains("could not reach"));
        }
        other => panic!("expected DispatchUnavailable, got {other:?}"),
    }
}
