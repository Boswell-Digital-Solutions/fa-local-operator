//! `adapters::serve::token_verify` cases from
//! `docs/plans/active/BDS_FAL_DAEMON_v0.1/02_IMPLEMENTATION_SCOPING_PACKET.md`'s
//! "Test allowlist and negative cases": 4 (missing header), 5 (unknown
//! kid), 6 (wrong scope), 7 (expired), 8 (empty key map fail-closed), plus a
//! positive control proving a well-formed token with the right scope
//! actually verifies.

mod support;

use std::collections::HashMap;

use fa_local::adapters::serve::{AuthDenyReason, REQUIRED_SCOPE, verify_bearer_token};
use support::mint_serve_test_token;

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

#[test]
fn a_well_formed_token_with_the_capability_read_scope_verifies() {
    let minted = mint_serve_test_token("kid-positive", REQUIRED_SCOPE, 3600);
    let mut public_keys = HashMap::new();
    public_keys.insert(minted.kid.clone(), minted.public_key_pem.clone());

    let result = verify_bearer_token(Some(&bearer(&minted.token)), &public_keys);

    assert_eq!(result, Ok(()));
}

#[test]
fn missing_authorization_header_is_denied() {
    let minted = mint_serve_test_token("kid-missing-header", REQUIRED_SCOPE, 3600);
    let mut public_keys = HashMap::new();
    public_keys.insert(minted.kid.clone(), minted.public_key_pem.clone());

    let result = verify_bearer_token(None, &public_keys);

    assert_eq!(result, Err(AuthDenyReason::MissingHeader));
}

#[test]
fn a_kid_not_present_in_the_configured_public_keys_is_denied() {
    let minted = mint_serve_test_token("kid-unpublished", REQUIRED_SCOPE, 3600);
    // Deliberately keyed under a different kid than `minted.kid` -- an
    // unknown key must be rejected, not silently accepted.
    let mut public_keys = HashMap::new();
    public_keys.insert("some-other-kid".to_owned(), minted.public_key_pem.clone());

    let result = verify_bearer_token(Some(&bearer(&minted.token)), &public_keys);

    assert_eq!(result, Err(AuthDenyReason::UnknownKid));
}

#[test]
fn a_token_with_anything_other_than_exactly_capability_read_scope_is_denied() {
    let minted = mint_serve_test_token("kid-wrong-scope", "capability:write", 3600);
    let mut public_keys = HashMap::new();
    public_keys.insert(minted.kid.clone(), minted.public_key_pem.clone());

    let result = verify_bearer_token(Some(&bearer(&minted.token)), &public_keys);

    assert_eq!(result, Err(AuthDenyReason::WrongScope));
}

#[test]
fn a_token_past_its_exp_is_denied() {
    let minted = mint_serve_test_token("kid-expired", REQUIRED_SCOPE, -7200);
    let mut public_keys = HashMap::new();
    public_keys.insert(minted.kid.clone(), minted.public_key_pem.clone());

    let result = verify_bearer_token(Some(&bearer(&minted.token)), &public_keys);

    assert_eq!(result, Err(AuthDenyReason::Expired));
}

#[test]
fn an_empty_public_key_map_denies_every_request_even_a_well_formed_token() {
    let minted = mint_serve_test_token("kid-fail-closed", REQUIRED_SCOPE, 3600);
    let public_keys: HashMap<String, String> = HashMap::new();

    let result = verify_bearer_token(Some(&bearer(&minted.token)), &public_keys);

    assert_eq!(result, Err(AuthDenyReason::NoKeysConfigured));
}

#[test]
fn a_token_signed_by_a_different_key_than_the_kid_publishes_is_denied() {
    let signer = mint_serve_test_token("kid-shared", REQUIRED_SCOPE, 3600);
    let other = mint_serve_test_token("kid-other", REQUIRED_SCOPE, 3600);
    let mut public_keys = HashMap::new();
    // Publish the *other* key under the signer's kid -- the signature must
    // not verify against a mismatched key.
    public_keys.insert(signer.kid.clone(), other.public_key_pem.clone());

    let result = verify_bearer_token(Some(&bearer(&signer.token)), &public_keys);

    assert_eq!(result, Err(AuthDenyReason::BadSignature));
}
