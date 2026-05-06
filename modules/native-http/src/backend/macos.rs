//! macOS HTTP backend implemented with NSURLSession.

use super::HttpBackend;
use crate::types::{Headers, HttpError, Method, Request, Response};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSData, NSMutableURLRequest, NSString, NSURLResponse, NSURLSession, NSURL};
use std::sync::mpsc;

pub struct MacosBackend {
    session: Retained<NSURLSession>,
}

// Safety: NSURLSession is thread-safe per Apple docs.
unsafe impl Send for MacosBackend {}
unsafe impl Sync for MacosBackend {}

impl MacosBackend {
    pub fn new() -> Self {
        let session = NSURLSession::sharedSession();
        Self { session }
    }
}

impl HttpBackend for MacosBackend {
    fn send(&self, request: &Request) -> Result<Response, HttpError> {
        let url_string = NSString::from_str(&request.url);
        let url = NSURL::URLWithString(&url_string)
            .ok_or_else(|| HttpError::InvalidUrl(request.url.clone()))?;

        let ns_request = NSMutableURLRequest::requestWithURL(&url);

        let method_str = match request.method {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
        };
        ns_request.setHTTPMethod(&NSString::from_str(method_str));

        for (key, value) in request.headers.iter() {
            ns_request.setValue_forHTTPHeaderField(
                Some(&NSString::from_str(value)),
                &NSString::from_str(key),
            );
        }

        if let Some(body) = &request.body {
            let ns_data = NSData::with_bytes(body);
            ns_request.setHTTPBody(Some(&ns_data));
        }

        // Synchronous dispatch via channel
        let (tx, rx) = mpsc::channel::<Result<Response, HttpError>>();

        // Completion handler: (NSData?, NSURLResponse?, NSError?) -> Void
        let block = block2::RcBlock::new(
            move |data: *mut NSData,
                  response: *mut NSURLResponse,
                  error: *mut objc2_foundation::NSError| {
                if !error.is_null() {
                    let err = unsafe { &*error };
                    let description = err.localizedDescription().to_string();
                    let _ = tx.send(Err(HttpError::RequestFailed(description)));
                    return;
                }

                // statusCode is on NSHTTPURLResponse (subclass of NSURLResponse)
                let status: u16 = if response.is_null() {
                    0
                } else {
                    let code: isize = unsafe { msg_send![&*response, statusCode] };
                    code as u16
                };

                let mut headers = Headers::new();
                if !response.is_null() {
                    let dict: Option<Retained<AnyObject>> =
                        unsafe { msg_send![&*response, allHeaderFields] };
                    if let Some(dict) = dict {
                        let keys: Retained<AnyObject> = unsafe { msg_send![&*dict, allKeys] };
                        let count: usize = unsafe { msg_send![&*keys, count] };
                        for i in 0..count {
                            let key: Retained<AnyObject> =
                                unsafe { msg_send![&*keys, objectAtIndex: i] };
                            let val: Option<Retained<AnyObject>> =
                                unsafe { msg_send![&*dict, objectForKey: &*key] };
                            if let Some(val) = val {
                                // Use ObjC `description` (available on any NSObject) instead
                                // of transmuting to NSString — safe even if the dict contains
                                // non-NSString values (which shouldn't happen for HTTP headers,
                                // but we don't want UB if it ever does).
                                let key_desc: Retained<NSString> =
                                    unsafe { msg_send![&*key, description] };
                                let val_desc: Retained<NSString> =
                                    unsafe { msg_send![&*val, description] };
                                headers.insert(key_desc.to_string(), val_desc.to_string());
                            }
                        }
                    }
                }

                let body = if data.is_null() {
                    Vec::new()
                } else {
                    unsafe { (*data).to_vec() }
                };

                let _ = tx.send(Ok(Response {
                    status,
                    headers,
                    body,
                }));
            },
        );

        let task = unsafe {
            self.session
                .dataTaskWithRequest_completionHandler(&ns_request, &block)
        };
        task.resume();

        rx.recv()
            .map_err(|_| HttpError::RequestFailed("completion handler never called".into()))?
    }
}
