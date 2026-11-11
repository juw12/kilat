//! The C interface, shared by the DLL (Python, C, C#...) and the WebAssembly module.
//!
//! One `Session` runs one program at a time:
//!
//! ```text
//! s = kilat_session_new()
//! kilat_start(s, source, len, mode, fuel, memory)      mode 0 run, 1 check only, 2 run and print the last value
//! loop:
//!     r = kilat_step(s, slice)                         0 done, 1 host call, 2 more, 3 error
//!     read kilat_text(s, 0) / kilat_text_len(s, 0), then kilat_clear_output(s)
//!     if r == 1: read texts 2 (name) and 3 (payload), then kilat_resume(s, ok, text, len)
//! kilat_session_free(s)
//! ```
//!
//! Numbers cross the boundary as `f64` so that JavaScript can pass them without BigInt.
//! This file holds all the `unsafe` code of the project.

use crate::vm::{Status, Vm};
use crate::{build, resolve_import, VERSION};
use std::rc::Rc;

const DEFAULT_FUEL: u64 = 200_000_000;
const DEFAULT_MEMORY: usize = 64 << 20;
const OUTPUT_LIMIT: u64 = 4 << 20;

pub struct Session {
    files: Vec<(String, String)>,
    seed: Option<u64>,
    vm: Option<Vm>,
    error: String,
    error_kind: u32,
}

fn fresh_seed() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
        let here = &nanos as *const u64 as u64;
        nanos ^ here.rotate_left(32) ^ 0x9E37_79B9_7F4A_7C15
    }
    #[cfg(target_arch = "wasm32")]
    {
        0x9E37_79B9_7F4A_7C15
    }
}

/// Build on a thread with a deep stack where threads exist; the checker and compiler recurse.
fn build_program(
    files: Vec<(String, String)>,
    source: String,
    print_last: bool,
) -> Result<crate::compile::Program, String> {
    let work = move || {
        let mut loader = |path: &str, importer: &str| -> Result<(String, String), String> {
            let name = resolve_import(importer, path);
            match files.iter().find(|(known, _)| *known == name || known == path) {
                Some((known, text)) => Ok((known.clone(), text.clone())),
                None => Err(format!("`{}` was not found", name)),
            }
        };
        build("main.kilat", &source, &mut loader, print_last)
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let spawned = std::thread::Builder::new().stack_size(64 << 20).spawn(work);
        match spawned.map(|handle| handle.join()) {
            Ok(Ok(result)) => result,
            _ => Err("error: internal: the compiler stopped unexpectedly".to_string()),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        work()
    }
}

/// Run `f` on the session; a panic inside the runtime becomes an error instead of crossing the boundary.
fn guard<T>(s: *mut Session, fallback: T, f: impl FnOnce(&mut Session) -> T) -> T {
    if s.is_null() {
        return fallback;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let session = unsafe { &mut *s };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(session))) {
            Ok(value) => value,
            Err(_) => {
                let session = unsafe { &mut *s };
                session.vm = None;
                session.error = "error: internal: the Kilat runtime stopped unexpectedly".to_string();
                session.error_kind = 2;
                fallback
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        f(unsafe { &mut *s })
    }
}

unsafe fn text_arg(ptr: *const u8, len: usize) -> String {
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned()
}

fn text_of(session: &Session, which: u32) -> &str {
    match (which, &session.vm) {
        (0, Some(vm)) => &vm.core.out,
        (1, _) => &session.error,
        (2, Some(vm)) => &vm.host_name,
        (3, Some(vm)) => &vm.host_payload,
        (4, _) => VERSION,
        _ => "",
    }
}

/// Memory for passing text in. Free it with `kilat_dealloc` and the same length.
#[no_mangle]
pub extern "C" fn kilat_alloc(len: usize) -> *mut u8 {
    let mut buffer: Vec<u8> = Vec::with_capacity(len.max(1));
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

/// # Safety
/// `ptr` must come from `kilat_alloc(len)` and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn kilat_dealloc(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
    }
}

#[no_mangle]
pub extern "C" fn kilat_session_new() -> *mut Session {
    Box::into_raw(Box::new(Session {
        files: Vec::new(),
        seed: None,
        vm: None,
        error: String::new(),
        error_kind: 0,
    }))
}

/// # Safety
/// `s` must come from `kilat_session_new` and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn kilat_session_free(s: *mut Session) {
    if !s.is_null() {
        drop(Box::from_raw(s));
    }
}

/// Make a file available to `import "name"`.
///
/// # Safety
/// The pointers must be valid for their lengths.
#[no_mangle]
pub unsafe extern "C" fn kilat_add_file(
    s: *mut Session,
    name: *const u8,
    name_len: usize,
    text: *const u8,
    text_len: usize,
) {
    let name = text_arg(name, name_len).replace('\\', "/");
    let text = text_arg(text, text_len);
    guard(s, (), |session| {
        session.files.retain(|(known, _)| *known != name);
        session.files.push((name, text));
    });
}

/// Fix the random seed (for repeatable runs). Without it, native builds seed from the clock.
#[no_mangle]
pub extern "C" fn kilat_set_seed(s: *mut Session, seed: f64) {
    guard(s, (), |session| {
        session.seed = Some((seed.abs() as u64) ^ seed.to_bits().rotate_left(17));
    });
}

/// Compile `src`. Returns 0 when ready (then call `kilat_step`) or 3 on a compile error.
/// `fuel` and `mem` (bytes) of 0 mean the defaults.
///
/// # Safety
/// `src` must be valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn kilat_start(
    s: *mut Session,
    src: *const u8,
    len: usize,
    mode: u32,
    fuel: f64,
    mem: f64,
) -> u32 {
    let source = text_arg(src, len);
    guard(s, 3, |session| {
        session.vm = None;
        session.error.clear();
        session.error_kind = 0;
        let program = match build_program(session.files.clone(), source, mode == 2) {
            Ok(program) => program,
            Err(text) => {
                session.error = text;
                session.error_kind = 1;
                return 3;
            }
        };
        if mode == 1 {
            return 0;
        }
        let fuel = if fuel >= 1.0 { fuel as u64 } else { DEFAULT_FUEL };
        let memory = if mem >= 1.0 { mem as usize } else { DEFAULT_MEMORY };
        let seed = session.seed.unwrap_or_else(fresh_seed);
        let mut vm = Vm::new(Rc::new(program), seed, memory, fuel);
        vm.core.out_limit = OUTPUT_LIMIT;
        session.vm = Some(vm);
        0
    })
}

/// Run for at most `slice` units of fuel. Returns 0 done, 1 host call, 2 more to do, 3 error.
#[no_mangle]
pub extern "C" fn kilat_step(s: *mut Session, slice: f64) -> u32 {
    guard(s, 3, |session| {
        let vm = match session.vm.as_mut() {
            Some(vm) => vm,
            None => return if session.error_kind == 0 { 0 } else { 3 },
        };
        let slice = if slice >= 1.0 { slice as u64 } else { 1_000_000 };
        match vm.run(slice) {
            Status::Done => 0,
            Status::Host => 1,
            Status::More => 2,
            Status::Error => {
                session.error = vm.error.clone();
                session.error_kind = vm.error_kind as u32;
                3
            }
        }
    })
}

/// Answer the pending host call with `Ok(text)` (ok != 0) or `Err(text)`.
///
/// # Safety
/// `text` must be valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn kilat_resume(s: *mut Session, ok: u32, text: *const u8, len: usize) {
    let text = text_arg(text, len);
    guard(s, (), |session| {
        if let Some(vm) = session.vm.as_mut() {
            vm.resume(ok != 0, &text);
        }
    });
}

/// Text owned by the session: 0 output, 1 error, 2 host call name, 3 host call payload, 4 version.
/// It is UTF-8 without a terminator (ask `kilat_text_len`) and valid until the next call.
#[no_mangle]
pub extern "C" fn kilat_text(s: *mut Session, which: u32) -> *const u8 {
    guard(s, std::ptr::null(), |session| text_of(session, which).as_ptr())
}

#[no_mangle]
pub extern "C" fn kilat_text_len(s: *mut Session, which: u32) -> usize {
    guard(s, 0, |session| text_of(session, which).len())
}

#[no_mangle]
pub extern "C" fn kilat_clear_output(s: *mut Session) {
    guard(s, (), |session| {
        if let Some(vm) = session.vm.as_mut() {
            vm.core.out.clear();
        }
    });
}

/// 0 none, 1 compile error, 2 runtime error, 3 out of fuel, 4 out of memory.
#[no_mangle]
pub extern "C" fn kilat_error_kind(s: *mut Session) -> u32 {
    guard(s, 0, |session| session.error_kind)
}

#[no_mangle]
pub extern "C" fn kilat_fuel_used(s: *mut Session) -> f64 {
    guard(s, 0.0, |session| session.vm.as_ref().map(|vm| vm.fuel_used as f64).unwrap_or(0.0))
}
