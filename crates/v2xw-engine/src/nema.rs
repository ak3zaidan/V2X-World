//! The NEMA dual-ring eight-phase controller (`traffic.signals.control: nema`, WP-17).
//!
//! Re-implemented from the FHWA Signal Timing Manual 2nd ed. (NCHRP Report 812, 2015) and
//! the MUTCD 2009; NEMA TS 2-2016 is paywalled, so the STM is the working text. SUMO's
//! NEMA controller was read for its documented parameter names and behaviour only, never
//! for code. The build plan is `docs/design/frameworks/wp16-24-build-plans.md` §WP-17.
//!
//! # Pieces
//!
//! * [`assign`] — NEMA phase numbers for a junction's movements, from its geometry
//!   (P17.3), with the junctions that do not map counted rather than guessed.
//!
//! The controller is not wired yet: the loader refuses `control: nema`
//! (`scenario::validate::nema_checks`) until P17.5, so nothing here runs in a scenario.

pub mod assign;

pub use assign::{AssignedMovement, Assignment, MovementKind, Unmapped, assign_phases};
