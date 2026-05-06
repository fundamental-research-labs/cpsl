//! Native HTTP transport with domain policy and credential injection.

mod backend;
mod gateway;
mod policy;
mod types;

pub use backend::HttpBackend;
pub use gateway::HttpGateway;
pub use policy::{CredentialStore, DomainPolicy, DomainPrompt, PromptResponse};
pub use types::{Headers, HttpError, Limits, Method, Request, Response};
