//! `app::serve_service::ServeService` cases from
//! `docs/plans/active/BDS_FAL_DAEMON_v0.1/02_IMPLEMENTATION_SCOPING_PACKET.md`'s
//! "Test allowlist and negative cases": the registry side of case 1 (found)
//! and case 2 (not found), case 10 (reload adds a capability), and case 11
//! (a corrupted reload leaves the previously good registry serving).
//!
//! `ServeService::reload` is exactly what `adapters::serve::http_server`'s
//! `SIGHUP` handler calls -- these tests exercise that call directly rather
//! than sending a real OS signal, since the reload *logic* (not signal
//! delivery) is what these cases are about.

mod support;

use fa_local::app::serve_service::{ServeLookupResult, ServeService};
use fa_local::domain::shared::CapabilityId;
use support::TempRegistryFile;

fn write_temp_registry(contents: &serde_json::Value) -> TempRegistryFile {
    TempRegistryFile::write(contents)
}

fn basic_registry_json() -> serde_json::Value {
    support::load_fixture_json("valid", "capability-registry-basic.json")
}

#[test]
fn lookup_returns_the_capability_record_present_in_the_loaded_registry() {
    let registry_json = basic_registry_json();
    let file = write_temp_registry(&registry_json);
    let service = ServeService::load(file.path()).expect("valid registry loads");

    let capability_id =
        CapabilityId::from_uuid("44444444-4444-4444-8444-444444444444".parse().unwrap());

    match service.lookup(capability_id) {
        ServeLookupResult::Found(record) => {
            assert_eq!(record.capability_id, capability_id);
            assert_eq!(record.owner_service, "fa-local");
        }
        other => panic!("expected Found, got {other:?}"),
    }
}

#[test]
fn lookup_returns_not_found_for_a_well_formed_id_absent_from_the_registry() {
    let registry_json = basic_registry_json();
    let file = write_temp_registry(&registry_json);
    let service = ServeService::load(file.path()).expect("valid registry loads");

    let absent_id =
        CapabilityId::from_uuid("99999999-9999-4999-8999-999999999999".parse().unwrap());

    match service.lookup(absent_id) {
        ServeLookupResult::NotFound => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn loading_an_invalid_registry_file_fails_loudly_at_startup() {
    let file = write_temp_registry(&serde_json::json!({ "not": "a valid capability registry" }));

    let result = ServeService::load(file.path());

    assert!(result.is_err());
}

#[test]
fn reload_after_editing_the_registry_file_makes_a_new_capability_immediately_lookupable() {
    let mut registry_json = basic_registry_json();
    let file = write_temp_registry(&registry_json);
    let service = ServeService::load(file.path()).expect("valid registry loads");

    let new_id = "55555555-5555-4555-8555-555555555555";
    let mut new_capability = registry_json["capabilities"][0].clone();
    new_capability["capability_id"] = serde_json::json!(new_id);
    registry_json["capabilities"]
        .as_array_mut()
        .unwrap()
        .push(new_capability);

    std::fs::write(file.path(), registry_json.to_string()).expect("registry file rewrites");

    service.reload().expect("reload succeeds on a valid file");

    let capability_id = CapabilityId::from_uuid(new_id.parse().unwrap());
    match service.lookup(capability_id) {
        ServeLookupResult::Found(record) => assert_eq!(record.capability_id, capability_id),
        other => panic!("expected Found after reload, got {other:?}"),
    }
}

#[test]
fn a_reload_over_a_corrupted_file_leaves_the_previously_good_registry_serving() {
    let registry_json = basic_registry_json();
    let file = write_temp_registry(&registry_json);
    let service = ServeService::load(file.path()).expect("valid registry loads");

    let original_id =
        CapabilityId::from_uuid("44444444-4444-4444-8444-444444444444".parse().unwrap());
    assert!(matches!(
        service.lookup(original_id),
        ServeLookupResult::Found(_)
    ));

    // Corrupt the file (not even valid JSON) and reload -- the reload must
    // report the failure, not silently succeed, and must not drop the
    // previously good registry.
    std::fs::write(file.path(), b"{ not json at all").expect("corrupt write");

    let reload_result = service.reload();
    assert!(reload_result.is_err());

    match service.lookup(original_id) {
        ServeLookupResult::Found(record) => assert_eq!(record.capability_id, original_id),
        other => panic!(
            "expected the previously good registry to keep serving after a failed reload, got {other:?}"
        ),
    }
}
