# PKI view fixtures (WP-E1)

Recorded by `crates/v2xw-engine/tests/pki_view.rs` (`V2XW_PKI_FIXTURES=<this dir>`) from
one engine run: NYC Manhattan (`scenarios/phase2-manhattan.yaml`), 150 s, the default
`us-production-2025` deployment (two providers on one CTL), every third roadside unit,
four constant-position attackers, real cryptography, report shuffle 5 s, CRL cadence 20 s.

- `pki-run.jsonl`: one record per line, `{"channel", "t", "record"}`, in recording order.
  A self-sufficient slice of the run: the first and last `pki.state` and every one that
  carries a new certificate or CRL version; the `pki.flow` records (messages, stage
  stamps, SPDUs with bytes) of every case's report and revocation instances and of one
  enrolment, provisioning and report instance per provider; every `pki.case`; every
  `node.credentials` of two vehicles per provider and of every revoked vehicle; the last
  `pki.knowledge` of each entity.
  In this run every vehicle is enrolled and provisioned before the drive
  (`ScmsRun::preload`, a scratch kernel): those SPDUs are `pki.flow` records of kind
  `pdu` and flow `preload`, with no message or stage and times not on the run's clock.
- `pki-sequence.jsonl`: the same record shapes from `crates/v2xw-proto/tests/pki_view.rs`
  (one provider, two devices): a whole in-run sequence with every message and stage
  stamp: enrolment, provisioning, two real SM-MBR reports (1VInconsistent/ConstantPosition
  and Implausible/MaxSpeed, BSM evidence signed under the accused's pseudonyms), the
  linkage resolution through both LAs and the CRL issuance. Use it for the Flows tab's
  sequence diagram. Regenerated 2026-10-18 (cleanup package) after E3's record changes.

All five files were re-recorded on 2026-10-22 from 176d6f6b plus the report and PQ fix
(certificate bytes had changed since the LOP fold; vehicle reports that cannot be built as
SM-MBR uploads are no longer filed, OF-2). `pki-run.jsonl`, `pki-answers.json`,
`pki-tabs.json` and `pki-studio.json` were re-recorded again on 2026-10-23 from d0e6530b
(`pki-sequence.jsonl` came out unchanged). A roadside unit no longer files a report
(R-UNSEC) and the GNSS fix moved the detections, so the run has 2 cases (both provider-a's,
each opened on a real report shared by provider-b's MA) against 4 before, and no modelled
evidence.

- `pki.case` (both files): `reports-in` carries `evidence`, one entry per report the case
  was opened on: the SM-MBR `type`/`type_name`, `subtypes`, `verified` and `shared` (it
  crossed the MA-to-MA link), or, for a report on the modelled path, `modelled` (why) and
  the detector `checks`. `enforced` is recorded once per vehicle that installs the case's
  entry, at its install instant, with the running `installed` count and `of`; it names no
  vehicle (the MA does not learn who installed its CRL).
- `pki-answers.json`: what `inspect.pdu` (ground truth, as the opener, as a third
  party), `inspect.cert` (every certificate of the chosen vehicles) and
  `inspect.knowledge` (ground truth and the entity's own view) answer on the full run.
  `crates/v2xw-server/tests/pki.rs` checks the server answers the same from the slice.
- `pki-tabs.json` (WP-E3): what `inspect.pki` answers on the slice: `vehicles` and every
  `vehicle` in ground truth, `crls`, every decodable `crl` (ground truth and as the RA) and
  `cases` (ground truth, as provider-a's MA and LA1, as provider-b's MA). Written by
  `crates/v2xw-server/tests/pki.rs` under `V2XW_PKI_FIXTURES`, checked by it otherwise; the
  Studio's unit tests read it, and the mock engine serves it for Playwright (`VWP_PKI_TABS`).
  The MA's `pki.knowledge` carries `truth.cases` (accused and reporters per case), the
  ground truth the Cases tab shows.

- `pki-studio.json` (WP-E2): what `v2xw-server` answers the Studio's PKI view on the two
  fixtures above: `inspect.trust`, `inspect.flows` and `inspect.flow` on both, and
  `inspect.pdu` and `inspect.knowledge` in six views on the sequence. `test/pki.test.ts` and
  `e2e/pki.spec.ts` serve it in place of an engine. `crates/v2xw-server/tests/pki.rs`
  writes it (`V2XW_PKI_STUDIO=1 cargo test -p v2xw-server --test pki studio_bundle`) and
  fails when it is stale.

Record shapes: `crates/v2xw-proto/src/scms/pki_view.rs`. Stable ids: a component is
`provider/role/instance`, the SCMS Manager and electors are under `scms-manager`, a
device is `ee/<backend device id>` (`node.credentials` carries both its engine `node` and
this `device`), a flow instance `provider/flow/run`, a PDU `provider/pdu/<n>`, a
certificate its HashedId8 in lower-case hex. Regions are labelled assumptions.

Regenerate after a change to the record shapes:

```
V2XW_PKI_FIXTURES=$PWD/ui/apps/studio/test/fixtures/pki \
  cargo test -p v2xw-engine --test pki_view -- --nocapture
V2XW_PKI_FIXTURES=1 cargo test -p v2xw-server --test pki    # then pki-tabs.json
```
