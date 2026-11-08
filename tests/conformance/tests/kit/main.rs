//! The three harnesses the conformance kit offers, driven against this repository.
//!
//! `tests/vwp/` is the wire checklist. This is the rest of the plug-in conformance suite: the
//! plug-in suite, the golden determinism harness and the interface firewall. Each of the
//! three is a library in `src/`, because each has users outside this repository — a
//! contributor's own crate, a CI job, a second implementation — and each is exercised here
//! both on a correct subject and on a deliberately broken one.
//!
//! The broken subjects are not decoration. A harness nobody has seen go red is a harness
//! that reads as evidence while proving nothing, and this project has had four checks that
//! were exactly that.

mod firewall_suite;
mod golden_suite;
mod plugin_suite;
