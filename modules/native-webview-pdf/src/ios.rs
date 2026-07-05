//! iOS implementation: WKWebView + `createPDF`.
//!
//! `objc2-web-kit` 0.3 exposes a typed `WKWebView` only for macOS because the
//! generated binding models it as an `NSView` subclass. On iOS, use the shared
//! typed configuration classes and send the small `WKWebView` message surface
//! dynamically.

use crate::{PdfError, PdfOptions};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send, MainThreadMarker};
use objc2_core_foundation::CGRect;
use objc2_foundation::{NSData, NSDate, NSError, NSRunLoop, NSString, NSURL};
use objc2_web_kit::{WKPDFConfiguration, WKWebViewConfiguration};
use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};

fn page_frame(opts: &PdfOptions) -> CGRect {
    let (w, h) = if opts.landscape {
        (opts.page_height, opts.page_width)
    } else {
        (opts.page_width, opts.page_height)
    };
    CGRect {
        origin: objc2_core_foundation::CGPoint { x: 0.0, y: 0.0 },
        size: objc2_core_foundation::CGSize {
            width: w * 72.0,
            height: h * 72.0,
        },
    }
}

pub(crate) fn html_to_pdf(html: &str, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    if MainThreadMarker::new().is_some() {
        return html_to_pdf_on_main(html, opts);
    }
    dispatch_pipeline(html, opts)
}

fn new_webview(
    _mtm: MainThreadMarker,
    frame: CGRect,
    config: &WKWebViewConfiguration,
) -> Result<Retained<AnyObject>, PdfError> {
    unsafe {
        let alloc: *mut AnyObject = msg_send![class!(WKWebView), alloc];
        let initialized: *mut AnyObject =
            msg_send![alloc, initWithFrame: frame, configuration: config];
        Retained::from_raw(initialized).ok_or_else(|| {
            PdfError::WebviewCreation("WKWebView initialization returned nil".into())
        })
    }
}

fn load_html(webview: &AnyObject, html: &NSString) {
    let _: Option<Retained<AnyObject>> =
        unsafe { msg_send![webview, loadHTMLString: html, baseURL: Option::<&NSURL>::None] };
}

fn is_loading(webview: &AnyObject) -> bool {
    unsafe { msg_send![webview, isLoading] }
}

fn create_pdf(
    webview: &AnyObject,
    config: &WKPDFConfiguration,
    block: &block2::DynBlock<dyn Fn(*mut NSData, *mut NSError)>,
) {
    let _: () = unsafe {
        msg_send![
            webview,
            createPDFWithConfiguration: Some(config),
            completionHandler: block
        ]
    };
}

fn html_to_pdf_on_main(html: &str, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| PdfError::WebviewCreation("must be called on main thread".into()))?;

    let styled = prepare_html(html, opts);
    let config = unsafe { WKWebViewConfiguration::new(mtm) };
    let webview = new_webview(mtm, page_frame(opts), &config)?;

    let ns = NSString::from_str(&styled);
    load_html(&webview, &ns);

    spin_runloop(|| !is_loading(&webview))?;

    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<u8>, String>>();
    let pdf_config = unsafe { WKPDFConfiguration::new(mtm) };

    let block = block2::RcBlock::new(move |data: *mut NSData, error: *mut NSError| {
        if !error.is_null() {
            let _ = tx.send(Err(unsafe { &*error }.localizedDescription().to_string()));
            return;
        }
        if !data.is_null() {
            let _ = tx.send(Ok(unsafe { (*data).to_vec() }));
        } else {
            let _ = tx.send(Err("no PDF data".into()));
        }
    });

    create_pdf(&webview, &pdf_config, &block);

    let mut result = None;
    spin_runloop(|| match rx.try_recv() {
        Ok(r) => {
            result = Some(r);
            true
        }
        Err(_) => false,
    })?;

    result
        .unwrap_or(Err("timed out".into()))
        .map_err(PdfError::PdfGeneration)
}

fn spin_runloop(mut done: impl FnMut() -> bool) -> Result<(), PdfError> {
    let rl = NSRunLoop::mainRunLoop();
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);
    while !done() {
        if start.elapsed() > timeout {
            return Err(PdfError::PdfGeneration(
                "timed out waiting for webview".into(),
            ));
        }
        let date = NSDate::dateWithTimeIntervalSinceNow(0.01);
        rl.runUntilDate(&date);
    }
    Ok(())
}

#[repr(C)]
struct dispatch_queue_s {
    _opaque: [u8; 0],
}

const DISPATCH_TIME_NOW: u64 = 0;

extern "C" {
    static _dispatch_main_q: dispatch_queue_s;
    fn dispatch_async_f(
        q: *const dispatch_queue_s,
        ctx: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
    fn dispatch_after_f(
        when: u64,
        q: *const dispatch_queue_s,
        ctx: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
    fn dispatch_time(when: u64, delta: i64) -> u64;
}

struct SharedResult {
    value: Mutex<Option<Result<Vec<u8>, PdfError>>>,
    cvar: Condvar,
}

struct Pipeline {
    webview: Option<Retained<AnyObject>>,
    shared: Arc<SharedResult>,
    start: std::time::Instant,
}

struct SetupCtx {
    html: String,
    frame: CGRect,
    shared: Arc<SharedResult>,
}

fn dispatch_pipeline(html: &str, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    let shared = Arc::new(SharedResult {
        value: Mutex::new(None),
        cvar: Condvar::new(),
    });

    let ctx = Box::into_raw(Box::new(SetupCtx {
        html: prepare_html(html, opts),
        frame: page_frame(opts),
        shared: shared.clone(),
    }));

    unsafe {
        dispatch_async_f(
            &_dispatch_main_q as *const dispatch_queue_s,
            ctx as *mut c_void,
            pipeline_setup,
        );
    }

    let mut guard = shared.value.lock().unwrap();
    let timeout = std::time::Duration::from_secs(35);
    loop {
        if let Some(result) = guard.take() {
            return result;
        }
        let (g, wait) = shared.cvar.wait_timeout(guard, timeout).unwrap();
        guard = g;
        if wait.timed_out() && guard.is_none() {
            return Err(PdfError::PdfGeneration(
                "timed out waiting for main thread".into(),
            ));
        }
    }
}

extern "C" fn pipeline_setup(ptr: *mut c_void) {
    let ctx = unsafe { Box::from_raw(ptr as *mut SetupCtx) };
    let mtm = match MainThreadMarker::new() {
        Some(m) => m,
        None => {
            signal_result(
                &ctx.shared,
                Err(PdfError::WebviewCreation("not main thread".into())),
            );
            return;
        }
    };

    let config = unsafe { WKWebViewConfiguration::new(mtm) };
    let webview = match new_webview(mtm, ctx.frame, &config) {
        Ok(webview) => webview,
        Err(error) => {
            signal_result(&ctx.shared, Err(error));
            return;
        }
    };
    let ns = NSString::from_str(&ctx.html);
    load_html(&webview, &ns);

    let pipe = Box::into_raw(Box::new(Pipeline {
        webview: Some(webview),
        shared: ctx.shared,
        start: std::time::Instant::now(),
    }));

    let when = unsafe { dispatch_time(DISPATCH_TIME_NOW, 10_000_000) };
    unsafe {
        dispatch_after_f(
            when,
            &_dispatch_main_q as *const dispatch_queue_s,
            pipe as *mut c_void,
            pipeline_poll,
        );
    }
}

extern "C" fn pipeline_poll(ptr: *mut c_void) {
    let pipe = unsafe { &mut *(ptr as *mut Pipeline) };

    if pipe.start.elapsed() > std::time::Duration::from_secs(30) {
        let shared = pipe.shared.clone();
        unsafe { drop(Box::from_raw(ptr as *mut Pipeline)) };
        signal_result(
            &shared,
            Err(PdfError::PdfGeneration(
                "timed out waiting for webview".into(),
            )),
        );
        return;
    }

    let loading = pipe.webview.as_ref().map_or(false, |wv| is_loading(wv));
    if loading {
        let when = unsafe { dispatch_time(DISPATCH_TIME_NOW, 10_000_000) };
        unsafe {
            dispatch_after_f(
                when,
                &_dispatch_main_q as *const dispatch_queue_s,
                ptr,
                pipeline_poll,
            );
        }
        return;
    }

    pipeline_create_pdf(ptr);
}

fn pipeline_create_pdf(ptr: *mut c_void) {
    let pipe = unsafe { &mut *(ptr as *mut Pipeline) };
    let mtm = MainThreadMarker::new().unwrap();
    let webview = pipe.webview.as_ref().unwrap();
    let pdf_config = unsafe { WKPDFConfiguration::new(mtm) };
    let shared = pipe.shared.clone();

    let block = block2::RcBlock::new(move |data: *mut NSData, error: *mut NSError| {
        let result = if !error.is_null() {
            Err(PdfError::PdfGeneration(
                unsafe { &*error }.localizedDescription().to_string(),
            ))
        } else if !data.is_null() {
            Ok(unsafe { (*data).to_vec() })
        } else {
            Err(PdfError::PdfGeneration("no PDF data".into()))
        };

        unsafe { drop(Box::from_raw(ptr as *mut Pipeline)) };
        signal_result(&shared, result);
    });

    create_pdf(webview, &pdf_config, &block);
}

fn signal_result(shared: &SharedResult, result: Result<Vec<u8>, PdfError>) {
    *shared.value.lock().unwrap() = Some(result);
    shared.cvar.notify_one();
}

fn prepare_html(html: &str, opts: &PdfOptions) -> String {
    let (w, h) = if opts.landscape {
        (opts.page_height, opts.page_width)
    } else {
        (opts.page_width, opts.page_height)
    };
    let page_w_pt = w * 72.0;
    let mt_pt = opts.margin_top * 72.0;
    let mr_pt = opts.margin_right * 72.0;
    let mb_pt = opts.margin_bottom * 72.0;
    let ml_pt = opts.margin_left * 72.0;
    format!(
        r#"<meta name="viewport" content="width={page_w_pt}"><style>@page {{ size: {w}in {h}in; margin: {mt}in {mr}in {mb}in {ml}in; }} *, *::before, *::after {{ box-sizing: border-box; }} html {{ margin: 0; padding: 0; }} body {{ width: {page_w_pt}px !important; margin: 0 auto !important; padding: {mt_pt}px {mr_pt}px {mb_pt}px {ml_pt}px !important; }}</style>{html}"#,
        mt = opts.margin_top,
        mr = opts.margin_right,
        mb = opts.margin_bottom,
        ml = opts.margin_left,
    )
}
