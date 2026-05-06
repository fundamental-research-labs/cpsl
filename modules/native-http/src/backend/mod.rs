//! Platform backend selection and shared HTTP backend trait.

use crate::types::{HttpError, Request, Response};

#[cfg(target_os = "emscripten")]
mod emscripten;
#[cfg(all(not(target_os = "macos"), not(target_os = "emscripten")))]
mod fallback;
#[cfg(target_os = "macos")]
mod macos;

/// Platform HTTP backend. Implementations must be Send + Sync.
pub trait HttpBackend: Send + Sync {
    fn send(&self, request: &Request) -> Result<Response, HttpError>;
}

/// Create a backend appropriate for the current platform.
pub fn platform_default() -> Box<dyn HttpBackend> {
    #[cfg(target_os = "emscripten")]
    {
        Box::new(emscripten::EmscriptenBackend::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacosBackend::new())
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "emscripten")))]
    {
        Box::new(fallback::FallbackBackend::new())
    }
}
