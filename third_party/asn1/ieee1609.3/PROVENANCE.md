# IEEE 1609.3 ASN.1 modules (WSA, WEE, WSM): provenance

Fetched 2026-10-27 for rule gap U5 (`docs/design/scms-real/rule-gap-specs-2026-10-27.md` §2.1)
and WP-19 piece P19.1 (`docs/design/frameworks/wp16-24-build-plans.md`). `crates/v2xw-msg/build.rs`
does not read this directory: the hand-written UPER codec `crates/v2xw-msg/src/wsa.rs` is
written from it (2026-10-28, WP-19 P19.1; `rasn-compiler` cannot compile the WEE
`Extension` information-object class), and `tools/oracle/asn1/ieee1609.3/` holds the
oracle's byte-identical copy. Do not edit the files;
they are byte-identical to the upstream copies (CRLF line endings, ASCII, no BOM).

## Source

- Mirror: the Certification Operating Council's TCI ASN.1 repository,
  <https://github.com/certificationoperatingcouncil/TCI_ASN1/tree/master/TCI%20Interface/ASN1/1609dot3>
  (the copy `docs/design/research/R4-messages-envelopes.md` §A.8 names).
- Pinned commit: `63d82d87976bf56e51ac3ef9f121b01d16274059` (2017-05-21, "Reverted range
  of ecwMax back to 0..15"). This is the last commit to touch `1609dot3/`. Repository
  `master` head on the fetch date: `764674975b147226c0ab807565c2479fccaa2cf8`
  (2019-03-25). Each file's git blob id at the pinned commit equals the one GitHub lists
  for `master`, so the two are the same text.
- Raw URL pattern:
  `https://raw.githubusercontent.com/certificationoperatingcouncil/TCI_ASN1/63d82d87976bf56e51ac3ef9f121b01d16274059/TCI%20Interface/ASN1/1609dot3/<FILE>`

| File | ASN.1 module (OID arc) | Bytes | sha256 | git blob |
|---|---|---:|---|---|
| `wsa.asn` | `IEEE-1609-3-WSA` `{... wave-stds(1609) dot3(3) wsa(3) version0(0)}` | 6 125 | `5f2f0a304506237bb43707869290c5ecd12ef8432aea41a87e0265d2a90fd93e` | `0edcd10defc77868a0601943f05149a9d0b57b54` |
| `wee.asn` | `IEEE-1609-3-WEE` `{... dot3(3) wee(1) version0(0)}` | 4 939 | `60ebab7e5bb79fd0746e815cd0d0fb6d2925be92de8f52b42f72c812ed5eb88c` | `4b76974fc97a5a7ab023f13a5f3c0e1363f2deb6` |
| `wsm.asn` | `IEEE-1609-3-WSM` `{... dot3(3) wsm(2) version0(0)}` | 9 436 | `3bf63da6aee4a02e8e09fc127eca84cb317c26ea2fd6b5a6d0c585b3ee64fb2a` | `4b94f3805d8ef783d542e2a876e2340c9bbbcc9e` |
| `17419_CITSapplMgmtIDs_v1.asn` | `CITSapplMgmtIDs` `{iso(1) standard(0) cits-applMgmt(17419) ids(1) version1(1)}` (ISO 17419, imported by WSA and WSM for `VarLengthNumber`) | 958 | `d357fb4018fd43ef8eb8057eb6d95c27c641d44f244e19308ddd76f4b237bad5` | `86186bb5c544c33b88bd2e9803ad8cae846d86a1` |

`wsm.asn` and the ISO 17419 module were not asked for by name. They are here because WSA
cannot compile without `CITSapplMgmtIDs`, and the WSMP header (the frame a WSA rides in)
is in `wsm.asn`.

Edition: the module headers say `version0` and `c-rsvAdvPrtVersionNo ::= 3` (WSA protocol
version 3). That is the IEEE 1609.3-2016 WSA. It is the edition USDOT RSU Specification
v4.1 (2017) requires (Req_586, "IEEE 1609.3-2016 Annex H.1"). Whether 1609.3-2020 changed
these modules is not known: the 2020 text is paywalled and no public 2020 ASN.1 copy was
found. Treat the edition as 2016 until one is.

## Licence

**None stated.** The GitHub repository has no `LICENSE` file and the API reports no
licence (checked 2026-10-27). The module text is IEEE copyright, transcribed from the
IEEE 1609.3 standard for the TCI test interface. These files are committed for the
same reason as the 1609.2 and 1609.2.1 copies in `../etsi/PROVENANCE.md` §3.5–3.7: they
are the public interoperability definitions every deployed WSA is encoded with. Anyone who
republishes this repository should confirm the terms with IEEE (or the Certification
Operating Council) and not rely on this note. No licence file has been invented.

## Encoding rule (read from the module headers)

All three IEEE modules declare `DEFINITIONS AUTOMATIC TAGS` and carry the same header
comment: "Proper syntax and correct mapping of **UPER** on the intended binary presentation
as specified in the main body of this document was validated with the OSS ASN.1 Studio
tool." So the WSA, its extensions and the WSMP header are **UPER** (unaligned PER,
ITU-T X.691), not COER. `wsm.asn` adds: `VarLengthNumber` "is the p-encoded PSID".

Cross-check, 2026-10-27, with asn1tools 0.169 (UPER) on these four files, outside the
build. A WSA with two ServiceInfos (PSID 0x23 and the IPv6 Routing PSID), two ChannelInfos
and a RoutingAdvertisement encodes to 73 bytes and round-trips. Its bytes match the
USDOT RSU Specification v4.1 Appendix D example WSA octet for octet where that example
gives a value:

| Field | Appendix D (pp. 118–120) | UPER from these modules |
|---|---|---|
| version + option indicator | `0x3F` (version 3, all four body options present) | `0x37` with header extensions absent, so `0x3F` with them present |
| WSA ID + content count | `0x11` | `0x11` |
| IPv6 Routing PSID | `0x10 20 40 7E` (shown p-encoded in the registry as 0pEF-FF-FF-FE) | `ef ff ff fe` |
| channel index octet | `0x09` / `0x11` (index 1 / 2, option indicator 1) | `0x08` / `0x10` with no options, so `0x09` / `0x11` with them |
| data rate | `0x0C` (fixed, 6 Mb/s) | `0x0c` |
| transmit power 20 dBm | `0x14` | `0x94`: UPER offsets `INTEGER(-128..127)` by +128. Appendix D writes the plain value. The ASN.1 governs; the build's oracle should flag this row and nothing else |

The OER codec of asn1tools cannot compile `wee.asn` (it has no `OpenType` for the
`Extension` class). That does not matter, since OER is not the rule here.

## Related public sources (small extracts only, not cached as files)

- USDOT, *DSRC Roadside Unit (RSU) Specifications Document v4.1* (2017),
  <https://cflsmartroads.com/projects/CV_Testing/USDOT%20RSU%20Specification%204%201_Final_R1.pdf>:
  Req_586 (WSA security profile per IEEE 1609.3-2016 Annex H.1), Req_587 (WSA on the CCH),
  Req_588 (WSA in time slot 0), Req_570–573 (what the WSA lists); Appendix D (example WSA).
  App. D, Repeat Rate row: "32 … indicates the WSA is repeated 50 times per 5 seconds".
  0x32 = 50, which confirms RepeatRate = **WSAs per 5 s**.
- FHWA-JPO-20-804, *RSU Lessons Learned and Best Practices* (May 2020), p. 4: NYC RSUs give
  "SCMS access so that OBUs can topoff … This service is advertised through the RSU using
  wave service announcements (WSAs)". p. 15 (Design 04): WSA transmission "was not defined
  in RSU Specification v4.1 … different vendors applied different practices." Neither
  page names a PSID.
- IEEE RA PSID registry, <https://standards.ieee.org/products-programs/regauth/psid/public/>
  (read 2026-10-27): 0x23 (0p23) "WAVE security management", IEEE Std 1609.2;
  0x87 (0p80-07) "WAVE Service Advertisement", IEEE Std 1609.3; 0x10 20 40 7E
  (0pEF-FF-FF-FE) "IPv6 routing", IEEE Std 1609.3; 0x26 "Misbehavior reporting for common
  applications", SAE J3268.
