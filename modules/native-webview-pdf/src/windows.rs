//! Windows implementation: WebView2 + `PrintToPdfStream`.
//!
//! Creates a hidden window, initializes WebView2 via COM, loads HTML,
//! waits for navigation to complete, then calls `PrintToPdfStream` and
//! returns the resulting bytes.

use crate::{PdfError, PdfOptions};
use std::sync::mpsc;
use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler,
    PrintToPdfStreamCompletedHandler,
};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub(crate) fn html_to_pdf(html: &str, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    unsafe {
        // Initialize COM for this thread.
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| PdfError::WebviewCreation(format!("COM init failed: {}", e)))?;
    }

    let result = html_to_pdf_inner(html, opts);

    unsafe {
        CoUninitialize();
    }

    result
}

fn html_to_pdf_inner(html: &str, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    let styled_html = prepare_html(html, opts);

    // Create a hidden window as parent for WebView2.
    let hwnd = create_hidden_window()?;

    // Create the WebView2 environment.
    let env = create_environment()?;

    // Create a controller attached to the hidden window.
    let controller = create_controller(&env, hwnd)?;

    // Get the core webview.
    let webview = unsafe {
        controller
            .CoreWebView2()
            .map_err(|e| PdfError::WebviewCreation(format!("get webview failed: {}", e)))?
    };

    // Navigate to the HTML string.
    let html_wide: Vec<u16> = styled_html
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        webview
            .NavigateToString(PCWSTR(html_wide.as_ptr()))
            .map_err(|e| PdfError::HtmlLoading(format!("NavigateToString failed: {}", e)))?;
    }

    // Wait for navigation to complete.
    wait_for_navigation(&webview)?;

    // Print to PDF stream via ICoreWebView2_20.
    let pdf_bytes = print_to_pdf_stream(&webview, opts)?;

    // Cleanup.
    unsafe {
        let _ = DestroyWindow(hwnd);
    }

    Ok(pdf_bytes)
}

unsafe extern "system" fn def_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn create_hidden_window() -> Result<HWND, PdfError> {
    use windows::core::w;

    let class_name = w!("NativeWebviewPdfHidden");

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(def_wnd_proc),
        lpszClassName: class_name,
        ..Default::default()
    };

    unsafe {
        RegisterClassExW(&wc);
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!(""),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1,
            1,
            None,
            None,
            None,
            None,
        )
    }
    .map_err(|e| PdfError::WebviewCreation(format!("CreateWindowExW failed: {}", e)))?;

    Ok(hwnd)
}

fn create_environment() -> Result<ICoreWebView2Environment, PdfError> {
    let (tx, rx) = mpsc::channel();
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(|handler| unsafe { CreateCoreWebView2Environment(&handler) }.map_err(Into::into)),
        Box::new(move |hr, env| {
            hr?;
            tx.send(env.ok_or_else(|| {
                windows::core::Error::new(windows::core::HRESULT(-1), "environment was null")
            }))
            .expect("send over mpsc channel");
            Ok(())
        }),
    )
    .map_err(|e| PdfError::WebviewCreation(format!("create environment failed: {}", e)))?;

    rx.recv()
        .map_err(|e| PdfError::WebviewCreation(format!("recv environment: {}", e)))?
        .map_err(|e| PdfError::WebviewCreation(format!("create environment failed: {}", e)))
}

fn create_controller(
    env: &ICoreWebView2Environment,
    hwnd: HWND,
) -> Result<ICoreWebView2Controller, PdfError> {
    let env_clone = env.clone();
    let (tx, rx) = mpsc::channel();
    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| {
            unsafe { env_clone.CreateCoreWebView2Controller(hwnd, &handler) }.map_err(Into::into)
        }),
        Box::new(move |hr, controller| {
            hr?;
            tx.send(controller.ok_or_else(|| {
                windows::core::Error::new(windows::core::HRESULT(-1), "controller was null")
            }))
            .expect("send over mpsc channel");
            Ok(())
        }),
    )
    .map_err(|e| PdfError::WebviewCreation(format!("create controller failed: {}", e)))?;

    rx.recv()
        .map_err(|e| PdfError::WebviewCreation(format!("recv controller: {}", e)))?
        .map_err(|e| PdfError::WebviewCreation(format!("create controller failed: {}", e)))
}

fn wait_for_navigation(webview: &ICoreWebView2) -> Result<(), PdfError> {
    let (tx, rx) = mpsc::channel::<std::result::Result<(), String>>();

    let handler = NavigationCompletedEventHandler::create(Box::new(move |_webview, args| {
        if let Some(args) = args {
            let mut success = false.into();
            unsafe {
                let _ = args.IsSuccess(&mut success);
            }
            if success.as_bool() {
                let _ = tx.send(Ok(()));
            } else {
                let _ = tx.send(Err("navigation failed".into()));
            }
        } else {
            let _ = tx.send(Err("no navigation args".into()));
        }
        Ok(())
    }));

    let mut token: i64 = 0;
    unsafe {
        webview
            .add_NavigationCompleted(&handler, &mut token)
            .map_err(|e| PdfError::HtmlLoading(format!("add_NavigationCompleted: {}", e)))?;
    }

    // Pump the message loop until navigation completes.
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);
    loop {
        if let Ok(result) = rx.try_recv() {
            result.map_err(PdfError::HtmlLoading)?;
            break;
        }
        if start.elapsed() > timeout {
            return Err(PdfError::HtmlLoading("navigation timed out".into()));
        }
        pump_messages();
    }

    Ok(())
}

fn print_to_pdf_stream(webview: &ICoreWebView2, _opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    use windows::core::Interface;

    // QI to ICoreWebView2_20 for PrintToPdfStream.
    let webview20: ICoreWebView2_20 = webview
        .cast()
        .map_err(|e| PdfError::PdfGeneration(format!("QI to ICoreWebView2_20: {}", e)))?;

    // Create print settings.
    let env6: ICoreWebView2Environment6 = {
        let wv2: ICoreWebView2_2 = webview
            .cast()
            .map_err(|e| PdfError::PdfGeneration(format!("QI to ICoreWebView2_2: {}", e)))?;
        let env_obj = unsafe {
            wv2.Environment()
                .map_err(|e| PdfError::PdfGeneration(format!("get environment: {}", e)))?
        };
        env_obj
            .cast()
            .map_err(|e| PdfError::PdfGeneration(format!("QI to env6: {}", e)))?
    };

    let settings = unsafe {
        env6.CreatePrintSettings()
            .map_err(|e| PdfError::PdfGeneration(format!("CreatePrintSettings: {}", e)))?
    };

    // Use PrintToPdfStream with these settings.
    let (tx, rx) = mpsc::channel::<std::result::Result<Vec<u8>, String>>();

    let handler = PrintToPdfStreamCompletedHandler::create(Box::new(move |hr, stream| {
        if let Err(e) = hr {
            let _ = tx.send(Err(format!("PrintToPdfStream failed: {}", e)));
            return Ok(());
        }
        if let Some(stream) = stream {
            match read_stream(&stream) {
                Ok(bytes) => {
                    let _ = tx.send(Ok(bytes));
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("read stream: {}", e)));
                }
            }
        } else {
            let _ = tx.send(Err("no stream returned".into()));
        }
        Ok(())
    }));

    unsafe {
        webview20
            .PrintToPdfStream(&settings, &handler)
            .map_err(|e| PdfError::PdfGeneration(format!("PrintToPdfStream call: {}", e)))?;
    }

    // Pump messages until PDF is ready.
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);
    loop {
        if let Ok(result) = rx.try_recv() {
            return result.map_err(PdfError::PdfGeneration);
        }
        if start.elapsed() > timeout {
            return Err(PdfError::PdfGeneration("PDF generation timed out".into()));
        }
        pump_messages();
    }
}

/// Read all bytes from an IStream into a Vec<u8>.
fn read_stream(stream: &IStream) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];

    loop {
        let mut bytes_read = 0u32;
        unsafe {
            stream
                .Read(
                    chunk.as_mut_ptr() as *mut _,
                    chunk.len() as u32,
                    Some(&mut bytes_read),
                )
                .ok()
                .map_err(|e| format!("IStream::Read: {}", e))?;
        }
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read as usize]);
    }

    Ok(buffer)
}

/// Pump the Windows message loop briefly.
fn pump_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(10));
}

/// Wrap HTML with `@page` CSS rules, viewport meta, and explicit body width
/// so layout is deterministic regardless of the hidden window size.
fn prepare_html(html: &str, opts: &PdfOptions) -> String {
    let (w, h) = if opts.landscape {
        (opts.page_height, opts.page_width)
    } else {
        (opts.page_width, opts.page_height)
    };
    let content_w_pt = (w - opts.margin_left - opts.margin_right) * 72.0;
    format!(
        r#"<meta name="viewport" content="width={content_w_pt}"><style>@page {{ size: {w}in {h}in; margin: {mt}in {mr}in {mb}in {ml}in; }} *, *::before, *::after {{ box-sizing: border-box; }} html, body {{ width: {content_w_pt}px; margin: 0; padding: 0; }}</style>{html}"#,
        mt = opts.margin_top,
        mr = opts.margin_right,
        mb = opts.margin_bottom,
        ml = opts.margin_left,
    )
}
