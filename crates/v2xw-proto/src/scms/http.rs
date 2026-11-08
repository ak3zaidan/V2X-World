//! HTTP/1.1 request and response framing on the EE-RA exchanges.
//!
//! The device reaches the RA over HTTPS ([CAMP122] §6.1-6.3): a REST-like service
//! whose payloads are binary OER, `Content-Type: application/octet-stream`, at
//! `https://<SERVER>:8892/<PATH>`. TLS records and handshakes are charged by the transport
//! (`v2xw-radio` `ip_transport.rs`, `v2xw-engine` `backend.rs`); this module charges what
//! rides inside them: the request line and headers of each request, the status line and
//! headers of each answer, and the payload as it is actually carried:
//!
//! - a `POST` carries the SPDU as its body (`Content-Length` bytes);
//! - a `GET` download carries no body: the `SecuredAuthenticatedDownloadRequest` travels
//!   Base64-encoded in the `Download-Req` header ([CAMP122] Table 79), so its
//!   bytes are charged once, at their Base64 length, never also as a body;
//! - the policy and chain-file checks are conditional `GET`s (`If-None-Match`,
//!   [CAMP122] SCMS-544), answered `200` with the file or `304` without one.
//!
//! **ASSUMPTION** (scms-constants.md `http_header_set`): the minimal header set an HTTP/1.1
//! client and server must or conventionally send. A request carries `Host`, plus
//! `Content-Type` and `Content-Length` when it has a body; an answer carries `Date`
//! (RFC 9110 §6.6.1, an origin server with a clock must send it) and `Content-Length`,
//! plus `Content-Type` when it has a body. No `User-Agent`, `Accept`, `Connection` or
//! `Server`. The paths are [CAMP122] Table 79's; the IEEE 1609.2.1 v3 path strings are
//! paywalled and **UNCONFIRMED**, so the CAMP/CV-Pilot ones stand for them. The
//! `Host` is the RA's certificate name and port, which the server's TLS certificate must
//! match ([SMP] §3.2).

use crate::sizes::{SizeProvenance, WireSize};

/// The protocol version on every request and status line.
pub const HTTP_VERSION: &str = "HTTP/1.1";
/// The RA's service port ([CAMP122] §6.2.1, §6.3.1).
pub const RA_PORT: u16 = 8892;
/// The MA's and CRL store's service port ([CAMP122] §6.2.1).
pub const MA_PORT: u16 = 8894;
/// Every payload's media type ([CAMP122] §6.1).
pub const CONTENT_TYPE: &str = "application/octet-stream";
/// An RFC 9110 §5.6.7 IMF-fixdate (`Sun, 06 Nov 1994 08:49:37 GMT`): always 29 bytes.
pub const IMF_FIXDATE_BYTES: u32 = 29;
/// The `Download-Req` header ([CAMP122] Table 79).
pub const DOWNLOAD_REQ: &str = "Download-Req";

/// [CAMP122] Table 79 paths.
pub mod path {
    /// Request pseudonym batch provisioning (POST).
    pub const PROVISION_PSEUDONYM_BATCH: &str = "/provision-pseudonym-certificate-batch";
    /// Download a pseudonym batch (GET, `Download-Req`).
    pub const DOWNLOAD_BATCH: &str = "/download/batch";
    /// Download the `.info` next-batch time (GET, `Download-Req`).
    pub const DOWNLOAD_INFO: &str = "/download/info";
    /// Download the local policy file (GET, `If-None-Match`).
    pub const LOCAL_POLICY: &str = "/download/policy/local";
    /// Download the local certificate chain file (GET, `If-None-Match`).
    pub const LOCAL_CHAIN: &str = "/download/local-certificate-chain";
    /// Submit a misbehaviour report (POST, body and `Download-Req`).
    pub const MISBEHAVIOR_REPORT: &str = "/process-misbehavior-report";
    /// Request an application certificate (POST, [CAMP122] Table 79's `provision-` form).
    pub const PROVISION_APP_CERT: &str = "/provision-application-certificate";
    /// Download an application certificate (GET, `Download-Req`).
    pub const DOWNLOAD_CERT: &str = "/download/certificate";
    /// Re-enrolment request (POST).
    pub const REENROL_REQUEST: &str = "/ee-re-enrollment-request";
    /// Re-enrolment download (GET, `Download-Req`).
    pub const REENROL_DOWNLOAD: &str = "/ee-re-enrollment-download";
}

/// A request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
}

impl Method {
    /// The token on the request line.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
        }
    }
}

/// A status the RA answers with (RFC 9110 §15 reason phrases).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// `200 OK`.
    Ok,
    /// `206 Partial Content`: a resumed (`Range`) batch download ([SMP] §4.3.2).
    PartialContent,
    /// `304 Not Modified`: the file named by `If-None-Match` is current (SCMS-544).
    NotModified,
    /// `412 Precondition Failed`: the `If-Modified-Since` copy is current ([SMP] §4.3.1).
    PreconditionFailed,
    /// `500 Internal Server Error`: the production RA's only error ([CAMP122] §6.3.1).
    InternalServerError,
}

impl Status {
    /// The status code.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Status::Ok => 200,
            Status::PartialContent => 206,
            Status::NotModified => 304,
            Status::PreconditionFailed => 412,
            Status::InternalServerError => 500,
        }
    }

    /// The reason phrase.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::PartialContent => "Partial Content",
            Status::NotModified => "Not Modified",
            Status::PreconditionFailed => "Precondition Failed",
            Status::InternalServerError => "Internal Server Error",
        }
    }
}

/// One header: its name and the byte length of its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// The field name.
    pub name: &'static str,
    /// The field value's length, bytes.
    pub value_len: u32,
}

impl Header {
    /// A header whose value is `value`.
    #[must_use]
    pub fn text(name: &'static str, value: &str) -> Header {
        Header {
            name,
            value_len: len32(value),
        }
    }

    /// A header whose value is the decimal number `n`.
    #[must_use]
    pub fn number(name: &'static str, n: u64) -> Header {
        Header {
            name,
            value_len: decimal_len(n),
        }
    }

    /// `name: value\r\n`.
    #[must_use]
    pub fn bytes(self) -> u32 {
        len32(self.name) + 2 + self.value_len + 2
    }
}

fn len32(s: &str) -> u32 {
    u32::try_from(s.len()).unwrap_or(u32::MAX)
}

/// The number of decimal digits of `n`.
#[must_use]
pub fn decimal_len(n: u64) -> u32 {
    n.checked_ilog10().map_or(1, |d| d + 1)
}

/// The RFC 4648 §4 Base64 length of `n` bytes, padded: `4·⌈n/3⌉` (within 2 bytes of
/// `⌈4n/3⌉`; the padded form is what a standard encoder emits).
#[must_use]
pub const fn base64_len(n: u32) -> u32 {
    n.div_ceil(3).saturating_mul(4)
}

/// A request's bytes: `METHOD path HTTP/1.1\r\n`, each header, the blank line, the body.
/// The caller names every header, `Content-Length` included.
#[must_use]
pub fn request_bytes(method: Method, path: &str, headers: &[Header], body_len: u32) -> u32 {
    let line = len32(method.token()) + 1 + len32(path) + 1 + len32(HTTP_VERSION) + 2;
    line + headers.iter().map(|h| h.bytes()).sum::<u32>() + 2 + body_len
}

/// An answer's bytes: `HTTP/1.1 code reason\r\n`, each header, the blank line, the body.
#[must_use]
pub fn response_bytes(status: Status, headers: &[Header], body_len: u32) -> u32 {
    let line = len32(HTTP_VERSION) + 1 + 3 + 1 + len32(status.reason()) + 2;
    line + headers.iter().map(|h| h.bytes()).sum::<u32>() + 2 + body_len
}

/// The `Host` header for `server` on `port`.
#[must_use]
pub fn host(server: &str, port: u16) -> Header {
    Header {
        name: "Host",
        value_len: len32(server) + 1 + decimal_len(u64::from(port)),
    }
}

/// A `POST` of `body_len` bytes of OER to `path`.
#[must_use]
pub fn post_bytes(host: Header, path: &str, body_len: u32) -> u32 {
    request_bytes(
        Method::Post,
        path,
        &[
            host,
            Header::text("Content-Type", CONTENT_TYPE),
            Header::number("Content-Length", u64::from(body_len)),
        ],
        body_len,
    )
}

/// A `GET` download whose `n`-byte `SecuredAuthenticatedDownloadRequest` rides in the
/// `Download-Req` header, Base64-encoded; `range_from` adds `Range: bytes=<from>-` for a
/// resumed download ([SMP] §4.3.2).
#[must_use]
pub fn download_bytes(host: Header, path: &str, n: u32, range_from: Option<u32>) -> u32 {
    let range = range_from.map(|from| Header {
        name: "Range",
        value_len: len32("bytes=") + decimal_len(u64::from(from)) + 1,
    });
    let dl = Header {
        name: DOWNLOAD_REQ,
        value_len: base64_len(n),
    };
    match range {
        Some(r) => request_bytes(Method::Get, path, &[host, dl, r], 0),
        None => request_bytes(Method::Get, path, &[host, dl], 0),
    }
}

/// A conditional `GET` of a policy or chain file; `etag_len` is the `If-None-Match`
/// value's length, `None` when the device holds no copy.
#[must_use]
pub fn conditional_get_bytes(host: Header, path: &str, etag_len: Option<u32>) -> u32 {
    match etag_len {
        Some(n) => request_bytes(
            Method::Get,
            path,
            &[
                host,
                Header {
                    name: "If-None-Match",
                    value_len: n,
                },
            ],
            0,
        ),
        None => request_bytes(Method::Get, path, &[host], 0),
    }
}

/// The misbehaviour-report upload ([SM-MBR] §4, Table 79): a `POST` whose body is the
/// `SecuredMisbehaviorReport` and whose `Download-Req` header carries the
/// enrolment-signed authentication (`download_req_len` bytes before Base64). The header's
/// bytes are counted once, as Base64, not again as body.
#[must_use]
pub fn upload_bytes(host: Header, body_len: u32, download_req_len: u32) -> u32 {
    request_bytes(
        Method::Post,
        path::MISBEHAVIOR_REPORT,
        &[
            host,
            Header::text("Content-Type", CONTENT_TYPE),
            Header::number("Content-Length", u64::from(body_len)),
            Header {
                name: DOWNLOAD_REQ,
                value_len: base64_len(download_req_len),
            },
        ],
        body_len,
    )
}

/// The RA's answer with `body_len` bytes: `Date` and `Content-Length`, plus
/// `Content-Type` when there is a body, plus `extra` (a `206`'s `Content-Range`). A `304`
/// carries `Date` only (RFC 9110 §15.4.5: no content, `Content-Length` optional).
#[must_use]
pub fn answer_bytes(status: Status, body_len: u32, extra: &[Header]) -> u32 {
    let date = Header {
        name: "Date",
        value_len: IMF_FIXDATE_BYTES,
    };
    let mut headers = vec![date];
    if status != Status::NotModified {
        if body_len > 0 {
            headers.push(Header::text("Content-Type", CONTENT_TYPE));
        }
        headers.push(Header::number("Content-Length", u64::from(body_len)));
    }
    headers.extend_from_slice(extra);
    response_bytes(status, &headers, body_len)
}

/// A `206`'s `Content-Range: bytes <from>-<last>/<total>` for the remainder of a
/// `total`-byte file from byte `from`.
#[must_use]
pub fn content_range(from: u32, total: u32) -> Header {
    let last = total.saturating_sub(1);
    Header {
        name: "Content-Range",
        value_len: len32("bytes ")
            + decimal_len(u64::from(from))
            + 1
            + decimal_len(u64::from(last))
            + 1
            + decimal_len(u64::from(total)),
    }
}

/// `size` with its byte count replaced by `bytes`, its provenance kept: the framing is
/// derived arithmetic around the payload, which stays the real encoder's, cited or
/// parameter size it was.
#[must_use]
pub fn reframed(size: WireSize, bytes: u32) -> WireSize {
    match size.provenance() {
        SizeProvenance::RealEncoder { what } => WireSize::real(bytes, what),
        SizeProvenance::Cited { citation } => WireSize::cited(bytes, citation),
        SizeProvenance::Derived {
            expression,
            citation,
        } => WireSize::derived(bytes, expression, citation),
        SizeProvenance::Parameter { param } => WireSize::parameter(bytes, param),
    }
}
