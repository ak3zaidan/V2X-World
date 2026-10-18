//! WP-F2 real-world cross-check: the size of a real 1609.2 linkage CRL against Brecht et
//! al.'s design figure, "about 40 bytes per entry, about 400 KB per 10,000 entries"
//! (IEEE T-ITS 2018 §VI-F, arXiv:1802.05323; 03 §3.4, constants.md `crl_entry_bytes`).
//!
//! The CRL is built by the same `crl::crl_contents` the CRL Generator uses: series 1,
//! one LA pair (one provider), jmax 20, every revoked device's `iMax` spread over the
//! three-year preload horizon (156 i-periods), which is the grouping a real CRL sees and
//! the worst case for the per-`iMax` group overhead. Hash CRLs (series 3/4) are measured
//! the same way: one `HashedId10` and a `Time32` expiry per entry.
//!
//! * with `V2XW_CRL_OUT=<file>` the first test writes the 10,000-entry contents for
//!   `tools/realworld/crl_sizes.py` (the A0 oracle decodes and re-encodes them).

use v2xw_msg::scms_types::WireEdition;
use v2xw_sec::crl::{self, CrlBody, CrlSpec, HashRevocation, LinkageRevocation};
use v2xw_sec::linkage::{LaId, LinkageSeed};
use v2xw_sec::sig1609::coer;

fn seed(k: u32, la: u8) -> LinkageSeed {
    let mut b = [la; 16];
    b[..4].copy_from_slice(&k.to_be_bytes());
    LinkageSeed::new(b)
}

fn linkage_crl(n: u32) -> usize {
    linkage_crl_bytes(n).len()
}

fn linkage_crl_bytes(n: u32) -> Vec<u8> {
    let i_rev = 614u16;
    let individual = (0..n)
        .map(|k| LinkageRevocation {
            la1: LaId(0x0102),
            la2: LaId(0x0304),
            ls1: seed(k, 1),
            ls2: seed(k, 2),
            jmax: 20,
            i_max: i_rev + 1 + (k % 156) as u16,
        })
        .collect();
    let c = crl::crl_contents(&CrlSpec {
        series: 1,
        craca: [7; 8],
        issue_date: 700_000_000,
        next_crl: 700_086_400,
        priority: None,
        delta: false,
        body: CrlBody::Linkage {
            i_rev,
            index_within_i: 0,
            individual,
        },
    });
    crl::check_crl(&c, WireEdition::Ieee2022).expect("a valid CRL");
    coer(&c).expect("encodes")
}

fn hash_crl(n: u32) -> usize {
    let entries = (0..n)
        .map(|k| {
            let mut id = [0u8; 10];
            id[..4].copy_from_slice(&k.to_be_bytes());
            HashRevocation {
                id,
                expiry: 700_000_000 + k,
            }
        })
        .collect();
    let c = crl::crl_contents(&CrlSpec {
        series: 3,
        craca: [7; 8],
        issue_date: 700_000_000,
        next_crl: 700_086_400,
        priority: None,
        delta: false,
        body: CrlBody::Hash { serial: 1, entries },
    });
    coer(&c).expect("encodes").len()
}

#[test]
fn a_linkage_crl_costs_about_brechts_40_bytes_per_entry() {
    let empty = linkage_crl(0);
    let sizes: Vec<(u32, usize)> = [1, 10, 100, 1_000, 10_000]
        .iter()
        .map(|&n| (n, linkage_crl(n)))
        .collect();
    eprintln!("linkage CRL contents: empty {empty} B, {sizes:?}");
    if let Ok(out) = std::env::var("V2XW_CRL_OUT") {
        // For tools/realworld/crl_sizes.py: the oracle decodes and re-encodes it.
        std::fs::write(out, linkage_crl_bytes(10_000)).unwrap();
    }
    let ten_k = sizes.last().unwrap().1;
    let per_entry = (ten_k - empty) as f64 / 10_000.0;
    eprintln!("per entry at 10,000: {per_entry:.2} B; Brecht ≈ 40 B, ≈ 400 KB per 10,000");
    // Two 16-byte seeds are the floor; the IMaxGroup overhead is spread over the entries
    // that share an iMax. The real encoder lands under Brecht's estimate, not over it.
    assert!(
        (33.0..=40.0).contains(&per_entry),
        "{per_entry} B per entry"
    );
    assert!(ten_k <= 400_000, "{ten_k} B for 10,000 entries");
    // Pinned as of WP-F2 (04-validation-report.md §5).
    assert_eq!(empty, 27);
    assert_eq!(ten_k, 330_818);
}

#[test]
fn a_hash_crl_costs_fifteen_bytes_per_entry() {
    let empty = hash_crl(0);
    let ten_k = hash_crl(10_000);
    eprintln!("hash CRL contents: empty {empty} B, 10,000 entries {ten_k} B");
    // HashedId10 + Time32 + the extension preamble; the count's length grows by one byte.
    assert_eq!(ten_k - empty, 10_000 * 15 + 1, "{ten_k} - {empty}");
}
