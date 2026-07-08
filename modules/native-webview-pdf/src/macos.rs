//! Apple implementation: WKWebView + `createPDF`.
//!
//! WKWebView MUST live on the main thread. Two code paths:
//!
//! **Fast path** (already on main thread): Create the webview, spin the run loop
//! until loading completes, call `createPDF`, spin until PDF bytes arrive.
//!
//! **Background-thread path**: Break work into non-blocking phases dispatched
//! to the GCD main queue. Between phases, the main queue is free to process
//! WebKit callbacks that would otherwise be blocked by a synchronous dispatch.
//! The calling thread blocks on a condvar until the result is ready.

use crate::{PdfError, PdfOptions};
use objc2::MainThreadMarker;
use objc2_core_foundation::CGRect;
use objc2_foundation::{NSData, NSDate, NSRunLoop, NSString};
use objc2_web_kit::{WKPDFConfiguration, WKWebViewConfiguration};
use std::ffi::c_void;
use std::sync::{mpsc, Arc, Condvar, Mutex};

#[cfg(target_os = "ios")]
use objc2::rc::Retained;
#[cfg(target_os = "ios")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2::MainThreadOnly;
#[cfg(target_os = "ios")]
use objc2::{class, msg_send};
#[cfg(target_os = "macos")]
use objc2_web_kit::WKWebView;

#[cfg(target_os = "macos")]
type PlatformWebView = WKWebView;
#[cfg(target_os = "ios")]
type PlatformWebView = AnyObject;

type PdfCompletionBlock = block2::DynBlock<dyn Fn(*mut NSData, *mut objc2_foundation::NSError)>;

/// Compute the full-page frame in points (72pt/in).
///
/// We size the WKWebView to the full page (not just the content area)
/// because `createPDF` captures the webview frame as-is and does NOT
/// honour `@page` CSS margins.  Margins are applied as body padding
/// in `prepare_html` instead.
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

// ═══════════════════════════════════════════════════════════════════
// Fast path — called directly on the main thread (CLI / tests)
// ═══════════════════════════════════════════════════════════════════

fn html_to_pdf_on_main(html: &str, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| PdfError::WebviewCreation("must be called on main thread".into()))?;

    let styled = prepare_html(html, opts);
    let config = unsafe { WKWebViewConfiguration::new(mtm) };
    let frame = page_frame(opts);
    let webview = create_webview(mtm, frame, &config);

    let ns = NSString::from_str(&styled);
    load_html(&webview, &ns);

    spin_runloop(|| !webview_is_loading(&webview))?;

    let (tx, rx) = mpsc::channel::<Result<Vec<u8>, String>>();
    let pdf_config = unsafe { WKPDFConfiguration::new(mtm) };

    let block = block2::RcBlock::new(
        move |data: *mut NSData, error: *mut objc2_foundation::NSError| {
            if !error.is_null() {
                let _ = tx.send(Err(unsafe { &*error }.localizedDescription().to_string()));
                return;
            }
            if !data.is_null() {
                let _ = tx.send(Ok(unsafe { (*data).to_vec() }));
            } else {
                let _ = tx.send(Err("no PDF data".into()));
            }
        },
    );

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

/// Spin the main run loop in 10ms increments until `done()` returns true.
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

// ═══════════════════════════════════════════════════════════════════
// Background-thread pipeline — non-blocking multi-phase GCD dispatch
//
// Phase 1 (dispatch_async):  Create WKWebView, load HTML
// Phase 2 (dispatch_after):  Poll isLoading() every 10ms
// Phase 3 (continuation):    Call createPDF
// Phase 4 (completion block): Collect bytes, signal caller
//
// Each phase is a SEPARATE GCD block so the main serial queue is
// free between phases — WebKit's internal callbacks can execute.
// ═══════════════════════════════════════════════════════════════════

// ── GCD FFI ──────────────────────────────────────────────────────

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

// ── Shared state ─────────────────────────────────────────────────

/// Condvar-guarded result shared between the pipeline (main thread)
/// and the caller (background thread).
struct SharedResult {
    value: Mutex<Option<Result<Vec<u8>, PdfError>>>,
    cvar: Condvar,
}

/// Pipeline state — heap-allocated, only ever accessed on the main thread.
struct Pipeline {
    webview: Option<objc2::rc::Retained<PlatformWebView>>,
    shared: Arc<SharedResult>,
    start: std::time::Instant,
}

/// Setup context for the first dispatch_async_f call.
struct SetupCtx {
    html: String,
    frame: CGRect,
    shared: Arc<SharedResult>,
}

// ── Entry point (background thread) ──────────────────────────────

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

    // Block until the pipeline signals completion (or our own timeout).
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

// ── Phase 1: create webview and load HTML ────────────────────────

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
    let webview = create_webview(mtm, ctx.frame, &config);
    let ns = NSString::from_str(&ctx.html);
    load_html(&webview, &ns);

    let pipe = Box::into_raw(Box::new(Pipeline {
        webview: Some(webview),
        shared: ctx.shared,
        start: std::time::Instant::now(),
    }));

    // Schedule first loading check in 10ms.
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

// ── Phase 2: poll isLoading() ────────────────────────────────────

extern "C" fn pipeline_poll(ptr: *mut c_void) {
    let pipe = unsafe { &mut *(ptr as *mut Pipeline) };

    // Internal timeout.
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

    let loading = pipe
        .webview
        .as_ref()
        .map_or(false, |wv| webview_is_loading(wv));

    if loading {
        // Not done yet — re-schedule in 10ms.
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

    // Loading complete → generate PDF.
    pipeline_create_pdf(ptr);
}

// ── Phase 3 + 4: createPDF → completion handler ─────────────────

fn pipeline_create_pdf(ptr: *mut c_void) {
    let pipe = unsafe { &mut *(ptr as *mut Pipeline) };
    let mtm = MainThreadMarker::new().unwrap();
    let webview = pipe.webview.as_ref().unwrap();
    let pdf_config = unsafe { WKPDFConfiguration::new(mtm) };

    let shared = pipe.shared.clone();

    let block = block2::RcBlock::new(
        move |data: *mut NSData, error: *mut objc2_foundation::NSError| {
            let result = if !error.is_null() {
                Err(PdfError::PdfGeneration(
                    unsafe { &*error }.localizedDescription().to_string(),
                ))
            } else if !data.is_null() {
                Ok(unsafe { (*data).to_vec() })
            } else {
                Err(PdfError::PdfGeneration("no PDF data".into()))
            };

            // Clean up the pipeline (drops the webview on the main thread).
            unsafe { drop(Box::from_raw(ptr as *mut Pipeline)) };
            signal_result(&shared, result);
        },
    );

    create_pdf(webview, &pdf_config, &block);
}

/// Signal the background thread with the final result.
fn signal_result(shared: &SharedResult, result: Result<Vec<u8>, PdfError>) {
    *shared.value.lock().unwrap() = Some(result);
    shared.cvar.notify_one();
}

// ═══════════════════════════════════════════════════════════════════

#[cfg(target_os = "macos")]
fn create_webview(
    mtm: MainThreadMarker,
    frame: CGRect,
    config: &WKWebViewConfiguration,
) -> objc2::rc::Retained<PlatformWebView> {
    unsafe { WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, config) }
}

#[cfg(target_os = "ios")]
fn create_webview(
    _mtm: MainThreadMarker,
    frame: CGRect,
    config: &WKWebViewConfiguration,
) -> Retained<PlatformWebView> {
    let allocated: *mut AnyObject = unsafe { msg_send![class!(WKWebView), alloc] };
    let webview: *mut AnyObject =
        unsafe { msg_send![allocated, initWithFrame: frame, configuration: config] };
    unsafe { Retained::from_raw(webview) }
        .expect("WKWebView initWithFrame:configuration: returned nil")
}

#[cfg(target_os = "macos")]
fn load_html(webview: &PlatformWebView, html: &NSString) {
    unsafe {
        webview.loadHTMLString_baseURL(html, None);
    }
}

#[cfg(target_os = "ios")]
fn load_html(webview: &PlatformWebView, html: &NSString) {
    let _: *mut AnyObject = unsafe {
        msg_send![webview, loadHTMLString: html, baseURL: std::ptr::null_mut::<AnyObject>()]
    };
}

#[cfg(target_os = "macos")]
fn webview_is_loading(webview: &PlatformWebView) -> bool {
    unsafe { webview.isLoading() }
}

#[cfg(target_os = "ios")]
fn webview_is_loading(webview: &PlatformWebView) -> bool {
    unsafe { msg_send![webview, isLoading] }
}

#[cfg(target_os = "macos")]
fn create_pdf(
    webview: &PlatformWebView,
    pdf_config: &WKPDFConfiguration,
    block: &PdfCompletionBlock,
) {
    unsafe {
        webview.createPDFWithConfiguration_completionHandler(Some(pdf_config), block);
    }
}

#[cfg(target_os = "ios")]
fn create_pdf(
    webview: &PlatformWebView,
    pdf_config: &WKPDFConfiguration,
    block: &PdfCompletionBlock,
) {
    let _: () = unsafe {
        msg_send![webview, createPDFWithConfiguration: pdf_config, completionHandler: block]
    };
}

fn prepare_html(html: &str, opts: &PdfOptions) -> String {
    let (w, h) = if opts.landscape {
        (opts.page_height, opts.page_width)
    } else {
        (opts.page_width, opts.page_height)
    };
    // Full page width in points — matches the WKWebView frame.
    let page_w_pt = w * 72.0;
    // Margin values in points for body padding.
    let mt_pt = opts.margin_top * 72.0;
    let mr_pt = opts.margin_right * 72.0;
    let mb_pt = opts.margin_bottom * 72.0;
    let ml_pt = opts.margin_left * 72.0;
    // WKWebView `createPDF` does NOT honour @page margins, so we apply them
    // as body padding instead.  The viewport and body width equal the full
    // page width; with box-sizing:border-box the content area ends up at
    // (page_w - margin_left - margin_right) points — exactly the intended
    // content width.  We keep @page as a hint for any future WebKit support.
    //
    // `margin: 0 auto` centers the body when the viewport is wider than the
    // body (shouldn't happen with our setup, but is a safe fallback).
    //
    // `!important` on body width/padding because these enforce physical page
    // geometry — content CSS must not override them (e.g., `body { padding: 0 }`
    // in the agent's HTML would silently eat the margins without it).
    format!(
        r#"<meta name="viewport" content="width={page_w_pt}"><style>@page {{ size: {w}in {h}in; margin: {mt}in {mr}in {mb}in {ml}in; }} *, *::before, *::after {{ box-sizing: border-box; }} html {{ margin: 0; padding: 0; }} body {{ width: {page_w_pt}px !important; margin: 0 auto !important; padding: {mt_pt}px {mr_pt}px {mb_pt}px {ml_pt}px !important; }}</style>{html}"#,
        mt = opts.margin_top,
        mr = opts.margin_right,
        mb = opts.margin_bottom,
        ml = opts.margin_left,
    )
}
