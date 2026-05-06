//! Reqwest-based HTTP backend used on non-macOS platforms.

use super::HttpBackend;
use crate::types::{Headers, HttpError, Method, Request, Response};

pub struct FallbackBackend {
    client: reqwest::blocking::Client,
}

impl FallbackBackend {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl HttpBackend for FallbackBackend {
    fn send(&self, request: &Request) -> Result<Response, HttpError> {
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
            Method::Head => reqwest::Method::HEAD,
            Method::Options => reqwest::Method::OPTIONS,
        };

        let mut builder = self.client.request(method, &request.url);

        for (key, value) in request.headers.iter() {
            builder = builder.header(key, value);
        }

        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }

        let resp = builder
            .send()
            .map_err(|e| HttpError::RequestFailed(e.to_string()))?;

        let status = resp.status().as_u16();
        let mut headers = Headers::new();
        for (key, value) in resp.headers() {
            if let Ok(v) = value.to_str() {
                headers.insert(key.as_str(), v);
            }
        }
        let body = resp
            .bytes()
            .map_err(|e| HttpError::RequestFailed(e.to_string()))?
            .to_vec();

        Ok(Response {
            status,
            headers,
            body,
        })
    }
}
