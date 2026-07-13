//! Core sandbox, transpilation, and built-in module exports for CPSL.

#[cfg(feature = "mod-base64")]
pub(crate) mod base64;
#[cfg(any(feature = "mod-base64", feature = "mod-fs"))]
mod base64_codec;
#[cfg(feature = "mod-apple-calendar")]
pub(crate) mod calendar;
#[cfg(feature = "mod-compress")]
pub(crate) mod compress;
#[cfg(feature = "mod-country")]
pub(crate) mod country;
#[cfg(feature = "mod-crypto")]
pub(crate) mod crypto;
#[cfg(feature = "mod-csv")]
pub(crate) mod csv_mod;
#[cfg(feature = "mod-datetime")]
pub(crate) mod datetime;
#[cfg(feature = "mod-doc")]
pub(crate) mod doc;
#[cfg(feature = "mod-doc")]
pub(crate) mod doc_reader;
#[cfg(feature = "mod-edgar")]
pub(crate) mod edgar;
#[cfg(feature = "mod-email")]
pub(crate) mod email;
#[cfg(feature = "mod-fin")]
pub(crate) mod fin;
#[cfg(feature = "mod-fuzzy")]
pub(crate) mod fuzzy;
#[cfg(any(feature = "mod-ripgrep", feature = "mod-fff"))]
pub(crate) mod grep_api;
#[cfg(feature = "mod-html")]
pub(crate) mod html_mod;
#[cfg(feature = "mod-http")]
pub(crate) mod http;
#[cfg(feature = "mod-image")]
pub(crate) mod image;
#[cfg(feature = "mod-json")]
pub(crate) mod json;
#[cfg(feature = "mod-location")]
pub(crate) mod location;
pub(crate) mod lua_util;
mod mount;
#[cfg(feature = "mod-numpy")]
pub(crate) mod numpy;
#[cfg(feature = "pdfium-render")]
mod pdfium_engine;
#[cfg(feature = "mod-phone")]
pub(crate) mod phone;
#[cfg(feature = "mod-plot")]
pub(crate) mod plot;
pub(crate) mod pyrt_compat;
#[cfg(feature = "mod-qr")]
pub(crate) mod qr;
#[cfg(feature = "mod-random")]
pub(crate) mod random;
#[cfg(feature = "mod-regex")]
pub(crate) mod regex_mod;
mod sandbox;
#[cfg(cpsl_experimental_sfae)]
pub(crate) mod sfae;
pub mod sh_transpile;
pub mod transpile;
#[cfg(feature = "mod-url")]
pub(crate) mod url_mod;
#[cfg(feature = "mod-webbrowser")]
pub(crate) mod webbrowser;
#[cfg(feature = "mod-xml")]
pub(crate) mod xml;
#[cfg(feature = "mod-yaml")]
pub(crate) mod yaml;
#[cfg(feature = "mod-yfinance")]
pub(crate) mod yfinance;

#[cfg(feature = "mod-apple-calendar")]
pub use apple_calendar::AppleCalendarGateway;
#[cfg(feature = "mod-apple-calendar")]
pub use calendar::CalendarActivityCallback;
#[cfg(feature = "mod-location")]
pub use location::LocationGateway;
pub use mount::{MountError, MountPermission, MountTable};
#[cfg(feature = "mod-http")]
pub use native_http::HttpGateway;
#[cfg(feature = "pdfium-render")]
pub use pdfium_engine::PdfiumEngine;
pub use sandbox::{
    clean_lua_error, humanize_error, DocReadCallback, ExecError, FileActivityCallback, Sandbox,
    SandboxBuilder, SandboxError, VisionCallback, VisionInput,
};
#[cfg(cpsl_experimental_sfae)]
pub use sfae::{BrowserOpener, CredentialPrompt};
#[cfg(feature = "mod-webbrowser")]
pub use webbrowser::WebBrowserGateway;
