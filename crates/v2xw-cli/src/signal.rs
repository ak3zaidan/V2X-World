//! A checkpoint on request: Ctrl-C, a `kill`, or the terminal closing.
//!
//! A run with `--checkpoint-every` installs handlers for SIGINT, SIGTERM and SIGHUP that only
//! set a flag. The run looks at the flag once a simulated second, between two events, and
//! when it is set writes a checkpoint there, finishes the recording segment it was writing
//! and stops; `v2xw resume` carries on from it. A second signal while the first is being
//! honoured exits at once, as an interrupt always did.
//!
//! A laptop that is closed sends no signal: the process is suspended with the machine and
//! carries on when it wakes, so nothing is needed for that case.

use std::sync::atomic::{AtomicBool, Ordering};

static REQUESTED: AtomicBool = AtomicBool::new(false);

#[allow(unsafe_code)]
extern "C" fn on_signal(_: libc::c_int) {
    // Only async-signal-safe operations here: an atomic swap and `_exit`.
    if REQUESTED.swap(true, Ordering::SeqCst) {
        // SAFETY: `_exit` is async-signal-safe and ends the process without unwinding.
        unsafe { libc::_exit(130) }
    }
}

/// Installs the handlers. Idempotent.
#[allow(unsafe_code)]
pub fn install() {
    #[cfg(unix)]
    for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        // SAFETY: the handler is an `extern "C"` function that only touches an atomic and
        // calls `_exit`, both async-signal-safe.
        unsafe {
            libc::signal(sig, on_signal as *const () as libc::sighandler_t);
        }
    }
}

/// Whether a stop has been asked for since the handlers were installed.
#[must_use]
pub fn requested() -> bool {
    REQUESTED.load(Ordering::SeqCst)
}

/// Asks for a stop, as a signal would. For tests.
pub fn request() {
    REQUESTED.store(true, Ordering::SeqCst);
}

/// Clears a request. For tests.
pub fn clear() {
    REQUESTED.store(false, Ordering::SeqCst);
}
