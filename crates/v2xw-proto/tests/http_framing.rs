//! The EE-RA exchanges are charged their HTTP/1.1 framing ([CAMP122] §6.2-6.3,
//! Table 79). Each expected count is built here as the literal request or
//! answer text, with this file's own Base64 encoder, so the framing in `scms/http.rs`
//! is checked against what an HTTP client would put on the wire.

mod common;

use common::{DEVICE_A, provisioned};
use v2xw_proto::scms::http::{self, Status};
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::SpduKind;
use v2xw_proto::scms::run::ScmsRun;

/// RFC 4648 §4 Base64, padded.
fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= c.len() {
                s.push(A[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn len(s: &str) -> u32 {
    u32::try_from(s.len()).unwrap()
}

fn run_with_bytes() -> ScmsRun {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    let mut run = ScmsRun::new(p).expect("certificates encode");
    run.add_device(DEVICE_A);
    run
}

fn host_line(run: &ScmsRun) -> String {
    format!("Host: ra.{}:8892\r\n", run.state.gov.config.provider)
}

#[test]
fn the_base64_of_a_download_req_is_four_bytes_per_three_padded() {
    for n in [0u32, 1, 2, 3, 4, 314, 315, 316] {
        let b = vec![0xA5u8; n as usize];
        assert_eq!(http::base64_len(n), len(&base64(&b)), "{n} bytes");
    }
    assert_eq!(http::base64_len(315), 420, "the upload's Download-Req");
}

/// The accept test: a download request is charged its Table 79 path, its headers and
/// the Base64 of its `Download-Req`, and the SPDU is not charged again as a body.
#[test]
fn a_download_request_is_charged_its_path_headers_and_base64() {
    let mut run = run_with_bytes();
    provisioned(&mut run, DEVICE_A, 0, 1, 3);
    let st = &run.state;
    assert!(st.pdu_rejections.is_empty(), "{:?}", st.pdu_rejections);
    let host = host_line(&run);
    let mut seen = 0;
    for step in &run.kernel.steps {
        if !matches!(
            step.step,
            "batch-download-request" | "batch-download-request-proxied"
        ) {
            continue;
        }
        let p = st
            .pdus
            .iter()
            .find(|p| {
                p.kind == SpduKind::EeRaDownloadRequest
                    && p.from == step.from.index()
                    && p.to == step.to.index()
            })
            .expect("the download request was recorded");
        let raw = p.bytes.as_ref().expect("bytes kept").bytes();
        let text = format!(
            "GET /download/batch HTTP/1.1\r\n{host}Download-Req: {}\r\n\r\n",
            base64(raw)
        );
        assert_eq!(step.bytes, len(&text), "{}", step.step);
        assert!(step.bytes > p.len, "framed, not the bare SPDU");
        seen += 1;
    }
    assert_eq!(seen, 2, "device to LOP and LOP to RA");
}

/// A `POST` carries the SPDU as its body, with `Content-Type` and `Content-Length`; the
/// RA's answer carries `Date`, `Content-Type` and `Content-Length`.
/// The RA's acknowledgement goes to the LOP over the backend network, and from the LOP to the
/// device over the air: only the proxied leg is cellular. Fault: `provisioning-ack` sent `Transport::CellularUu` -> red.
#[test]
fn only_the_proxied_ack_is_cellular() {
    use v2xw_proto::net::Transport;
    let mut run = common::deployment(1);
    common::provisioned(&mut run, common::DEVICE_A, 0, 1, 2);
    let leg = |name: &str| {
        run.kernel
            .steps
            .iter()
            .find(|s| s.step == name)
            .unwrap_or_else(|| panic!("no {name}"))
            .transport
    };
    assert_eq!(leg("provisioning-ack"), Transport::BackendNet);
    assert_eq!(leg("provisioning-ack-proxied"), Transport::CellularUu);
}

#[test]
fn a_provisioning_request_and_its_ack_are_framed_posts_and_answers() {
    let mut run = run_with_bytes();
    provisioned(&mut run, DEVICE_A, 0, 1, 3);
    let st = &run.state;
    let host = host_line(&run);
    let spdu = |kind: SpduKind| {
        st.pdus
            .iter()
            .find(|p| p.kind == kind)
            .map(|p| p.len)
            .expect("recorded")
    };
    let step = |name: &str| {
        run.kernel
            .steps
            .iter()
            .find(|s| s.step == name)
            .map(|s| s.bytes)
            .expect("sent")
    };
    let n = spdu(SpduKind::EeRaCertRequest);
    let post = format!(
        "POST /provision-pseudonym-certificate-batch HTTP/1.1\r\n{host}\
         Content-Type: application/octet-stream\r\nContent-Length: {n}\r\n\r\n"
    );
    assert_eq!(step("provisioning-request"), len(&post) + n);
    assert_eq!(step("provisioning-request-proxied"), len(&post) + n);
    let a = spdu(SpduKind::RaEeCertAck);
    let answer = format!(
        "HTTP/1.1 200 OK\r\nDate: Tue, 06 Jan 2026 09:00:00 GMT\r\n\
         Content-Type: application/octet-stream\r\nContent-Length: {a}\r\n\r\n"
    );
    assert_eq!(step("provisioning-ack-proxied"), len(&answer) + a);
}

/// The file check is two conditional `GET`s, answered `200` with a file or `304`.
#[test]
fn the_file_check_is_two_conditional_gets() {
    let mut run = run_with_bytes();
    provisioned(&mut run, DEVICE_A, 0, 1, 3);
    let host = host_line(&run);
    let req = run
        .kernel
        .steps
        .iter()
        .find(|s| s.step == "file-request")
        .expect("the session's file check");
    // The device holds the policy and chain files the DCM gave it.
    let dev = &run.state.devices[&DEVICE_A];
    let (l, c) = (
        dev.trust.lpf.as_ref().map(|f| f.version),
        dev.trust.lccf_version,
    );
    let g = run.state.gov.gpf.version;
    let tag = |t: Option<String>| t.map_or(String::new(), |t| format!("If-None-Match: {t}\r\n"));
    let text = format!(
        "GET /download/policy/local HTTP/1.1\r\n{host}{}\r\n\
         GET /download/local-certificate-chain HTTP/1.1\r\n{host}{}\r\n",
        tag(l.map(|l| format!("\"local_policy_{g}_{l}.oer\""))),
        tag(c.map(|c| format!("\"local_certificate_chain_{c}.oer\""))),
    );
    assert_eq!(req.bytes, len(&text));
    let not_modified = "HTTP/1.1 304 Not Modified\r\nDate: Tue, 06 Jan 2026 09:00:00 GMT\r\n\r\n";
    assert_eq!(
        http::answer_bytes(Status::NotModified, 0, &[]),
        len(not_modified)
    );
}

/// The 412 answer of a current CRL poll is the engine's `CRL_NOT_MODIFIED_BYTES` (92,
/// `phase2.rs`), so B9b can take it from here; a `500` refusal and a `206` resume.
#[test]
fn answers_have_their_status_line_and_headers() {
    let date = "Date: Tue, 06 Jan 2026 09:00:00 GMT\r\n";
    let t = format!("HTTP/1.1 412 Precondition Failed\r\n{date}Content-Length: 0\r\n\r\n");
    assert_eq!(len(&t), 92);
    assert_eq!(http::answer_bytes(Status::PreconditionFailed, 0, &[]), 92);
    let t = format!(
        "HTTP/1.1 500 Internal Server Error\r\n{date}\
         Content-Type: application/octet-stream\r\nContent-Length: 102\r\n\r\n"
    );
    assert_eq!(
        http::answer_bytes(Status::InternalServerError, 102, &[]),
        len(&t) + 102
    );
    let t = format!(
        "HTTP/1.1 206 Partial Content\r\n{date}\
         Content-Type: application/octet-stream\r\nContent-Length: 600\r\n\
         Content-Range: bytes 400-999/1000\r\n\r\n"
    );
    assert_eq!(
        http::answer_bytes(
            Status::PartialContent,
            600,
            &[http::content_range(400, 1000)]
        ),
        len(&t) + 600
    );
}

/// The misbehaviour upload's 315 B `Download-Req` ([SM-MBR] §4, `mbr.rs`
/// `REFERENCE_UPLOAD_BYTES`) rides in its header as Base64 and is not counted again.
#[test]
fn the_report_upload_counts_its_download_req_once() {
    let host = "Host: ra.provider-a:8892\r\n";
    let t = format!(
        "POST /process-misbehavior-report HTTP/1.1\r\n{host}\
         Content-Type: application/octet-stream\r\nContent-Length: 804\r\n\
         Download-Req: {}\r\n\r\n",
        base64(&[0u8; 315])
    );
    let h = http::host("ra.provider-a", http::RA_PORT);
    assert_eq!(http::upload_bytes(h, 804, 315), len(&t) + 804);
    assert_eq!(
        804 + 315,
        v2xw_proto::scms::mbr::REFERENCE_UPLOAD_BYTES,
        "the bare upload"
    );
}

/// A roadside unit's application-certificate exchanges are HTTPS too: each step is its SPDU
/// plus the literal request or answer. Fault: the raw PDU (385,
/// 292, 319 and 454 B equal their raw PDUs, where a vehicle's 319-octet request reads 502).
#[test]
fn an_rsu_application_certificate_exchange_is_framed() {
    use common::DEVICE_B;
    use v2xw_proto::scms::appcert::AppCertKind;
    let mut run = run_with_bytes();
    run.add_rsu(DEVICE_B);
    run.enrol(DEVICE_B);
    run.run().expect("enrolment runs");
    let t0 = run.kernel.now();
    run.start_app_certs(DEVICE_B, AppCertKind::RsuApplication, t0);
    let horizon = run.state.params.period_start(2);
    run.run_until(horizon).expect("runs");
    let host = host_line(&run);
    let spdu = |kind: SpduKind| {
        run.state
            .pdus
            .iter()
            .find(|p| p.kind == kind && (p.from == DEVICE_B.index() || p.to == DEVICE_B.index()))
            .map(|p| p.len)
            .expect("recorded")
    };
    let step = |name: &str| {
        run.kernel
            .steps
            .iter()
            .find(|s| s.step == name)
            .map(|s| s.bytes)
            .expect("sent")
    };
    let n = spdu(SpduKind::EeRaCertRequest);
    let post = format!(
        "POST /provision-application-certificate HTTP/1.1\r\n{host}\
         Content-Type: application/octet-stream\r\nContent-Length: {n}\r\n\r\n"
    );
    assert_eq!(step("app-cert-request"), len(&post) + n);
    let a = spdu(SpduKind::RaEeCertAck);
    let answer = format!(
        "HTTP/1.1 200 OK\r\nDate: Tue, 06 Jan 2026 09:00:00 GMT\r\n\
         Content-Type: application/octet-stream\r\nContent-Length: {a}\r\n\r\n"
    );
    assert_eq!(step("app-cert-ack"), len(&answer) + a);
    let d = spdu(SpduKind::EeRaDownloadRequest);
    let get = format!(
        "GET /download/certificate HTTP/1.1\r\n{host}Download-Req: {}\r\n\r\n",
        base64(&vec![0u8; d as usize])
    );
    assert_eq!(step("app-cert-download-request"), len(&get));
    let f = spdu(SpduKind::AcaEeCertResponsePrivate);
    let file = format!(
        "HTTP/1.1 200 OK\r\nDate: Tue, 06 Jan 2026 09:00:00 GMT\r\n\
         Content-Type: application/octet-stream\r\nContent-Length: {f}\r\n\r\n"
    );
    assert_eq!(step("app-cert-download"), len(&file) + f);
}
