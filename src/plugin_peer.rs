#![allow(unused)]

extern crate libc;

use std::ffi::{CStr,CString};
use libc::{c_char, c_void, size_t};

use super::Specifier;
use super::ConstructParams;

#[repr(i8)]
pub enum Status {
    Complete = 1,
    RetryLater = 0,
    Error = -1,
}

#[repr(C)]
pub struct OverlayPluginPeerClass {
    name: [c_char; 16],
    construct: extern "C" fn(peer_to_be_filled_in:*mut OverlayPluginPeer)->Status,
}

/// Instances of this type are provided by Websocat
#[repr(C)]
pub struct NativePeer {
    /// This field is used by Websocat
    usrdata: *mut c_void,
    read: extern "C" fn(p: *mut NativePeer, buffer: *mut u8, maxbytes: size_t, processed_bytes: *mut size_t)->Status,
    write: extern "C" fn(p: *mut NativePeer, buffer: *const u8, maxbytes: size_t, processed_bytes: *mut size_t)->Status,
}

/// You need to fill in function pointers to appropriate values in OverlayPluginPeerClass::construct
#[repr(C)]
pub struct OverlayPluginPeer {
    /// Use those usrdata fields as you see fit
    usrdata0: *mut c_void,
    usrdata1: *mut c_void,
    usrdata2: u64,
    usrdata3: u32,
    usrdata4: u32,

    /// This field comes pre-filled when OverlayPluginPeerClass::construct is called
    native_peer: NativePeer,

    /// You need to fill this and other function pointers
    read: extern "C" fn(p: *mut OverlayPluginPeer, buffer: *mut u8, maxbytes: size_t, processed_bytes: *mut size_t)->Status,
    write: extern "C" fn(p: *mut OverlayPluginPeer, buffer: *const u8, maxbytes: size_t, processed_bytes: *mut size_t)->Status,

    destroy: extern "C" fn(p :*mut OverlayPluginPeer) -> Status,
}

/// Call this function to register a class
#[no_mangle]
pub extern "C" fn websocat_register_overlay_class(c: &'static OverlayPluginPeerClass) {
    unimplemented!()
}

/*

#[no_mangle]
pub extern "C" fn websocat_new_peer(wsline:*const c_char) -> *mut NewPeerInWaiting {
    let wsline = todo!();
    let specifier = super::specparse::spec(wsline);
    if let Ok(spec) = specifier {
        let cp : ConstructParams = todo!();
        let peer_f = spec.construct(cp).get_only_first_conn(cp.left_to_right);
        
        let x = peer_f;
        let x = Box::new(x);
        let x = Box::into_raw(x);
        let x : *mut c_void = x as *mut c_void;
        let p = Box::new(NewPeerInWaiting {
            usrdata0: std::ptr::null_mut(),
            usrdata1: x,
            usrdata2: 0,
            usrdata3: 0,
            usrdata4: 0,

            finish_construction: todo!(),
        });
        Box::into_raw(p)
    } else {
        std::ptr::null_mut()
    }
}

/// Only for peers created by `websocat_new_peer`, not for peers that 
/// was filled in by `WebsocatClass::construct`.
#[no_mangle]
pub extern "C" fn websocat_delete_peer(p:*mut Peer) {
    unsafe {
        let mut p = Box::from_raw(p);
        if p.usrdata3 == 0xDEADBEEF {
            error!("websocat_delete_peer: Double free?");
            return;
        }
        if p.usrdata1 == std::ptr::null_mut() || p.usrdata0 != std::ptr::null_mut() || p.usrdata2 != 0 || p.usrdata3 != 0 || p.usrdata4 != 0 {
            error!("websocat_delete_peer: Are you sure it is originates from websocat_new_peer?");
            return;
        }
        (&mut p.usrdata3 as *mut u32).write_volatile(0xDEADBEEF);
        let x = p.usrdata1;
        (&mut p.usrdata1 as *mut *mut c_void).write_volatile(std::ptr::null_mut());
        let x = x as *mut std::rc::Rc<dyn Specifier>;
        let x = Box::from_raw(x);
    }
}
*/