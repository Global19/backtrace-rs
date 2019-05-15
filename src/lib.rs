//! A library for acquiring a backtrace at runtime
//!
//! This library is meant to supplement the `RUST_BACKTRACE=1` support of the
//! standard library by allowing an acquisition of a backtrace at runtime
//! programmatically. The backtraces generated by this library do not need to be
//! parsed, for example, and expose the functionality of multiple backend
//! implementations.
//!
//! # Implementation
//!
//! This library makes use of a number of strategies for actually acquiring a
//! backtrace. For example unix uses libgcc's libunwind bindings by default to
//! acquire a backtrace, but coresymbolication or dladdr is used on OSX to
//! acquire symbol names while linux uses gcc's libbacktrace.
//!
//! When using the default feature set of this library the "most reasonable" set
//! of defaults is chosen for the current platform, but the features activated
//! can also be controlled at a finer granularity.
//!
//! # API Principles
//!
//! This library attempts to be as flexible as possible to accommodate different
//! backend implementations of acquiring a backtrace. Consequently the currently
//! exported functions are closure-based as opposed to the likely expected
//! iterator-based versions. This is done due to limitations of the underlying
//! APIs used from the system.
//!
//! # Usage
//!
//! First, add this to your Cargo.toml
//!
//! ```toml
//! [dependencies]
//! backtrace = "0.3"
//! ```
//!
//! Next:
//!
//! ```
//! extern crate backtrace;
//!
//! fn main() {
//! # // Unsafe here so test passes on no_std.
//! # #[cfg(feature = "std")] {
//!     backtrace::trace(|frame| {
//!         let ip = frame.ip();
//!         let symbol_address = frame.symbol_address();
//!
//!         // Resolve this instruction pointer to a symbol name
//!         backtrace::resolve_frame(frame, |symbol| {
//!             if let Some(name) = symbol.name() {
//!                 // ...
//!             }
//!             if let Some(filename) = symbol.filename() {
//!                 // ...
//!             }
//!         });
//!
//!         true // keep going to the next frame
//!     });
//! }
//! # }
//! ```

#![doc(html_root_url = "https://docs.rs/backtrace")]
#![deny(missing_docs)]
#![no_std]
#![cfg_attr(target_env = "sgx", feature(sgx_platform))]
#![allow(bare_trait_objects)] // TODO: remove when updating to 2018 edition
#![allow(rust_2018_idioms)]   // TODO: remove when updating to 2018 edition

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

extern crate libc;
#[cfg(all(windows, feature = "verify-winapi"))]
extern crate winapi;

#[cfg(feature = "serde_derive")]
#[cfg_attr(feature = "serde_derive", macro_use)]
extern crate serde_derive;

#[cfg(feature = "rustc-serialize")]
extern crate rustc_serialize;

#[macro_use]
extern crate cfg_if;

extern crate rustc_demangle;

#[cfg(feature = "cpp_demangle")]
extern crate cpp_demangle;

cfg_if! {
    if #[cfg(all(feature = "gimli-symbolize", unix, target_os = "linux"))] {
        extern crate addr2line;
        extern crate findshlibs;
        extern crate memmap;
    }
}

pub use backtrace::{trace_unsynchronized, Frame};
mod backtrace;

pub use symbolize::{resolve_unsynchronized, Symbol, SymbolName};
pub use symbolize::resolve_frame_unsynchronized;
mod symbolize;

pub use types::BytesOrWideString;
mod types;

cfg_if! {
    if #[cfg(feature = "std")] {
        pub use backtrace::trace;
        pub use symbolize::{resolve, resolve_frame};
        pub use capture::{Backtrace, BacktraceFrame, BacktraceSymbol};
        mod capture;
    }
}

#[allow(dead_code)]
struct Bomb {
    enabled: bool,
}

#[allow(dead_code)]
impl Drop for Bomb {
    fn drop(&mut self) {
        if self.enabled {
            panic!("cannot panic during the backtrace function");
        }
    }
}

#[allow(dead_code)]
#[cfg(feature = "std")]
mod lock {
    use std::boxed::Box;
    use std::cell::Cell;
    use std::sync::{Mutex, MutexGuard, Once, ONCE_INIT};

    pub struct LockGuard(Option<MutexGuard<'static, ()>>);

    static mut LOCK: *mut Mutex<()> = 0 as *mut _;
    static INIT: Once = ONCE_INIT;
    thread_local!(static LOCK_HELD: Cell<bool> = Cell::new(false));

    impl Drop for LockGuard {
        fn drop(&mut self) {
            if self.0.is_some() {
                LOCK_HELD.with(|slot| {
                    assert!(slot.get());
                    slot.set(false);
                });
            }
        }
    }

    pub fn lock() -> LockGuard {
        if LOCK_HELD.with(|l| l.get()) {
            return LockGuard(None);
        }
        LOCK_HELD.with(|s| s.set(true));
        unsafe {
            INIT.call_once(|| {
                LOCK = Box::into_raw(Box::new(Mutex::new(())));
            });
            LockGuard(Some((*LOCK).lock().unwrap()))
        }
    }
}

#[cfg(all(windows, feature = "dbghelp"))]
mod dbghelp;
#[cfg(windows)]
mod windows;
