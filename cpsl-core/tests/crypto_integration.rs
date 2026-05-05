#![cfg(feature = "mod-crypto")]

use cpsl_core::{Sandbox, transpile};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── 1. Hashing tests ─────────────────────────────────────────────

#[test]
fn sha256_known_vector() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.sha256("hello")"#)
        .unwrap();
    assert_eq!(
        r,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}

#[test]
fn sha512_known_vector() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.sha512("hello")"#)
        .unwrap();
    assert_eq!(
        r,
        "9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043"
    );
}

#[test]
fn md5_known_vector() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.md5("hello")"#)
        .unwrap();
    assert_eq!(r, "5d41402abc4b2a76b9719d911017c592");
}

#[test]
fn sha256_empty_string() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.sha256("")"#)
        .unwrap();
    // SHA-256 of empty string is well-known
    assert_eq!(
        r,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn sha256_returns_64_hex_chars() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.sha256("test"))"#)
        .unwrap();
    assert_eq!(r, "64");
}

#[test]
fn sha512_returns_128_hex_chars() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.sha512("test"))"#)
        .unwrap();
    assert_eq!(r, "128");
}

#[test]
fn md5_returns_32_hex_chars() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.md5("test"))"#)
        .unwrap();
    assert_eq!(r, "32");
}

// ── 2. HMAC tests ────────────────────────────────────────────────

#[test]
fn hmac_sha256_known_vector() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.hmac_sha256("secret", "hello")"#)
        .unwrap();
    assert_eq!(
        r,
        "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b"
    );
}

#[test]
fn hmac_sha512_known_key_data() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.hmac_sha512("secret", "hello")"#)
        .unwrap();
    // hmac_sha512 should return 128 hex chars
    assert_eq!(r.len(), 128, "hmac_sha512 length: {}", r.len());
}

#[test]
fn hmac_sha256_returns_64_hex_chars() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.hmac_sha256("key", "data"))"#)
        .unwrap();
    assert_eq!(r, "64");
}

// ── 3. Encryption tests ─────────────────────────────────────────

#[test]
fn encrypt_returns_table_with_ciphertext_and_nonce() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = crypto.encrypt("hello", "mykey")
            return tostring(result.ciphertext ~= nil) .. " " .. tostring(result.nonce ~= nil)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true true");
}

#[test]
fn encrypt_decrypt_roundtrip() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local enc = crypto.encrypt("hello world", "mysecretkey")
            local dec = crypto.decrypt(enc.ciphertext, "mysecretkey", enc.nonce)
            return dec
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello world");
}

#[test]
fn decrypt_with_wrong_key_fails() {
    let s = sb();
    let err = s
        .exec(
            r#"
            local enc = crypto.encrypt("hello", "correct_key")
            return crypto.decrypt(enc.ciphertext, "wrong_key", enc.nonce)
        "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("decrypt") || err.message.contains("failed"),
        "msg: {}",
        err.message
    );
}

#[test]
fn decrypt_with_wrong_nonce_fails() {
    let s = sb();
    let err = s
        .exec(
            r#"
            local enc = crypto.encrypt("hello", "mykey")
            -- Use a different valid 12-byte nonce (24 hex chars)
            return crypto.decrypt(enc.ciphertext, "mykey", "aabbccddeeff00112233445566")
        "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("decrypt") || err.message.contains("failed"),
        "msg: {}",
        err.message
    );
}

#[test]
fn encrypt_produces_different_nonces_each_time() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local e1 = crypto.encrypt("hello", "key")
            local e2 = crypto.encrypt("hello", "key")
            return tostring(e1.nonce ~= e2.nonce)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn encrypt_decrypt_empty_plaintext() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local enc = crypto.encrypt("", "mykey")
            local dec = crypto.decrypt(enc.ciphertext, "mykey", enc.nonce)
            return dec
        "#,
        )
        .unwrap();
    assert_eq!(r, "");
}

#[test]
fn encrypt_decrypt_long_plaintext() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local long = string.rep("abcdefghij", 100)
            local enc = crypto.encrypt(long, "mykey")
            local dec = crypto.decrypt(enc.ciphertext, "mykey", enc.nonce)
            return tostring(dec == long)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

// ── 4. JWT tests ─────────────────────────────────────────────────

#[test]
fn jwt_encode_produces_three_part_token() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local token = crypto.jwt_encode({sub="1234567890", name="Test"}, "secret")
            local parts = 0
            for _ in string.gmatch(token, "[^%.]+") do
                parts = parts + 1
            end
            return tostring(parts)
        "#,
        )
        .unwrap();
    assert_eq!(r, "3");
}

#[test]
fn jwt_encode_decode_roundtrip() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local payload = {sub="user123", name="Alice", admin=true}
            local token = crypto.jwt_encode(payload, "mysecret", {expiresIn=3600})
            local decoded = crypto.jwt_decode(token, "mysecret")
            return decoded.sub .. " " .. decoded.name .. " " .. tostring(decoded.admin)
        "#,
        )
        .unwrap();
    assert_eq!(r, "user123 Alice true");
}

#[test]
fn jwt_decode_wrong_secret_fails() {
    let s = sb();
    let err = s
        .exec(
            r#"
            local token = crypto.jwt_encode({sub="test"}, "correct_secret", {expiresIn=3600})
            return crypto.jwt_decode(token, "wrong_secret")
        "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("jwt_decode") || err.message.contains("InvalidSignature"),
        "msg: {}",
        err.message
    );
}

#[test]
fn jwt_encode_with_custom_algorithm() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local token = crypto.jwt_encode({sub="test"}, "secret", {algorithm="HS384", expiresIn=3600})
            local decoded = crypto.jwt_decode(token, "secret", {algorithm="HS384"})
            return decoded.sub
        "#,
        )
        .unwrap();
    assert_eq!(r, "test");
}

#[test]
fn jwt_encode_with_expires_in_adds_exp() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local token = crypto.jwt_encode({sub="test"}, "secret", {expiresIn=3600})
            local decoded = crypto.jwt_decode(token, "secret")
            return tostring(decoded.exp ~= nil)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn jwt_decode_validate_false_skips_signature() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local token = crypto.jwt_encode({sub="test"}, "real_secret", {expiresIn=3600})
            local decoded = crypto.jwt_decode(token, "any_secret", {validate=false})
            return decoded.sub
        "#,
        )
        .unwrap();
    assert_eq!(r, "test");
}

// ── 5. UUID tests ────────────────────────────────────────────────

#[test]
fn uuid_v4_format() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local id = crypto.uuid()
            -- v4 UUID format: 8-4-4-4-12 hex with dashes, version nibble = 4
            local ok = string.match(id, "^%x%x%x%x%x%x%x%x%-%x%x%x%x%-4%x%x%x%-%x%x%x%x%-%x%x%x%x%x%x%x%x%x%x%x%x$") ~= nil
            return tostring(ok)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn uuid_v7_format() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local id = crypto.uuid_v7()
            -- v7 UUID format: 8-4-4-4-12 hex with dashes, version nibble = 7
            local ok = string.match(id, "^%x%x%x%x%x%x%x%x%-%x%x%x%x%-7%x%x%x%-%x%x%x%x%-%x%x%x%x%x%x%x%x%x%x%x%x$") ~= nil
            return tostring(ok)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn uuid_generates_unique_values() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local a = crypto.uuid()
            local b = crypto.uuid()
            return tostring(a ~= b)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn uuid_v7_is_time_sortable() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local a = crypto.uuid_v7()
            local b = crypto.uuid_v7()
            return tostring(b >= a)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

// ── 6. Hex encode/decode tests ───────────────────────────────────

#[test]
fn hex_encode_hello() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.hex_encode("hello")"#)
        .unwrap();
    assert_eq!(r, "68656c6c6f");
}

#[test]
fn hex_decode_hello() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.hex_decode("68656c6c6f")"#)
        .unwrap();
    assert_eq!(r, "hello");
}

#[test]
fn hex_encode_decode_roundtrip() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local encoded = crypto.hex_encode("test data 123!")
            local decoded = crypto.hex_decode(encoded)
            return decoded
        "#,
        )
        .unwrap();
    assert_eq!(r, "test data 123!");
}

#[test]
fn hex_decode_invalid_hex_errors() {
    let s = sb();
    let err = s
        .exec(r#"return crypto.hex_decode("xyz")"#)
        .unwrap_err();
    assert!(
        err.message.contains("hex_decode") || err.message.contains("invalid"),
        "msg: {}",
        err.message
    );
}

// ── 7. Random bytes tests ────────────────────────────────────────

#[test]
fn random_bytes_correct_length() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.random_bytes(16))"#)
        .unwrap();
    // 16 bytes = 32 hex chars
    assert_eq!(r, "32");
}

#[test]
fn random_bytes_zero_returns_empty() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.random_bytes(0)"#)
        .unwrap();
    assert_eq!(r, "");
}

#[test]
fn random_bytes_generates_different_values() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local a = crypto.random_bytes(16)
            local b = crypto.random_bytes(16)
            return tostring(a ~= b)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn random_bytes_large_n() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.random_bytes(256))"#)
        .unwrap();
    // 256 bytes = 512 hex chars
    assert_eq!(r, "512");
}

// ── 8. Dual-signature tests (table form for shell dispatch) ──────

#[test]
fn sha256_table_form_positional() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.sha256({[1]="hello"})"#)
        .unwrap();
    assert_eq!(
        r,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}

#[test]
fn sha256_table_form_named() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.sha256({data="hello"})"#)
        .unwrap();
    assert_eq!(
        r,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}

#[test]
fn hmac_sha256_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.hmac_sha256({key="secret", data="hello"})"#)
        .unwrap();
    assert_eq!(
        r,
        "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b"
    );
}

#[test]
fn encrypt_table_form() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local enc = crypto.encrypt({plaintext="hello", key="mykey"})
            local dec = crypto.decrypt(enc.ciphertext, "mykey", enc.nonce)
            return dec
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello");
}

// ── 9. Shell dispatch tests ──────────────────────────────────────

#[test]
fn shell_crypto_sha256() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"crypto sha256 "hello""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"),
        "expected sha256 hash, got: {}",
        r
    );
}

#[test]
fn shell_crypto_uuid() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh("crypto uuid");
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau);
    assert!(
        r.is_ok(),
        "shell crypto uuid should not error: {:?}",
        r.err()
    );
}

#[test]
fn shell_crypto_hex_encode() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"crypto hex_encode "hello""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("68656c6c6f"),
        "expected hex of hello, got: {}",
        r
    );
}

// ── 10. Error handling tests ─────────────────────────────────────

#[test]
fn sha256_no_args_errors() {
    let s = sb();
    let err = s.exec("crypto.sha256()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn sha256_wrong_type_errors() {
    let s = sb();
    let err = s.exec("crypto.sha256(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn hmac_sha256_missing_second_arg_errors() {
    let s = sb();
    let err = s
        .exec(r#"crypto.hmac_sha256("key")"#)
        .unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn encrypt_no_args_errors() {
    let s = sb();
    let err = s.exec("crypto.encrypt()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn decrypt_wrong_number_of_args_errors() {
    let s = sb();
    let err = s
        .exec(r#"crypto.decrypt("aabb")"#)
        .unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn random_bytes_non_number_errors() {
    let s = sb();
    let err = s
        .exec(r#"crypto.random_bytes("abc")"#)
        .unwrap_err();
    assert!(
        err.message.contains("number") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

// ── 11. Help tests ───────────────────────────────────────────────

#[test]
fn crypto_help_returns_help_text() {
    let s = sb();
    let r = s.exec("return crypto.help()").unwrap();
    assert!(r.contains("crypto"), "help: {}", r);
    assert!(r.contains("crypto.sha256"), "help: {}", r);
    assert!(r.contains("crypto.encrypt"), "help: {}", r);
    assert!(r.contains("crypto.jwt_encode"), "help: {}", r);
    assert!(r.contains("crypto.uuid"), "help: {}", r);
}

#[test]
fn crypto_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("crypto.foo()").unwrap_err();
    assert!(
        err.message.contains("crypto.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call crypto.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_crypto() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(
        r.contains("crypto"),
        "global help should list crypto: {}",
        r
    );
}

// ── 12. Sandbox safety tests ─────────────────────────────────────

#[test]
fn crypto_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(crypto.sha256)) .. " " ..
                   tostring(type(crypto.encrypt)) .. " " ..
                   tostring(type(crypto.uuid)) .. " " ..
                   tostring(rawget(crypto, "io")) .. " " ..
                   tostring(rawget(crypto, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function nil nil");
}

#[test]
fn crypto_metatable_does_not_leak_globals() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local mt = getmetatable(crypto)
            if mt then
                local idx = rawget(mt, "__index")
                if type(idx) == "table" then
                    if rawget(idx, "io") or rawget(idx, "os") then
                        return "metatable leaks dangerous globals"
                    end
                end
            end
            local count = 0
            for k, v in pairs(crypto) do
                count = count + 1
            end
            return "safe:" .. count
        "#,
        )
        .unwrap();
    assert!(
        r.starts_with("safe:"),
        "expected safe table, got: {}",
        r
    );
}

#[test]
fn crypto_is_purely_computational() {
    let s = sb();
    let r = s
        .exec(
            r#"
            -- All crypto operations should work without any fs/network
            local results = {}
            table.insert(results, crypto.sha256("test"))
            table.insert(results, crypto.hex_encode("hi"))
            table.insert(results, crypto.hex_decode("6869"))
            local enc = crypto.encrypt("data", "key")
            table.insert(results, crypto.decrypt(enc.ciphertext, "key", enc.nonce))
            return table.concat(results, ",")
        "#,
        )
        .unwrap();
    assert!(
        r.contains("hi") && r.contains("data"),
        "expected computational results, got: {}",
        r
    );
}

// ── 13. Python transpiler e2e tests ──────────────────────────────

#[test]
fn python_hashlib_sha256() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import hashlib
result = hashlib.sha256("hello")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(
        r,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}

#[test]
fn python_import_hmac_passthrough() {
    let py_code = r#"
import hmac
result = hmac.hmac_sha256("key", "data")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    // hmac maps to crypto global
    assert!(
        transpiled.luau_source.contains("crypto"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_import_jwt_passthrough() {
    let py_code = r#"
import jwt
token = jwt.jwt_encode({"sub": "test"}, "secret")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("crypto"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_import_uuid_passthrough() {
    let py_code = r#"
import uuid
result = uuid.uuid()
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("crypto"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_hashlib_import_sha256() {
    let py_code = r#"
from hashlib import sha256
result = sha256("hello")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    // from hashlib import sha256 → require("hashlib").sha256
    // hashlib is mapped to crypto at require-resolution time
    assert!(
        transpiled.luau_source.contains("hashlib"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

// ── Additional edge case tests ───────────────────────────────────

#[test]
fn sha256_binary_like_data() {
    let s = sb();
    // Hash of a string with special chars
    let r = s
        .exec(r#"return tostring(#crypto.sha256("\0\1\2\3"))"#)
        .unwrap();
    assert_eq!(r, "64");
}

#[test]
fn hmac_sha256_empty_key() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.hmac_sha256("", "data"))"#)
        .unwrap();
    assert_eq!(r, "64");
}

#[test]
fn encrypt_decrypt_with_special_chars() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local text = "Hello! @#$%^&*() 123"
            local enc = crypto.encrypt(text, "key")
            local dec = crypto.decrypt(enc.ciphertext, "key", enc.nonce)
            return dec
        "#,
        )
        .unwrap();
    assert_eq!(r, "Hello! @#$%^&*() 123");
}

#[test]
fn jwt_encode_decode_with_numeric_claims() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local payload = {count=42, ratio=3.14}
            local token = crypto.jwt_encode(payload, "secret", {expiresIn=3600})
            local decoded = crypto.jwt_decode(token, "secret")
            return tostring(decoded.count) .. " " .. tostring(decoded.ratio)
        "#,
        )
        .unwrap();
    assert_eq!(r, "42 3.14");
}

#[test]
fn md5_no_args_errors() {
    let s = sb();
    let err = s.exec("crypto.md5()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn hex_encode_empty_string() {
    let s = sb();
    let r = s
        .exec(r#"return crypto.hex_encode("")"#)
        .unwrap();
    assert_eq!(r, "");
}

#[test]
fn uuid_v4_length() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.uuid())"#)
        .unwrap();
    // UUID v4 string is 36 chars: 8-4-4-4-12
    assert_eq!(r, "36");
}

#[test]
fn uuid_v7_length() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.uuid_v7())"#)
        .unwrap();
    assert_eq!(r, "36");
}

#[test]
fn random_bytes_one_byte() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(#crypto.random_bytes(1))"#)
        .unwrap();
    // 1 byte = 2 hex chars
    assert_eq!(r, "2");
}

#[test]
fn jwt_encode_unsupported_algorithm_errors() {
    let s = sb();
    let err = s
        .exec(
            r#"
            crypto.jwt_encode({sub="test"}, "secret", {algorithm="RS256"})
        "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("unsupported algorithm"),
        "msg: {}",
        err.message
    );
}

#[test]
fn decrypt_invalid_ciphertext_hex_errors() {
    let s = sb();
    let err = s
        .exec(
            r#"
            crypto.decrypt("not_valid_hex!", "key", "aabbccddeeff001122334455")
        "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("hex") || err.message.contains("invalid"),
        "msg: {}",
        err.message
    );
}

#[test]
fn decrypt_invalid_nonce_length_errors() {
    let s = sb();
    let err = s
        .exec(
            r#"
            crypto.decrypt("aabb", "key", "aabb")
        "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("nonce") || err.message.contains("12 bytes"),
        "msg: {}",
        err.message
    );
}
