//! Python builtin, method, and module-name mappings for Luau generation.

// ── Maps ────────────────────────────────────────────────────────

pub(super) fn builtin_map(name: &str) -> Option<&'static str> {
    match name {
        "len" => Some("py.len"),
        "range" => Some("py.range"),
        "enumerate" => Some("py.enumerate"),
        "zip" => Some("py.zip"),
        "sorted" => Some("py.sorted"),
        "reversed" => Some("py.reversed"),
        "print" => Some("py.print"),
        "int" => Some("py.int"),
        "float" => Some("py.float"),
        "abs" => Some("py.abs"),
        "min" => Some("py.min"),
        "max" => Some("py.max"),
        "sum" => Some("py.sum"),
        "str" => Some("py.str"),
        "bool" => Some("py.bool"),
        "list" => Some("py.list"),
        "dict" => Some("py.dict"),
        "tuple" => Some("py.tuple"),
        "type" => Some("py.type_display"),
        "isinstance" => Some("py.isinstance"),
        "open" => Some("py.open"),
        // Exception constructors
        "ValueError" => Some("py.ValueError"),
        "TypeError" => Some("py.TypeError"),
        "ZeroDivisionError" => Some("py.ZeroDivisionError"),
        "IndexError" => Some("py.IndexError"),
        "KeyError" => Some("py.KeyError"),
        "RuntimeError" => Some("py.RuntimeError"),
        "Exception" => Some("py.Exception"),
        _ => None,
    }
}

pub(super) fn direct_method_map(method: &str) -> Option<&'static str> {
    match method {
        "append" => Some("py.append"),
        "extend" => Some("py.extend"),
        "pop" => Some("py.pop"),
        "insert" => Some("py.insert"),
        "sort" => Some("py.sort"),
        "reverse" => Some("py.reverse"),
        "keys" => Some("py.keys"),
        "values" => Some("py.values"),
        "items" => Some("py.items"),
        "get" => Some("py.get"),
        "update" => Some("py.update"),
        "upper" => Some("py.str_upper"),
        "lower" => Some("py.str_lower"),
        "split" => Some("py.str_split"),
        "join" => Some("py.str_join"),
        "startswith" => Some("py.str_startswith"),
        "endswith" => Some("py.str_endswith"),
        "strip" => Some("py.str_strip"),
        "lstrip" => Some("py.str_lstrip"),
        "rstrip" => Some("py.str_rstrip"),
        "replace" => Some("py.str_replace"),
        "find" => Some("py.str_find"),
        "count" => Some("py.str_count"),
        _ => None,
    }
}

pub(super) fn is_passthrough_module(name: &str) -> bool {
    matches!(
        name,
        "fs" | "math"
            | "http"
            | "compress"
            | "json"
            | "csv"
            | "doc"
            | "sh"
            | "plot"
            | "yaml"
            | "xml"
            | "numx"
            | "random"
            | "fuzzy"
            | "phone"
            | "email"
            | "country"
            | "currency"
            | "datetime"
            | "image"
            | "base64"
            | "fin"
            | "yfinance"
            | "edgar"
            | "crypto"
            | "regex"
            | "html"
            | "url"
            | "qr"
    )
}

/// Map a Python `import X` module name to its Luau equivalent expression.
/// Known modules map to sandbox globals; unknown ones fall through to `require()`.
pub(super) fn python_module_to_luau(module: &str) -> String {
    match module {
        // matplotlib.pyplot → plot global
        "matplotlib.pyplot" | "matplotlib" => "plot".to_string(),
        // seaborn → plot global (same visualization API)
        "seaborn" => "plot".to_string(),
        // numpy → numx global (renamed to avoid agent confusion with real numpy)
        "numpy" => "numx".to_string(),
        // numpy submodules → numx global (linalg/random are sub-tables)
        "numpy.linalg" => "numx.linalg".to_string(),
        "numpy.random" => "numx.random".to_string(),
        // lxml/xml.etree → xml global
        "lxml" | "lxml.etree" | "xml.etree" | "xml.etree.ElementTree" => "xml".to_string(),
        // rapidfuzz/fuzzywuzzy → fuzzy global
        "rapidfuzz" | "rapidfuzz.fuzz" | "rapidfuzz.process" | "fuzzywuzzy" | "fuzzywuzzy.fuzz"
        | "fuzzywuzzy.process" => "fuzzy".to_string(),
        // phonenumbers → phone global
        "phonenumbers" => "phone".to_string(),
        // email_validator → email global
        "email_validator" => "email".to_string(),
        // pycountry → country global
        "pycountry" => "country".to_string(),
        // dateutil → datetime global
        "dateutil" | "dateutil.parser" | "python-dateutil" => "datetime".to_string(),
        // PIL/Pillow → image global (all submodules map to the same sandbox module)
        "PIL" | "PIL.Image" | "PIL.ImageDraw" | "PIL.ImageFilter" | "PIL.ImageFont"
        | "PIL.ImageEnhance" | "PIL.ImageOps" | "Pillow" => "image".to_string(),
        // numpy_financial → fin global
        "numpy_financial" => "fin".to_string(),
        // yfinance → yfinance global
        "yfinance" => "yfinance".to_string(),
        // sec-edgar-downloader / edgar → edgar global
        "sec_edgar_downloader" | "edgar" | "edgartools" => "edgar".to_string(),
        // hashlib / hmac / jwt / uuid / cryptography → crypto global
        "hashlib" | "hmac" | "jwt" | "uuid" | "cryptography" => "crypto".to_string(),
        // re → regex global
        "re" => "regex".to_string(),
        // bs4 / selectolax / html.parser → html global
        "bs4" | "selectolax" | "html.parser" => "html".to_string(),
        // urllib / urllib.parse → url global
        "urllib" | "urllib.parse" => "url".to_string(),
        // qrcode → qr global
        "qrcode" => "qr".to_string(),
        // Passthrough modules are already sandbox globals — no require() needed
        m if is_passthrough_module(m) => m.to_string(),
        // Unknown modules fail immediately with a clear message
        _ => format!(
            r#"error("module '{}' is not available in the sandbox. Run help() to see available modules.")"#,
            module
        ),
    }
}

/// Map a Python `from X import Y` to its Luau equivalent expression.
/// E.g., `from matplotlib import pyplot` → `plot`
pub(super) fn python_from_import_to_luau(module: &str, attr: &str) -> String {
    match (module, attr) {
        ("matplotlib", "pyplot") => "plot".to_string(),
        // from lxml import etree → xml
        ("lxml", "etree") => "xml".to_string(),
        // from bs4 import BeautifulSoup → xml
        ("bs4", "BeautifulSoup") => "xml".to_string(),
        // from xml.etree import ElementTree → xml
        ("xml.etree", "ElementTree") => "xml".to_string(),
        // from numpy import linalg → numx.linalg
        ("numpy", "linalg") => "numx.linalg".to_string(),
        // from numpy import random → numx.random
        ("numpy", "random") => "numx.random".to_string(),
        // from rapidfuzz import fuzz → fuzzy
        ("rapidfuzz", "fuzz") | ("rapidfuzz", "process") => "fuzzy".to_string(),
        // from fuzzywuzzy import fuzz → fuzzy
        ("fuzzywuzzy", "fuzz") | ("fuzzywuzzy", "process") => "fuzzy".to_string(),
        // from phonenumbers import phonenumberutil/PhoneNumber → phone
        ("phonenumbers", _) => "phone".to_string(),
        // from email_validator import validate_email → email
        ("email_validator", _) => "email".to_string(),
        // from pycountry import countries/currencies → country
        ("pycountry", _) => "country".to_string(),
        // from datetime import datetime/timedelta → datetime global
        ("datetime", _) => "datetime".to_string(),
        // from dateutil import parser → datetime global
        ("dateutil", _) => "datetime".to_string(),
        // from PIL import Image → image global
        ("PIL", "Image") | ("PIL", _) => "image".to_string(),
        // from Pillow import ... → image global
        ("Pillow", _) => "image".to_string(),
        // from numpy_financial import npv/irr/etc → fin
        ("numpy_financial", _) => "fin".to_string(),
        // from yfinance import Ticker → yfinance
        ("yfinance", _) => "yfinance".to_string(),
        // from sec_edgar_downloader import Downloader → edgar
        ("sec_edgar_downloader", _) => "edgar".to_string(),
        // from edgar import Company/... → edgar
        ("edgar", _) => "edgar".to_string(),
        ("edgartools", _) => "edgar".to_string(),
        // from urllib.parse import urlparse/urlencode/quote/unquote/parse_qs/urljoin → url.*
        ("urllib.parse", "urlparse") => "url.parse".to_string(),
        ("urllib.parse", "urlencode") => "url.query_build".to_string(),
        ("urllib.parse", "quote") => "url.encode".to_string(),
        ("urllib.parse", "unquote") => "url.decode".to_string(),
        ("urllib.parse", "parse_qs") => "url.query_parse".to_string(),
        ("urllib.parse", "urljoin") => "url.join".to_string(),
        ("urllib.parse", "urlunparse") => "url.build".to_string(),
        _ => format!("require(\"{}\").{}", module, attr),
    }
}

// ── Expression type inference ──────────────────────────────────
