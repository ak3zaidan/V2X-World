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
  stamp: enrolment, provisioning, two reports, the linkage resolution through both LAs
  and the CRL issuance. Use it for the Flows tab's sequence diagram.
- `pki-answers.json`: what `inspect.pdu` (ground truth, as the opener, as a third
  party), `inspect.cert` (every certificate of the chosen vehicles) and
  `inspect.knowledge` (ground truth and the entity's own view) answer on the full run.
  `crates/v2xw-server/tests/pki.rs` checks the server answers the same from the slice.

Record shapes: `crates/v2xw-proto/src/scms/pki_view.rs`. Stable ids: a component is
`provider/role/instance`, the SCMS Manager and electors are under `scms-manager`, a
device is `ee/<backend device id>` (`node.credentials` carries both its engine `node` and
this `device`), a flow instance `provider/flow/run`, a PDU `provider/pdu/<n>`, a
certificate its HashedId8 in lower-case hex. Regions are labelled assumptions.

Regenerate after a change to the record shapes:

```
V2XW_PKI_FIXTURES=$PWD/ui/apps/studio/test/fixtures/pki \
  cargo test -p v2xw-engine --test pki_view -- --nocapture
```
