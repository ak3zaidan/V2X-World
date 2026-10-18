//! WP-D3: the engine, node and security constants on the SCMS path agree with
//! `docs/design/scms-real/constants.md`, row by row, and every row carries a status and a
//! source. The card's own parameters are checked by `v2xw-proto/tests/model_card.rs`.

use std::collections::BTreeMap;

use v2xw_engine::backend;
use v2xw_engine::phase2;
use v2xw_proto::scms::{deployment, governance, params::ThresholdDeployment};

/// `constants.md`'s rows: name → (default cell, status).
fn constants_md() -> BTreeMap<String, (String, String)> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/design/scms-real/constants.md"
    );
    let text = std::fs::read_to_string(path).expect("constants.md is readable");
    let mut rows = BTreeMap::new();
    for line in text.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 8 {
            continue;
        }
        let name = cells[1].trim_matches('`').to_string();
        let status = cells[4].to_string();
        assert!(
            ["CITED", "DERIVED", "MEASURED", "ASSUMPTION"].contains(&status.as_str()),
            "`{name}`: status `{status}`"
        );
        assert!(cells[6].len() > 10, "`{name}` gives no source or reason");
        assert!(
            rows.insert(name.clone(), (cells[2].to_string(), status))
                .is_none(),
            "`{name}` listed twice"
        );
    }
    rows
}

/// The code's value of every engine-side row, with the status the row must carry.
fn code_values() -> Vec<(&'static str, f64, &'static str)> {
    let (h13, s13, f13) = backend::SCMS_TLS13_FLIGHTS;
    let (h12, s12, f12) = backend::SCMS_TLS12_FLIGHTS;
    let p2pcd = v2xw_sec::p2pcd::P2pcdParams::RECOMMENDED;
    let deployment::LatencyModel::Region {
        fibre_fraction,
        per_link_overhead,
    } = deployment::LatencyModel::DEFAULT_REGION
    else {
        panic!("the default latency model is the region model");
    };
    let app = v2xw_proto::scms::run::appcert::AppCertParams::default();
    let th = ThresholdDeployment::default();
    let s = |d: v2xw_core::time::Duration| d.as_nanos() as f64 / 1e9;
    let ms = |d: v2xw_core::time::Duration| d.as_nanos() as f64 / 1e6;
    vec![
        ("relay_range_m", backend::RELAY_RANGE_M, "ASSUMPTION"),
        (
            "backhaul_latency_ms",
            backend::BACKHAUL_LATENCY_MS,
            "ASSUMPTION",
        ),
        (
            "backhaul_capacity_mbps",
            backend::BACKHAUL_CAPACITY_MBPS,
            "ASSUMPTION",
        ),
        (
            "internet_one_way_ms",
            backend::INTERNET_ONE_WAY_MS,
            "DERIVED",
        ),
        ("wired_rto_ms", backend::WIRED_RTO_MS, "CITED"),
        ("reconnect_s", backend::RECONNECT_S, "CITED"),
        ("mbms_carousel_s", backend::MBMS_CAROUSEL_S, "ASSUMPTION"),
        ("tls_session_max_s", s(backend::TLS_SESSION_MAX), "CITED"),
        ("tls13_client_hello_bytes", f64::from(h13), "MEASURED"),
        ("tls13_server_flight_bytes", f64::from(s13), "MEASURED"),
        ("tls13_client_finish_bytes", f64::from(f13), "MEASURED"),
        ("tls12_client_hello_bytes", f64::from(h12), "MEASURED"),
        ("tls12_server_flight_bytes", f64::from(s12), "MEASURED"),
        ("tls12_client_finish_bytes", f64::from(f12), "MEASURED"),
        (
            "crl_fetch_interval_s",
            phase2::CRL_FETCH_INTERVAL_S,
            "ASSUMPTION",
        ),
        (
            "crl_broadcast_interval_s",
            phase2::CRL_BROADCAST_INTERVAL_S,
            "CITED",
        ),
        ("report_bytes", f64::from(phase2::report_bytes()), "DERIVED"),
        (
            "report_store_quota_bytes",
            phase2::REPORT_STORE_QUOTA_BYTES as f64,
            "ASSUMPTION",
        ),
        ("report_interval_s", phase2::REPORT_INTERVAL_S, "ASSUMPTION"),
        (
            "overdue_crl_tolerance_s",
            f64::from(v2xw_node::rxval::OVERDUE_CRL_TOLERANCE_S),
            "CITED",
        ),
        (
            "p2pcd_max_response_backoff_ms",
            ms(p2pcd.max_response_backoff),
            "CITED",
        ),
        (
            "p2pcd_response_count_threshold",
            f64::from(p2pcd.response_count_threshold),
            "CITED",
        ),
        (
            "p2pcd_max_active_responses",
            p2pcd.max_active_responses as f64,
            "ASSUMPTION",
        ),
        (
            "rxval_ee_cache",
            v2xw_node::rxval::EE_CACHE as f64,
            "ASSUMPTION",
        ),
        ("us_electors", f64::from(deployment::US_ELECTORS), "CITED"),
        ("us_quorum", f64::from(deployment::US_QUORUM), "CITED"),
        (
            "elector_start_time32",
            f64::from(governance::ELECTOR_START_TIME32),
            "CITED",
        ),
        (
            "genesis_backdate_s",
            f64::from(governance::GENESIS_BACKDATE_S),
            "ASSUMPTION",
        ),
        (
            "default_t0_unix_s",
            governance::DEFAULT_T0_UNIX_S as f64,
            "DERIVED",
        ),
        ("fibre_fraction", fibre_fraction, "ASSUMPTION"),
        ("per_link_overhead_ms", ms(per_link_overhead), "ASSUMPTION"),
        (
            "appcert_backhaul_latency_ms",
            ms(app.backhaul_latency),
            "ASSUMPTION",
        ),
        ("appcert_id_period_s", s(app.id_period), "ASSUMPTION"),
        ("appcert_max_gen_ahead_s", s(app.max_gen_ahead), "CITED"),
        ("appcert_retry_wait_s", s(app.retry_wait), "CITED"),
        (
            "threshold_round_timeout_ms",
            ms(th.round_timeout),
            "ASSUMPTION",
        ),
        (
            "threshold_member_link_latency_ms",
            ms(th.member_link_latency),
            "ASSUMPTION",
        ),
    ]
}

/// The mismatches between the code values and the rows; a fraction like 2/3 is printed
/// to four places, so a row matches within half a unit of its last printed digit.
fn mismatches(
    rows: &BTreeMap<String, (String, String)>,
    code: &[(&str, f64, &str)],
) -> Vec<String> {
    let mut out = Vec::new();
    for &(name, value, status) in code {
        let Some((cell, st)) = rows.get(name) else {
            out.push(format!("`{name}` is not in constants.md"));
            continue;
        };
        let decimals = cell.split('.').nth(1).map_or(0, str::len);
        let tol = 0.5 * 10f64.powi(-i32::try_from(decimals).unwrap_or(0)) + 1e-9;
        match cell.parse::<f64>() {
            Ok(v) if (v - value).abs() <= tol => {}
            _ => out.push(format!(
                "`{name}`: constants.md says {cell}, the code {value}"
            )),
        }
        if st != status {
            out.push(format!(
                "`{name}`: constants.md says {st}, expected {status}"
            ));
        }
    }
    out
}

#[test]
fn constants_md_matches_the_engine_constants() {
    let problems = mismatches(&constants_md(), &code_values());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_engine_constants_check_can_fail() {
    // The injected faults: a moved constant, a relabelled status and a missing row.
    let rows = constants_md();
    let mut code = code_values();
    code[0].1 = 301.0;
    code[1].2 = "CITED";
    code.push(("not_a_row", 1.0, "CITED"));
    let found = mismatches(&rows, &code);
    assert_eq!(found.len(), 3, "{found:?}");
}

#[test]
fn the_scms_carrier_uses_the_measured_handshake() {
    use v2xw_radio::ip_transport::TlsVersion;
    let c13 = backend::scms_tcp_config(TlsVersion::Tls13);
    assert_eq!(
        (c13.client_hello, c13.server_flight, c13.client_finish),
        backend::SCMS_TLS13_FLIGHTS
    );
    let c12 = backend::scms_tcp_config(TlsVersion::Tls12);
    assert_eq!(
        (c12.client_hello, c12.server_flight, c12.client_finish),
        backend::SCMS_TLS12_FLIGHTS
    );
    assert_eq!(c12.tls, TlsVersion::Tls12);
}
