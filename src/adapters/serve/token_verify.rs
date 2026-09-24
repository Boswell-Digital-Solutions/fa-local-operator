//! Pure Ed25519 JWS verification for the `serve` route's `Authorization`
//! header.
//!
//! Authorized by `BDS-FAL-DAEMON-v0.1`'s implementation scoping packet,
//! "Auth verification (Rust side)". Reuses `Forge_Command`'s existing token
//! authority (`src-tauri/src/token_authority.rs`, `POST /fc/token`) under a
//! new `capability:read` scope rather than building new auth machinery or
//! extending FA Local's own local, file-argument-supplied requester-trust
//! model (`DECISIONS/0002-requester-trust-model.md`) to a network boundary
//! it was never scoped for.
//!
//! Mirrors `dataforge-Local`'s own
//! `execution_authority/services/run_token.py` `RunTokenVerifier`: an empty
//! or unset key map denies every request before anything else is even
//! inspected -- "no key configured means fail closed", not an open door.

use std::collections::HashMap;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

/// The only scope this route admits. A token minted for any other scope is
/// denied, same as a token with no scope at all.
pub const REQUIRED_SCOPE: &str = "capability:read";

/// Why one `Authorization` header failed verification.
///
/// Kept internal to logs -- the HTTP response body is always the flat
/// `{"error": "unauthorized"}` `02_IMPLEMENTATION_SCOPING_PACKET.md`'s route
/// contract specifies, mirroring `run_token.py`'s own `TokenVerdict.deny`
/// (a named reason for operators, a flat denial for the wire, "never a more
/// specific reason").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthDenyReason {
    /// The configured public-key map is empty (unset or blank
    /// `FA_LOCAL_SERVE_PUBLIC_KEYS`). Fail closed: no key to verify against
    /// means every request is denied, not an open door.
    NoKeysConfigured,
    /// No `Authorization` header was present at all.
    MissingHeader,
    /// The header was present but not a well-formed `Bearer <JWS>` value,
    /// or the JWS could not be parsed.
    Malformed,
    /// The token's `kid` is missing, or is not one of the configured
    /// public keys.
    UnknownKid,
    /// The token's `kid` is known, but its signature does not verify.
    BadSignature,
    /// The token's `exp` claim is in the past.
    Expired,
    /// The token verified, but its `scope` claim is not exactly
    /// [`REQUIRED_SCOPE`].
    WrongScope,
}

/// The subset of Forge_Command's `TokenClaims`
/// (`src-tauri/src/models/token_authority.rs`) this route inspects. Unknown
/// fields (`sub`, `aud`, `iss`, `jti`, `iat`, `kid`) are ignored by `serde`,
/// not rejected -- this route does not scope-bind on audience or issuer,
/// only on `scope`.
#[derive(Debug, Deserialize)]
struct ServeTokenClaims {
    scope: Option<String>,
}

/// Verifies one `Authorization` header value against the configured
/// `kid -> public key` map.
///
/// Pure: no I/O beyond what the caller already did to obtain `public_keys`
/// (loaded once at startup from `FA_LOCAL_SERVE_PUBLIC_KEYS`, or re-read on
/// process restart -- this route has no key-rotation mechanism of its own,
/// unlike Forge_Command's own hot-swap).
pub fn verify_bearer_token(
    authorization_header: Option<&str>,
    public_keys: &HashMap<String, String>,
) -> Result<(), AuthDenyReason> {
    if public_keys.is_empty() {
        return Err(AuthDenyReason::NoKeysConfigured);
    }

    let token = authorization_header
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or(AuthDenyReason::MissingHeader)?;

    let header = decode_header(token).map_err(|_| AuthDenyReason::Malformed)?;
    let kid = header.kid.ok_or(AuthDenyReason::UnknownKid)?;
    let raw_key = public_keys.get(&kid).ok_or(AuthDenyReason::UnknownKid)?;
    let decoding_key =
        decoding_key_from_configured_value(raw_key).map_err(|_| AuthDenyReason::UnknownKid)?;

    // Only EdDSA is ever accepted; only `exp` is required (default). No
    // audience/issuer binding -- this packet's route contract checks
    // exactly `scope`, `exp`, and `kid`, nothing else. Forge_Command's
    // real tokens always carry an `aud` claim (its own token authority
    // requires one), so `validate_aud` must be turned off explicitly --
    // `jsonwebtoken`'s default validates `aud` whenever the claim is
    // present even with no configured audience, which would otherwise
    // deny every real token with `InvalidAudience`.
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.validate_aud = false;

    let token_data = decode::<ServeTokenClaims>(token, &decoding_key, &validation).map_err(
        |error| match error.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthDenyReason::Expired,
            jsonwebtoken::errors::ErrorKind::InvalidSignature => AuthDenyReason::BadSignature,
            _ => AuthDenyReason::Malformed,
        },
    )?;

    if token_data.claims.scope.as_deref() != Some(REQUIRED_SCOPE) {
        return Err(AuthDenyReason::WrongScope);
    }

    Ok(())
}

/// Builds a `jsonwebtoken` decoding key from one configured public-key
/// string.
///
/// Accepts PEM SPKI (what Forge_Command's own
/// `token_authority::get_public_key_pem` actually publishes) or a raw
/// base64url-no-pad Ed25519 public key (the JWK `x`-parameter convention
/// `jsonwebtoken::DecodingKey::from_ed_components` already decodes,
/// entirely inside the two authorized dependencies).
///
/// Deliberately narrower than `dataforge-Local`'s Python verifier, which
/// also accepts standard base64 and padded url-safe base64: this packet's
/// dependency table authorizes exactly `tiny_http`, `jsonwebtoken`, and
/// `ed25519-dalek`, none of which expose a general base64 decoder, so a raw
/// key here must already be in the one base64 flavor `jsonwebtoken` itself
/// understands.
fn decoding_key_from_configured_value(value: &str) -> Result<DecodingKey, ()> {
    let trimmed = value.trim();
    if trimmed.contains("-----BEGIN") {
        DecodingKey::from_ed_pem(trimmed.as_bytes()).map_err(|_| ())
    } else {
        DecodingKey::from_ed_components(trimmed).map_err(|_| ())
    }
}
