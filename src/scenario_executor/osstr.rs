use std::ffi::OsString;

use base64::Engine as _;
use rhai::{Engine, NativeCallContext};

use crate::scenario_executor::utils1::SimpleErr;

use super::utils1::RhResult;

//@ Decode base64 buffer and interpret using Rust's `OsString::from_encoded_bytes_unchecked`.
//@ This format is not intended to be portable and is mostly for internal use within Websocat.
fn osstr_base64_unchecked_encoded_bytes(ctx: NativeCallContext, x: String) -> RhResult<OsString> {
    let Ok(buf) = base64::prelude::BASE64_STANDARD.decode(x) else {
        return Err(ctx.err("Invalid base64"));
    };
    unsafe { Ok(OsString::from_encoded_bytes_unchecked(buf)) }
}

//@ On Unix or WASI platforms, decode base64 buffer and convert it OsString.
fn osstr_base64_unix_bytes(ctx: NativeCallContext, x: String) -> RhResult<OsString> {
    let Ok(buf) = base64::prelude::BASE64_STANDARD.decode(x) else {
        return Err(ctx.err("Invalid base64"));
    };

    #[cfg(any(unix, target_os = "wasi"))]
    {
        #[cfg(unix)]
        use std::os::unix::ffi::OsStringExt;
        #[cfg(all(not(unix), target_os = "wasi"))]
        use std::os::wasi::ffi::OsStringExt;

        Ok(OsString::from_vec(buf))
    }
    #[cfg(not(any(unix, target_os = "wasi")))]
    {
        Err(ctx.err("osstr_base64_unix_bytes function is not supported on this platform"))
    }
}

#[allow(unused)]
//@ On Windows, decode base64 buffer and convert it OsString.
fn osstr_base64_windows_utf16le(ctx: NativeCallContext, x: String) -> RhResult<OsString> {
    let Ok(buf) = base64::prelude::BASE64_STANDARD.decode(x) else {
        return Err(ctx.err("Invalid base64"));
    };
    if buf.len() % 2 != 0 {
        return Err(ctx.err("Odd number of bytes in base64 buffer"));
    }
    let buf = Vec::from_iter(
        buf.chunks_exact(2)
            .map(|c| u16::from_le_bytes(c.try_into().unwrap())),
    );

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        Ok(OsString::from_wide(&buf[..]))
    }
    #[cfg(not(windows))]
    {
        Err(ctx.err("osstr_base64_windows_utf16le function is not supported on this platform"))
    }
}

//@ Convert a usual UTF-8 string to an OsString
fn osstr_str(x: String) -> OsString {
    x.into()
}

pub fn register(engine: &mut Engine) {
    engine.register_fn(
        "osstr_base64_unchecked_encoded_bytes",
        osstr_base64_unchecked_encoded_bytes,
    );
    engine.register_fn("osstr_base64_unix_bytes", osstr_base64_unix_bytes);
    engine.register_fn("osstr_base64_windows_utf16le", osstr_base64_windows_utf16le);
    engine.register_fn("osstr_str", osstr_str);
}
