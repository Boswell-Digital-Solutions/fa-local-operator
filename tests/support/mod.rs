#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fa_local::SchemaName;
use serde_json::Value;

/// A registry (or other JSON contract) file under `std::env::temp_dir()`,
/// removed when dropped. Hand-rolled instead of pulling in a `tempfile`
/// crate -- `BDS-FAL-DAEMON-v0.1`'s implementation scoping packet
/// authorizes exactly `tiny_http`, `jsonwebtoken`, and `ed25519-dalek`,
/// nothing broader.
pub struct TempRegistryFile {
    path: PathBuf,
}

impl TempRegistryFile {
    pub fn write(contents: &Value) -> Self {
        let path =
            std::env::temp_dir().join(format!("fa-local-serve-test-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, contents.to_string()).expect("temp registry file writes");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRegistryFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn fixture_dir(kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("contracts")
        .join("fixtures")
        .join(kind)
}

pub fn fixture_path(kind: &str, file_name: &str) -> PathBuf {
    fixture_dir(kind).join(file_name)
}

pub fn discover_fixture_paths(kind: &str) -> Vec<PathBuf> {
    let mut paths = std::fs::read_dir(fixture_dir(kind))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn load_fixture_json(kind: &str, file_name: &str) -> Value {
    let path = fixture_path(kind, file_name);
    let raw = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&raw).unwrap()
}

pub fn schema_for_fixture(path: &Path) -> SchemaName {
    let file_name = path.file_name().unwrap().to_string_lossy();
    SchemaName::all()
        .iter()
        .copied()
        .max_by_key(|schema| {
            if file_name.starts_with(schema.fixture_prefix()) {
                schema.fixture_prefix().len()
            } else {
                0
            }
        })
        .filter(|schema| file_name.starts_with(schema.fixture_prefix()))
        .unwrap_or_else(|| panic!("no schema matches fixture {}", path.display()))
}

pub fn coverage_map() -> BTreeMap<&'static str, usize> {
    SchemaName::all()
        .iter()
        .map(|schema| (schema.fixture_prefix(), 0))
        .collect()
}

/// One minted-for-tests Ed25519 JWS, for `serve` route auth tests
/// (`BDS-FAL-DAEMON-v0.1`). Not a real secret -- the signing key is
/// deterministically derived from `kid` so tests never collide, mirroring
/// the shape Forge_Command's own `token_authority::mint_service_token`
/// produces (`{sub, aud, iss, scope, jti, iat, exp, kid}`, `alg: EdDSA`,
/// header `kid`).
pub struct ServeTestToken {
    pub kid: String,
    pub public_key_pem: String,
    pub token: String,
}

/// Mints one test JWS for `kid`, with the given `scope`, expiring
/// `expires_in_seconds` from now (negative for an already-expired token).
pub fn mint_serve_test_token(kid: &str, scope: &str, expires_in_seconds: i64) -> ServeTestToken {
    use ed25519_dalek::SigningKey;
    use ed25519_dalek::pkcs8::EncodePrivateKey;
    use ed25519_dalek::pkcs8::spki::EncodePublicKey;
    use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
    use jsonwebtoken::{Algorithm, EncodingKey, Header};

    let mut seed = [0u8; 32];
    for (index, byte) in kid.bytes().enumerate().take(32) {
        seed[index] = byte;
    }
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();

    let public_key_pem = verifying_key
        .to_public_key_pem(LineEnding::LF)
        .expect("verifying key exports as SPKI PEM");

    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": "forge_command_local",
        "aud": "fa-local-serve",
        "iss": "forge_command_local",
        "scope": scope,
        "jti": uuid::Uuid::new_v4().to_string(),
        "iat": now,
        "exp": now + expires_in_seconds,
        "kid": kid,
    });

    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = Some(kid.to_owned());

    let pkcs8_pem = signing_key
        .to_pkcs8_pem(LineEnding::LF)
        .expect("signing key exports as PKCS8 PEM");
    let encoding_key =
        EncodingKey::from_ed_pem(pkcs8_pem.as_bytes()).expect("PKCS8 PEM decodes as an Ed key");

    let token = jsonwebtoken::encode(&header, &claims, &encoding_key).expect("token encodes");

    ServeTestToken {
        kid: kid.to_owned(),
        public_key_pem,
        token,
    }
}
