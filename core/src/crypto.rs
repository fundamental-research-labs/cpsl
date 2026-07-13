//! Cryptography module for the Luau sandbox.
//!
//! Exposes hashing (SHA-256, SHA-512, MD5), HMAC, AES-256-GCM encryption,
//! JWT encode/decode, UUID generation, and hex/random-bytes utilities
//! as `crypto.*` globals.  Pure computation — no filesystem or network access.

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::Mac;
use jsonwebtoken::{
    dangerous::insecure_decode, decode, encode, Algorithm, DecodingKey, EncodingKey, Header,
    Validation,
};
use mlua::{Lua, MultiValue, Value};
use sha2::{Digest as Sha2Digest, Sha256, Sha512};
use std::collections::HashMap;

// Type alias for HMAC
type HmacSha256 = hmac::Hmac<Sha256>;
type HmacSha512 = hmac::Hmac<Sha512>;

// ---------------------------------------------------------------------------
// Module documentation
// ---------------------------------------------------------------------------

const JWT_ENCODE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "algorithm",
        typ: "string",
        required: false,
        description: "JWT algorithm: HS256 (default), HS384, HS512",
    },
    FieldDoc {
        name: "expiresIn",
        typ: "number",
        required: false,
        description: "Expiration in seconds from now",
    },
];

const JWT_DECODE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "algorithm",
        typ: "string",
        required: false,
        description: "JWT algorithm: HS256 (default), HS384, HS512",
    },
    FieldDoc {
        name: "validate",
        typ: "boolean",
        required: false,
        description: "Validate signature and expiry (default true)",
    },
];

pub(crate) static CRYPTO_DOC: ModuleDoc = ModuleDoc {
    name: "crypto",
    summary: "Cryptography: hashing, HMAC, AES-256-GCM, JWT, UUID, hex",
    functions: &[
        // --- Hashing ---
        FnDoc {
            name: "sha256",
            description: "Compute SHA-256 hash of a string. Returns hex-encoded digest.",
            params: &[Param {
                name: "data",
                short: Some('d'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: Some(r#"local hash = crypto.sha256("hello world")"#),
        },
        FnDoc {
            name: "sha512",
            description: "Compute SHA-512 hash of a string. Returns hex-encoded digest.",
            params: &[Param {
                name: "data",
                short: Some('d'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "md5",
            description: "Compute MD5 hash of a string. Returns hex-encoded digest. (Not cryptographically secure — use SHA-256 for security.)",
            params: &[Param {
                name: "data",
                short: Some('d'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
        // --- HMAC ---
        FnDoc {
            name: "hmac_sha256",
            description: "Compute HMAC-SHA256 of data with the given key. Returns hex-encoded MAC.",
            params: &[
                Param {
                    name: "key",
                    short: Some('k'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "data",
                    short: Some('d'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "hmac_sha512",
            description: "Compute HMAC-SHA512 of data with the given key. Returns hex-encoded MAC.",
            params: &[
                Param {
                    name: "key",
                    short: Some('k'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "data",
                    short: Some('d'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: None,
        },
        // --- AES-256-GCM ---
        FnDoc {
            name: "encrypt",
            description: "Encrypt plaintext with AES-256-GCM. Key is a passphrase (hashed with SHA-256 to derive 32-byte key). Returns {ciphertext=hex, nonce=hex}.",
            params: &[
                Param {
                    name: "plaintext",
                    short: Some('p'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "key",
                    short: Some('k'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "decrypt",
            description: "Decrypt AES-256-GCM ciphertext. Accepts hex-encoded ciphertext and nonce, plus the same passphrase used for encryption.",
            params: &[
                Param {
                    name: "ciphertext",
                    short: Some('c'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "key",
                    short: Some('k'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "nonce",
                    short: Some('n'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"crypto.decrypt({ciphertext=enc.ciphertext, key="secret", nonce=enc.nonce})"#),
        },
        // --- JWT ---
        FnDoc {
            name: "jwt_encode",
            description: "Encode a payload as a JWT string. Default algorithm: HS256.",
            params: &[
                Param {
                    name: "payload",
                    short: Some('p'),
                    typ: ParamType::Table,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "secret",
                    short: Some('s'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: Some('o'),
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(JWT_ENCODE_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"crypto.jwt_encode({payload={sub="user1"}, secret="secret", expiresIn=3600})"#),
        },
        FnDoc {
            name: "jwt_decode",
            description: "Decode and verify a JWT token. Returns the payload table.",
            params: &[
                Param {
                    name: "token",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "secret",
                    short: Some('s'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: Some('o'),
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(JWT_DECODE_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"crypto.jwt_decode({token=tok, secret="secret"})"#),
        },
        // --- UUID ---
        FnDoc {
            name: "uuid",
            description: "Generate a random UUID v4 string.",
            params: &[],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "uuid_v7",
            description: "Generate a time-sortable UUID v7 string.",
            params: &[],
            returns: ReturnType::String,
            example: None,
        },
        // --- Hex / bytes ---
        FnDoc {
            name: "hex_encode",
            description: "Hex-encode a raw string. Returns lowercase hex.",
            params: &[Param {
                name: "data",
                short: Some('d'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "hex_decode",
            description: "Decode a hex string to raw bytes (returned as a Luau string).",
            params: &[Param {
                name: "hex",
                short: Some('h'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "random_bytes",
            description: "Generate n cryptographically-random bytes, returned as a hex-encoded string.",
            params: &[Param {
                name: "n",
                short: Some('n'),
                typ: ParamType::Number,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
    ],
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse an algorithm string like "HS256" into a jsonwebtoken::Algorithm.
fn parse_algorithm(s: &str) -> Result<Algorithm, mlua::Error> {
    match s.to_uppercase().as_str() {
        "HS256" => Ok(Algorithm::HS256),
        "HS384" => Ok(Algorithm::HS384),
        "HS512" => Ok(Algorithm::HS512),
        other => Err(mlua::Error::external(format!(
            "crypto.jwt: unsupported algorithm '{}'. Supported: HS256, HS384, HS512",
            other
        ))),
    }
}

/// Convert a Lua value to serde_json::Value for JWT payloads.
fn lua_value_to_json(val: &Value) -> Result<serde_json::Value, mlua::Error> {
    match val {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Integer(i) => Ok(serde_json::Value::Number(serde_json::Number::from(
            *i as i64,
        ))),
        Value::Number(n) => {
            if let Some(num) = serde_json::Number::from_f64(*n) {
                Ok(serde_json::Value::Number(num))
            } else {
                Err(mlua::Error::external(format!(
                    "crypto.jwt: cannot represent {} as JSON number",
                    n
                )))
            }
        }
        Value::String(s) => Ok(serde_json::Value::String(s.to_str()?.to_string())),
        Value::Table(t) => {
            // Detect array vs object: if integer keys 1..n, treat as array
            let len = t.raw_len();
            if len > 0 {
                // Check if it's a pure array (sequential integer keys)
                let mut is_array = true;
                let mut has_string_keys = false;
                for pair in t.clone().pairs::<Value, Value>() {
                    let (k, _) = pair?;
                    match k {
                        Value::Integer(_) => {}
                        Value::String(_) => {
                            has_string_keys = true;
                            is_array = false;
                        }
                        _ => {
                            is_array = false;
                        }
                    }
                }
                if is_array && !has_string_keys {
                    let mut arr = Vec::new();
                    for i in 1..=len {
                        let v: Value = t.raw_get(i)?;
                        arr.push(lua_value_to_json(&v)?);
                    }
                    return Ok(serde_json::Value::Array(arr));
                }
            }
            // Object
            let mut map = serde_json::Map::new();
            for pair in t.clone().pairs::<mlua::LuaString, Value>() {
                let (k, v) = pair?;
                map.insert(k.to_str()?.to_string(), lua_value_to_json(&v)?);
            }
            Ok(serde_json::Value::Object(map))
        }
        _ => Err(mlua::Error::external(
            "crypto.jwt: unsupported Lua type in payload",
        )),
    }
}

/// Convert a serde_json::Value to a Lua value.
fn json_to_lua_value(lua: &Lua, val: &serde_json::Value) -> Result<Value, mlua::Error> {
    match val {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i as mlua::Integer))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Ok(Value::Nil)
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(lua.create_string(s)?)),
        serde_json::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                table.raw_set(i + 1, json_to_lua_value(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
        serde_json::Value::Object(map) => {
            let table = lua.create_table()?;
            for (k, v) in map {
                table.raw_set(k.as_str(), json_to_lua_value(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
    }
}

/// Derive a 32-byte AES key from a passphrase via SHA-256.
fn derive_key(passphrase: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(passphrase);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register `crypto.*` globals in the Lua VM.
pub fn register_crypto_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let crypto_table = lua.create_table()?;

    // --- crypto.sha256(data) -> hex string ---
    crypto_table.set(
        "sha256",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, CRYPTO_DOC.params("sha256"), "crypto.sha256")?;
            let data = match &validated[0] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            let mut hasher = Sha256::new();
            hasher.update(&data);
            Ok(hex::encode(hasher.finalize()))
        })?,
    )?;

    // --- crypto.sha512(data) -> hex string ---
    crypto_table.set(
        "sha512",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, CRYPTO_DOC.params("sha512"), "crypto.sha512")?;
            let data = match &validated[0] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            let mut hasher = Sha512::new();
            hasher.update(&data);
            Ok(hex::encode(hasher.finalize()))
        })?,
    )?;

    // --- crypto.md5(data) -> hex string ---
    crypto_table.set(
        "md5",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, CRYPTO_DOC.params("md5"), "crypto.md5")?;
            let data = match &validated[0] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            let mut hasher = md5::Md5::new();
            hasher.update(&data);
            Ok(hex::encode(hasher.finalize()))
        })?,
    )?;

    // --- crypto.hmac_sha256(key, data) -> hex string ---
    crypto_table.set(
        "hmac_sha256",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(
                &args,
                CRYPTO_DOC.params("hmac_sha256"),
                "crypto.hmac_sha256",
            )?;
            let key = match &validated[0] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            let data = match &validated[1] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            let mut mac = <HmacSha256 as Mac>::new_from_slice(&key)
                .map_err(|e| mlua::Error::external(format!("crypto.hmac_sha256: {}", e)))?;
            mac.update(&data);
            Ok(hex::encode(mac.finalize().into_bytes()))
        })?,
    )?;

    // --- crypto.hmac_sha512(key, data) -> hex string ---
    crypto_table.set(
        "hmac_sha512",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(
                &args,
                CRYPTO_DOC.params("hmac_sha512"),
                "crypto.hmac_sha512",
            )?;
            let key = match &validated[0] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            let data = match &validated[1] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            let mut mac = <HmacSha512 as Mac>::new_from_slice(&key)
                .map_err(|e| mlua::Error::external(format!("crypto.hmac_sha512: {}", e)))?;
            mac.update(&data);
            Ok(hex::encode(mac.finalize().into_bytes()))
        })?,
    )?;

    // --- crypto.encrypt(plaintext, key) -> {ciphertext=hex, nonce=hex} ---
    crypto_table.set(
        "encrypt",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, CRYPTO_DOC.params("encrypt"), "crypto.encrypt")?;
            let plaintext = match &validated[0] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            let passphrase = match &validated[1] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };

            let key_bytes = derive_key(&passphrase);
            let cipher = Aes256Gcm::new_from_slice(&key_bytes)
                .map_err(|e| mlua::Error::external(format!("crypto.encrypt: {}", e)))?;

            // Generate random 12-byte nonce
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);

            let ciphertext = cipher
                .encrypt(nonce, plaintext.as_ref())
                .map_err(|e| mlua::Error::external(format!("crypto.encrypt: {}", e)))?;

            let result = lua.create_table()?;
            result.set("ciphertext", hex::encode(&ciphertext))?;
            result.set("nonce", hex::encode(nonce_bytes))?;
            Ok(Value::Table(result))
        })?,
    )?;

    // --- crypto.decrypt(ciphertext, key, nonce) -> plaintext string ---
    crypto_table.set(
        "decrypt",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, CRYPTO_DOC.params("decrypt"), "crypto.decrypt")?;
            let ct_hex = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let passphrase = match &validated[1] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            let nonce_hex = match &validated[2] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let ciphertext = hex::decode(&ct_hex).map_err(|e| {
                mlua::Error::external(format!("crypto.decrypt: invalid ciphertext hex: {}", e))
            })?;
            let nonce_bytes = hex::decode(&nonce_hex).map_err(|e| {
                mlua::Error::external(format!("crypto.decrypt: invalid nonce hex: {}", e))
            })?;
            if nonce_bytes.len() != 12 {
                return Err(mlua::Error::external(format!(
                    "crypto.decrypt: nonce must be 12 bytes (24 hex chars), got {} bytes",
                    nonce_bytes.len()
                )));
            }

            let key_bytes = derive_key(&passphrase);
            let cipher = Aes256Gcm::new_from_slice(&key_bytes)
                .map_err(|e| mlua::Error::external(format!("crypto.decrypt: {}", e)))?;
            let nonce = Nonce::from_slice(&nonce_bytes);

            let plaintext = cipher.decrypt(nonce, ciphertext.as_ref()).map_err(|e| {
                mlua::Error::external(format!(
                    "crypto.decrypt: decryption failed (wrong key or corrupted data): {}",
                    e
                ))
            })?;

            let s = lua.create_string(&plaintext)?;
            Ok(Value::String(s))
        })?,
    )?;

    // --- crypto.jwt_encode(payload, secret, opts?) -> token string ---
    crypto_table.set(
        "jwt_encode",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, CRYPTO_DOC.params("jwt_encode"), "crypto.jwt_encode")?;

            let payload_table = match &validated[0] {
                Value::Table(t) => t.clone(),
                _ => unreachable!(),
            };
            let secret = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            // Parse opts
            let mut algorithm = Algorithm::HS256;
            let mut expires_in: Option<u64> = None;
            if validated.len() > 2 {
                if let Value::Table(opts) = &validated[2] {
                    if let Ok(alg_str) = opts.get::<String>("algorithm") {
                        algorithm = parse_algorithm(&alg_str)?;
                    }
                    if let Ok(exp) = opts.get::<f64>("expiresIn") {
                        expires_in = Some(exp as u64);
                    }
                }
            }

            // Convert payload table to JSON
            let mut claims = match lua_value_to_json(&Value::Table(payload_table))? {
                serde_json::Value::Object(m) => m,
                _ => {
                    return Err(mlua::Error::external(
                        "crypto.jwt_encode: payload must be a table (object)",
                    ))
                }
            };

            // Add exp claim if expiresIn specified
            if let Some(secs) = expires_in {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                claims.insert(
                    "exp".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(now + secs)),
                );
            }

            let header = Header::new(algorithm);
            let token = encode(
                &header,
                &claims,
                &EncodingKey::from_secret(secret.as_bytes()),
            )
            .map_err(|e| mlua::Error::external(format!("crypto.jwt_encode: {}", e)))?;

            Ok(token)
        })?,
    )?;

    // --- crypto.jwt_decode(token, secret, opts?) -> payload table ---
    crypto_table.set(
        "jwt_decode",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, CRYPTO_DOC.params("jwt_decode"), "crypto.jwt_decode")?;

            let token = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let secret = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            // Parse opts
            let mut algorithm = Algorithm::HS256;
            let mut do_validate = true;
            if validated.len() > 2 {
                if let Value::Table(opts) = &validated[2] {
                    if let Ok(alg_str) = opts.get::<String>("algorithm") {
                        algorithm = parse_algorithm(&alg_str)?;
                    }
                    if let Ok(v) = opts.get::<bool>("validate") {
                        do_validate = v;
                    }
                }
            }

            let token_data = if do_validate {
                let validation = Validation::new(algorithm);
                decode::<HashMap<String, serde_json::Value>>(
                    &token,
                    &DecodingKey::from_secret(secret.as_bytes()),
                    &validation,
                )
            } else {
                insecure_decode::<HashMap<String, serde_json::Value>>(&token)
            }
            .map_err(|e| mlua::Error::external(format!("crypto.jwt_decode: {}", e)))?;

            // Convert claims to Lua table
            let result = lua.create_table()?;
            for (k, v) in &token_data.claims {
                result.set(k.as_str(), json_to_lua_value(lua, v)?)?;
            }
            Ok(Value::Table(result))
        })?,
    )?;

    // --- crypto.uuid() -> v4 UUID string ---
    crypto_table.set(
        "uuid",
        lua.create_function(|_, args: MultiValue| {
            let _ = validate_args(&args, CRYPTO_DOC.params("uuid"), "crypto.uuid")?;
            Ok(uuid::Uuid::new_v4().to_string())
        })?,
    )?;

    // --- crypto.uuid_v7() -> v7 UUID string ---
    crypto_table.set(
        "uuid_v7",
        lua.create_function(|_, args: MultiValue| {
            let _ = validate_args(&args, CRYPTO_DOC.params("uuid_v7"), "crypto.uuid_v7")?;
            Ok(uuid::Uuid::now_v7().to_string())
        })?,
    )?;

    // --- crypto.hex_encode(data) -> hex string ---
    crypto_table.set(
        "hex_encode",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, CRYPTO_DOC.params("hex_encode"), "crypto.hex_encode")?;
            let data = match &validated[0] {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => unreachable!(),
            };
            Ok(hex::encode(data))
        })?,
    )?;

    // --- crypto.hex_decode(hex) -> raw string ---
    crypto_table.set(
        "hex_decode",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, CRYPTO_DOC.params("hex_decode"), "crypto.hex_decode")?;
            let hex_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let bytes = hex::decode(&hex_str).map_err(|e| {
                mlua::Error::external(format!("crypto.hex_decode: invalid hex: {}", e))
            })?;
            let s = lua.create_string(&bytes)?;
            Ok(Value::String(s))
        })?,
    )?;

    // --- crypto.random_bytes(n) -> hex string ---
    crypto_table.set(
        "random_bytes",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(
                &args,
                CRYPTO_DOC.params("random_bytes"),
                "crypto.random_bytes",
            )?;
            let n = match &validated[0] {
                Value::Integer(i) => *i as usize,
                Value::Number(f) => *f as usize,
                _ => unreachable!(),
            };
            if n == 0 {
                return Ok(String::new());
            }
            if n > 1024 * 1024 {
                return Err(mlua::Error::external(
                    "crypto.random_bytes: n must be <= 1048576 (1 MiB)",
                ));
            }
            let mut bytes = vec![0u8; n];
            OsRng.fill_bytes(&mut bytes);
            Ok(hex::encode(bytes))
        })?,
    )?;

    register_help_functions(lua, &crypto_table, &CRYPTO_DOC)?;
    lua.globals().set("crypto", crypto_table)?;
    wrap_module_with_help_hints(lua, "crypto")?;

    Ok(())
}
