//! Pinned regression digests for the SCMS runs (WP-F3).
//!
//! One JSON file per case beside this module: the record digest, the frame digest and the
//! record count one run of that case produced. A case whose file is missing **fails**: the
//! first failure is where a human reads the numbers and decides they are right, because a
//! check that wrote its own oracle on first run would agree with it for ever. Re-bless
//! after a deliberate change to the engine's output with
//!
//! ```text
//! V2XW_BLESS_SCMS_DIGESTS=1 cargo test -p v2xw-engine --test phase2 --test scms_scale
//! ```
//!
//! and say in the commit message what changed and why.
//!
//! Included by `#[path]` from the test binaries that pin a case; each uses only some of it.
#![allow(dead_code)]

use std::path::PathBuf;

use serde_json::{Value, json};

/// The environment variable that writes the records instead of comparing them.
pub const BLESS_ENV: &str = "V2XW_BLESS_SCMS_DIGESTS";

/// What one run of a case produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    /// SHA-256 over every record, `DigestRecorder::digest_hex`.
    pub record_digest: String,
    /// SHA-256 over every VWP frame, `DigestRecorder::frame_digest_hex`.
    pub frame_digest: String,
    /// How many records the run wrote.
    pub records: u64,
}

impl Pinned {
    fn to_json(&self, case: &str) -> Value {
        json!({
            "case": case,
            "record_digest": self.record_digest,
            "frame_digest": self.frame_digest,
            "records": self.records,
        })
    }
}

fn path(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("regression")
        .join(format!("{case}.json"))
}

/// Compares `got` with the committed record of `case`, or writes it under [`BLESS_ENV`].
///
/// # Panics
/// When the record is missing, unreadable or differs.
pub fn check(case: &str, got: &Pinned) {
    let file = path(case);
    if std::env::var_os(BLESS_ENV).is_some() {
        let text = serde_json::to_string_pretty(&got.to_json(case)).expect("serialises") + "\n";
        std::fs::write(&file, text).expect("the regression record is writable");
        eprintln!("blessed {}", file.display());
        return;
    }
    let Ok(text) = std::fs::read_to_string(&file) else {
        panic!(
            "no regression record for `{case}` at {}.\nRead the run's numbers and, if they \
             are right, commit the record: {BLESS_ENV}=1 cargo test -p v2xw-engine \
             <the test>.\nThis fails rather than writing the file itself, because a check \
             that invents its own oracle on the first run agrees with it for ever.",
            file.display()
        )
    };
    let want: Value = serde_json::from_str(&text).expect("the regression record is JSON");
    let field = |k: &str| want[k].clone();
    let pinned = Pinned {
        record_digest: field("record_digest").as_str().unwrap_or_default().to_string(),
        frame_digest: field("frame_digest").as_str().unwrap_or_default().to_string(),
        records: field("records").as_u64().unwrap_or_default(),
    };
    assert_eq!(pinned.record_digest.len(), 64, "{}: no record digest", file.display());
    assert!(
        pinned == *got,
        "`{case}` no longer reproduces its committed regression digest ({}):\n  \
         committed {pinned:?}\n  this run  {got:?}\nIf the change to the engine's output is \
         deliberate, re-bless with {BLESS_ENV}=1 and explain it in the commit message.",
        file.display()
    );
}
