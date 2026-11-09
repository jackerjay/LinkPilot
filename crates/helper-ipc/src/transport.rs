//! XPC with mutual system-enforced code requirements and a non-sensitive hello
//! before transmitting user data. The Mach name is scoped to the signing team.
use crate::{Capabilities, Command, Reply, Request, Response, MAX_FRAME, PROTOCOL};
use std::ffi::{c_char, CStr};

extern "C" {
    fn lp_helper_service_name() -> *mut c_char;
    fn lp_helper_start() -> i32;
    fn lp_helper_request(
        bytes: *const u8,
        length: usize,
        out_length: *mut usize,
        error: *mut i32,
    ) -> *mut u8;
    fn lp_helper_serve(
        handler: extern "C" fn(*const u8, usize, *mut usize) -> *mut u8,
        release: extern "C" fn(*mut u8, usize),
    ) -> i32;
}

pub fn service_name() -> Result<String, String> {
    unsafe {
        let ptr = lp_helper_service_name();
        if ptr.is_null() {
            return Err("Helper requires an Apple-signed bundle".into());
        }
        let name = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        libc::free(ptr.cast());
        Ok(name)
    }
}

pub fn start() -> Result<(), String> {
    match unsafe { lp_helper_start() } {
        0 => Ok(()),
        -2 => {
            Err("Install and open LinkPilot Helper from the official GitHub release first".into())
        }
        code => Err(format!(
            "Cannot start trusted LinkPilot Helper (macOS {code})"
        )),
    }
}

pub fn request(command: Command) -> Result<Response, String> {
    let bytes = serde_json::to_vec(&Request {
        protocol: PROTOCOL,
        command,
    })
    .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FRAME {
        return Err("Oversized Helper request".into());
    }
    let mut size = 0;
    let mut error = 0;
    let ptr = unsafe { lp_helper_request(bytes.as_ptr(), bytes.len(), &mut size, &mut error) };
    if ptr.is_null() {
        return Err(match error {
            -3 => "Helper peer signature rejected; no request was sent".into(),
            -2 => "LinkPilot Helper is unavailable or untrusted. Install/open the official Helper and retry".into(),
            _ => format!("Helper request was not confirmed ({error}). Check the browser before retrying"),
        });
    }
    let reply = unsafe {
        let result = serde_json::from_slice::<Reply>(std::slice::from_raw_parts(ptr, size));
        libc::free(ptr.cast());
        result
    }
    .map_err(|e| e.to_string())?;
    if reply.protocol != PROTOCOL {
        return Err("Helper protocol is incompatible. Update LinkPilot and Helper together".into());
    }
    match reply.response {
        Response::Error { message } => Err(message),
        response => Ok(response),
    }
}

pub fn status() -> Result<Capabilities, String> {
    match request(Command::Status)? {
        Response::Status { capabilities } => Ok(capabilities),
        _ => Err("Unexpected Helper status response".into()),
    }
}

/// The dedicated fixed-protocol handler is the entire exposed privilege surface.
pub fn serve(handler: fn(Command) -> Result<Response, String>) -> Result<(), String> {
    type Handler = fn(Command) -> Result<Response, String>;
    static HANDLER: std::sync::OnceLock<Handler> = std::sync::OnceLock::new();
    HANDLER
        .set(handler)
        .map_err(|_| "Helper is already serving".to_string())?;
    extern "C" fn receive(bytes: *const u8, length: usize, output: *mut usize) -> *mut u8 {
        let result = std::panic::catch_unwind(|| {
            let reply = crate::dispatch_request(
                unsafe { std::slice::from_raw_parts(bytes, length) },
                HANDLER.get().expect("registered handler"),
            );
            serde_json::to_vec(&reply)
        });
        match result {
            Ok(Ok(bytes)) if bytes.len() <= MAX_FRAME => {
                let bytes = bytes.into_boxed_slice();
                unsafe {
                    *output = bytes.len();
                }
                Box::into_raw(bytes).cast()
            }
            _ => std::ptr::null_mut(),
        }
    }
    extern "C" fn release(bytes: *mut u8, length: usize) {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                bytes, length,
            )));
        }
    }
    let code = unsafe { lp_helper_serve(receive, release) };
    Err(format!("Helper service stopped ({code})"))
}
