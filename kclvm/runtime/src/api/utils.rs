//! Copyright The KCL Authors. All rights reserved.

use std::os::raw::c_char;

use crate::{Context, ValueRef};

/// New a mutable raw pointer.
/// Safety: The caller must ensure that `ctx` lives longer than the returned pointer
/// and that the pointer is properly deallocated by calling `free_mut_ptr`.
pub fn new_mut_ptr(ctx: &mut Context, x: ValueRef) -> *mut ValueRef {
    let ptr = Box::into_raw(Box::new(x));
    // Store the object pointer address to
    // drop it it after execution is complete
    ctx.objects.insert(ptr as usize);
    ptr
}

/// Free a mutable raw pointer.
/// Safety: The caller must ensure `p` is a valid pointer obtained from `new_mut_ptr`.
pub(crate) fn free_mut_ptr<T>(p: *mut T) {
    if !p.is_null() {
        unsafe {
            drop(Box::from_raw(p));
        }
    }
}

/// Convert a const raw pointer to a immutable borrow.
/// Safety: The caller must ensure that `p` is valid for the lifetime `'a`.
pub(crate) fn ptr_as_ref<'a, T>(p: *const T) -> &'a T {
    assert!(!p.is_null());
    unsafe { &*p }
}

/// Convert a mutable raw pointer to a mutable borrow.
/// Safety: The caller must ensure that `p` is valid for the lifetime `'a`.
pub(crate) fn mut_ptr_as_ref<'a, T>(p: *mut T) -> &'a mut T {
    assert!(!p.is_null());

    unsafe { &mut *p }
}

/// Convert a C str pointer to a Rust &str.
/// Safety: The caller must ensure that `s` is a valid null-terminated C string.
pub(crate) fn c2str<'a>(s: *const c_char) -> &'a str {
    let s = unsafe { std::ffi::CStr::from_ptr(s) }.to_str().unwrap();
    s
}

pub fn assert_panic<F: FnOnce() + std::panic::UnwindSafe>(msg: &str, func: F) {
    match std::panic::catch_unwind(func) {
        Ok(_v) => {
            panic!("not panic, expect={msg}");
        }
        Err(e) => match e.downcast::<String>() {
            Ok(v) => {
                let got = v.to_string();
                assert!(got.contains(msg), "expect={msg}, got={got}");
            }
            Err(e) => match e.downcast::<&str>() {
                Ok(v) => {
                    let got = v.to_string();
                    assert!(got.contains(msg), "expect={msg}, got={got}");
                }
                _ => unreachable!(),
            },
        },
    };
}
