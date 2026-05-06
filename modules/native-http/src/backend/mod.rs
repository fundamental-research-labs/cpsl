//! Platform backend selection and shared HTTP backend trait.

use crate::types::{HttpError, Request, Response};

#[cfg(not(target_os = "macos"))]
mod fallback;
#[cfg(target_os = "macos")]
mod macos;

/// Platform HTTP backend. Implementations must be Send + Sync.
pub trait HttpBackend: Send + Sync {
    fn send(&self, request: &Request) -> Result<Response, HttpError>;
}

/// Create a backend appropriate for the current platform.
pub fn platform_default() -> Box<dyn HttpBackend> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacosBackend::new())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(fallback::FallbackBackend::new())
    }
}
