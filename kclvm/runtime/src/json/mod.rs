//! Copyright The KCL Authors. All rights reserved.

use crate::*;

// data, sort_keys=False, indent=None, ignore_private=False, ignore_none=False

#[no_mangle]
#[runtime_fn]
pub extern "C" fn kclvm_json_encode(
    ctx: *mut kclvm_context_t,
    args: *const kclvm_value_ref_t,
    kwargs: *const kclvm_value_ref_t,
) -> *const kclvm_value_ref_t {
    let args = ptr_as_ref(args);
    let ctx = mut_ptr_as_ref(ctx);
    let kwargs = ptr_as_ref(kwargs);

    if let Some(arg0) = args.arg_i(0) {
        let s = ValueRef::str(
            arg0.to_json_string_with_option(&kwargs_to_opts(kwargs))
                .as_ref(),
        );
        return s.into_raw(ctx);
    }
    panic!("encode() missing 1 required positional argument: 'value'")
}

#[no_mangle]
#[runtime_fn]
pub extern "C" fn kclvm_json_decode(
    ctx: *mut kclvm_context_t,
    args: *const kclvm_value_ref_t,
    _kwargs: *const kclvm_value_ref_t,
) -> *const kclvm_value_ref_t {
    let args = ptr_as_ref(args);
    let ctx = mut_ptr_as_ref(ctx);

    if let Some(arg0) = args.arg_i(0) {
        match ValueRef::from_json(ctx, arg0.as_str().as_ref()) {
            Ok(x) => return x.into_raw(ctx),
            Err(err) => panic!("{}", err),
        }
    }
    panic!("decode() missing 1 required positional argument: 'value'")
}

#[no_mangle]
#[runtime_fn]
pub extern "C" fn kclvm_json_validate(
    ctx: *mut kclvm_context_t,
    args: *const kclvm_value_ref_t,
    _kwargs: *const kclvm_value_ref_t,
) -> *const kclvm_value_ref_t {
    let args = ptr_as_ref(args);
    let ctx = mut_ptr_as_ref(ctx);

    if let Some(arg0) = args.arg_i(0) {
        match ValueRef::from_json(ctx, arg0.as_str().as_ref()) {
            Ok(_) => return kclvm_value_True(ctx),
            Err(_) => return kclvm_value_False(ctx),
        }
    }
    panic!("validate() missing 1 required positional argument: 'value'")
}

#[no_mangle]
#[runtime_fn]
pub extern "C" fn kclvm_json_dump_to_file(
    _ctx: *mut kclvm_context_t,
    args: *const kclvm_value_ref_t,
    kwargs: *const kclvm_value_ref_t,
) -> *const kclvm_value_ref_t {
    let args = ptr_as_ref(args);
    let kwargs = ptr_as_ref(kwargs);

    if let Some(data) = args.arg_i(0) {
        if let Some(filename) = args.arg_i(0) {
            let filename = filename.as_str();
            let json = data.to_json_string_with_option(&kwargs_to_opts(kwargs));
            std::fs::write(filename, json).expect("Unable to write file");
        }
    }
    panic!("dump_to_file() missing 2 required positional arguments: 'data' and 'filename'")
}

fn kwargs_to_opts(kwargs: &ValueRef) -> JsonEncodeOptions {
    let mut opts = JsonEncodeOptions::default();
    if let Some(sort_keys) = kwargs.kwarg_bool("sort_keys", None) {
        opts.sort_keys = sort_keys;
    }
    if let Some(indent) = kwargs.kwarg_int("indent", None) {
        opts.indent = indent;
    }
    if let Some(ignore_private) = kwargs.kwarg_bool("ignore_private", None) {
        opts.ignore_private = ignore_private;
    }
    if let Some(ignore_none) = kwargs.kwarg_bool("ignore_none", None) {
        opts.ignore_none = ignore_none;
    }
    opts
}
