//! Base64 module for the Luau sandbox.
//!
//! Exposes `base64.encode(data)` and `base64.decode(text)` as globals.
//! Also provides `base64.b64encode(data)` and `base64.b64decode(text)` aliases
//! for Python compatibility.
//!
//! Uses a pure-Rust implementation (no external dependencies).

use crate::base64_codec::encode as base64_encode;
use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};

pub(crate) static BASE64_DOC: ModuleDoc = ModuleDoc {
    name: "base64",
    summary: "Base64 encoding & decoding",
    functions: &[
        FnDoc {
            name: "encode",
            description: "Encode a string to base64.",
            params: &[Param {
                name: "data",
                short: Some('d'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::String,
            example: Some(r#"base64.encode("hello world") -- "aGVsbG8gd29ybGQ=""#),
        },
        FnDoc {
            name: "decode",
            description: "Decode a base64 string. Returns the original bytes as a string.",
            params: &[Param {
                name: "text",
                short: Some('t'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::String,
            example: Some(r#"base64.decode("aGVsbG8gd29ybGQ=") -- "hello world""#),
        },
        FnDoc {
            name: "b64encode",
            description: "Encode a string to base64 (Python-compatible alias for encode).",
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
            name: "b64decode",
            description: "Decode a base64 string (Python-compatible alias for decode).",
            params: &[Param {
                name: "text",
                short: Some('t'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
    ],
};

fn base64_decode_byte(c: u8) -> Result<u8, ()> {
    match c {
        b'A'..=b'Z' => Ok(c - b'A'),
        b'a'..=b'z' => Ok(c - b'a' + 26),
        b'0'..=b'9' => Ok(c - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(()),
    }
}

fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    // Strip whitespace (base64 input may have newlines)
    let clean: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if clean.is_empty() {
        return Ok(Vec::new());
    }
    if clean.len() % 4 != 0 {
        return Err("invalid base64: input length is not a multiple of 4".to_string());
    }

    let mut result = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let pad = chunk.iter().filter(|&&b| b == b'=').count();
        if pad > 2 {
            return Err("invalid base64: too much padding".to_string());
        }

        let mut vals = [0u8; 4];
        for (i, &b) in chunk.iter().enumerate() {
            if b == b'=' {
                vals[i] = 0;
            } else {
                vals[i] = base64_decode_byte(b)
                    .map_err(|_| format!("invalid base64: unexpected character '{}'", b as char))?;
            }
        }

        let triple = (vals[0] as u32) << 18
            | (vals[1] as u32) << 12
            | (vals[2] as u32) << 6
            | vals[3] as u32;

        result.push((triple >> 16) as u8);
        if pad < 2 {
            result.push((triple >> 8) as u8);
        }
        if pad < 1 {
            result.push(triple as u8);
        }
    }
    Ok(result)
}

/// Register `base64.*` globals in the Lua VM.
pub(crate) fn register_base64_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let b64 = lua.create_table()?;

    // base64.encode(data) -> string
    let encode_fn = lua.create_function(|_, args: MultiValue| {
        let validated = validate_args(&args, BASE64_DOC.params("encode"), "base64.encode")?;
        let data = match &validated[0] {
            Value::String(s) => s.as_bytes().to_vec(),
            _ => unreachable!("validate_args ensures string"),
        };
        Ok(base64_encode(&data))
    })?;
    b64.set("encode", encode_fn.clone())?;
    b64.set("b64encode", encode_fn)?;

    // base64.decode(text) -> string
    let decode_fn = lua.create_function(|lua, args: MultiValue| {
        let validated = validate_args(&args, BASE64_DOC.params("decode"), "base64.decode")?;
        let text = match &validated[0] {
            Value::String(s) => s.to_string_lossy().to_string(),
            _ => unreachable!("validate_args ensures string"),
        };
        let bytes = base64_decode(&text).map_err(mlua::Error::external)?;
        // Return as Lua string (may contain non-UTF8 bytes)
        lua.create_string(&bytes).map(Value::String)
    })?;
    b64.set("decode", decode_fn.clone())?;
    b64.set("b64decode", decode_fn)?;

    register_help_functions(lua, &b64, &BASE64_DOC)?;

    lua.globals().set("base64", b64)?;
    wrap_module_with_help_hints(lua, "base64")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn encode_hello() {
        assert_eq!(base64_encode(b"Hello, World!"), "SGVsbG8sIFdvcmxkIQ==");
    }

    #[test]
    fn encode_one_byte() {
        assert_eq!(base64_encode(b"a"), "YQ==");
    }

    #[test]
    fn encode_two_bytes() {
        assert_eq!(base64_encode(b"ab"), "YWI=");
    }

    #[test]
    fn encode_three_bytes() {
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }

    #[test]
    fn encode_binary() {
        assert_eq!(base64_encode(&[0, 1, 2, 255]), "AAEC/w==");
    }

    #[test]
    fn decode_empty() {
        assert_eq!(base64_decode("").unwrap(), b"");
    }

    #[test]
    fn decode_hello() {
        assert_eq!(
            base64_decode("SGVsbG8sIFdvcmxkIQ==").unwrap(),
            b"Hello, World!"
        );
    }

    #[test]
    fn decode_no_padding() {
        assert_eq!(base64_decode("YWJj").unwrap(), b"abc");
    }

    #[test]
    fn decode_one_pad() {
        assert_eq!(base64_decode("YWI=").unwrap(), b"ab");
    }

    #[test]
    fn decode_two_pad() {
        assert_eq!(base64_decode("YQ==").unwrap(), b"a");
    }

    #[test]
    fn decode_with_whitespace() {
        assert_eq!(base64_decode("SGVs\nbG8=").unwrap(), b"Hello");
    }

    #[test]
    fn decode_invalid_char() {
        assert!(base64_decode("!!!").is_err());
    }

    #[test]
    fn decode_invalid_length() {
        assert!(base64_decode("abc").is_err());
    }

    #[test]
    fn roundtrip() {
        let inputs: &[&[u8]] = &[
            b"",
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            b"Hello, World!",
            &[0, 1, 2, 3, 4, 5, 253, 254, 255],
        ];
        for input in inputs {
            let encoded = base64_encode(input);
            let decoded = base64_decode(&encoded).unwrap();
            assert_eq!(&decoded, input, "roundtrip failed for {:?}", input);
        }
    }
}
