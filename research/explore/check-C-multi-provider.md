# Check C: Two providers, one fleet (cross-provider MA cooperation and CTL change propagation)

Blind prior-art and feasibility check, 2026-10-18. Sources: the corpus (catalog ids), primary documents downloaded and grepped (page numbers are PDF pages), web, and engine code read with grep and sed only. Search limits: OpenAlex and Semantic Scholar refused anonymous queries that day (budget exhausted or HTTP 429). Crossref, arXiv, WebSearch and WebFetch were used instead. Yoshizawa 2022 could only be read from its corpus summary (abstract basis), because no open full text was found.

## Verdict

**Partly done: the problem is named, but nothing has been measured.** At least five sources name the problem qualitatively or architecturally, listed below. None of them measures split evidence, cooperation delay, policy shopping across providers, or how fast a CTL change reaches the fleet in V2X. The measurement is open.

But three of the pitch's factual premises need correcting (see "Pitch corrections"). The most interesting result, that the issuing provider's MA is the only one that can decide, follows from the architecture, so the engine would mostly confirm it. The CTL half adds little that the engine can show. A narrower reframing is recommended at the end.

## 1. Prior art

### Standards and policy texts (they define the gap, they do not measure it)

- **SCMS Manager Provider Requirements v1.1 (9 Aug 2024), §2.2, p9.** "The Provider SHALL agree to the cooperative sharing of misbehavior reports with other SCMS Manager Providers." Every provider "shall implement a Misbehavior Authority (MA) component". No deadline and no MA-to-MA format is given. Source: https://www.scmsmanager.org/wp-content/uploads/2024/10/SCMS-Manager-Provider-Requirements-v1_1.pdf
- **Same document, §1.1, p4.** It lists the "SCMS Manager Certified Trust List (CTL) Update Process [in development – to be published in 2024]". The publications page has no such document as of 2026-10-18 (https://www.scmsmanager.org/publications/, fetched today). The repo's own notes agree (docs/design/scms-real/01-deployment-and-governance.md §1.2).
- **5GAA TR "Credential Management Supporting V2X Commercial Deployments" (14 Jul 2025, 95 pp, contains the proposed CP).**
  - (13), p21: "An SCMS Provider **shall** cooperate with MAs."
  - (24)–(26), p22: "An MA **should** cooperate with all SCMS Providers published on the CTL", "should provide linking information for reported ACs", "should support the end entity revocation requests from other MAs".
  - Timings: (47), p33: the CTL is published "within 15 days after an addition", and within 24 h for a removal. (49), p33: CRLs are published 24 h after a status change. (261), p58: the ACA processes a revocation "within 5 days". p90–91: the CTL inclusion process includes a one-week last call.
  - Source: https://5gaa.org/content/uploads/2025/07/credential-management-supporting-v2x-commercial-deployments.pdf
- **ETSI TS 103 759 V2.2.1 (2026-01), p10.** "Multiple instances of the Misbehaviour Authority component may exist in the system." p18: reports go to "the MA responsible for … a given ITS-AID". Coordination between MAs is not specified. Source: https://etsi.org/deliver/etsi_ts/103700_103799/103759/02.02.01_60/ts_103759v020201p.pdf
- **5GAA MBD white papers (2022 and 2024 MBD-Op).** A grep of both PDFs found no treatment of several MAs or providers. Sources: https://5gaa.org/content/uploads/2022/07/5GAA-Misbehaviour-detection-Final.pdf and https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf

### Academic and grey literature (architectural or qualitative only)

- **Brecht et al. 2018, SCMS design (brecht2018security, arXiv 1802.05323).**
  - p2: "While there is only one SCMS, components that are not central can have multiple instances." The MA is an "intrinsically central" component.
  - p24: once electors endorse a new root CA, "EEs can immediately begin to trust messages that chain up to the new root CA". The design assumed one MA, and the multi-provider reality breaks that assumption.
- **ESCRYPT/ETAS for Transport Canada 2019 (canada2019systeme).**
  - p103: singular misbehaviour components across borders need a shared governance structure.
  - p106: "the deployment model of Root CAs and ICAs will have an impact on misbehaviour detection and reporting", and misbehaviour handling is "in a research phase".
  - This is the closest qualitative statement of the problem, with no measurement.
- **Yoshizawa & Preneel 2022 (yoshizawa2022misbehaviour, DOI 10.1109/CSCN57023.2022.10050926, pp. 144–150).** "ETSI misbehaviour reporting is ineffective, especially cross-border". It is a standards analysis with a qualitative fix and covers ETSI, not the US (corpus summary, abstract basis; full text not obtained).
- **Blackchain (heijden2017blackchain, arXiv 1710.08891).** A set of MAs reaches consensus through a ledger. p5 says it is conceptual and that "Blackchain itself may not be feasible"; there is no quantitative evaluation.
- **Multi-domain revocation designs: xia2026esga (TMC 2026), naskar2024scheme, wasef2008ecmv, li2025blockchain, farran2026blockchain.** All are new schemes, mostly blockchain-based. None models today's US SCMS with one CTL and per-provider MAs, and none measures cooperation policy.
- **jha2018pseudonym.** Proposes cross-certifying SCMSs, with no MA content.
- **Qualcomm patent US11082846B2 (priority 2018-03-20), https://patents.google.com/patent/US11082846B2/en.** Routes misbehaviour reports by the identity of the Pseudonym CA towards the issuer's report catcher. It contains no evaluation. It shows that "forwarding to the issuing provider" is already claimed prior art as a mechanism.
- **Web-PKI analogue for CTL lag.** Ma et al., "Tracing Your Roots", IMC 2021 (dblp conf/imc/MaAMDB21; https://zakird.com/papers/roots.pdf) found derivative root stores months behind their upstream. Larisch et al., "No Root Store Left Behind", HotNets 2023, cites that result. Trust-anchor propagation lag is therefore a known concept; no V2X-specific measurement was found.
- **Practice that is moving.** FHWA Sources Sought 693JJ3-25-SS-0004 (posted 24 Jan 2025) seeks a single contractor to "facilitate collaboration among various SCMS providers" and to "validate advanced SCMS capabilities, including certificate trust lists and misbehavior detection" (https://console.sweetspotgov.com/federal-contracts/3e251528-3d2a-5e97-9cbf-6223e792ecd7). The repo notes record that the first over-the-air MBD-to-CRL demonstration was only at a June 2026 plugfest (01-deployment-and-governance.md, item 7). An operational interoperability test programme could therefore publish CTL and MBD results first, although it would be testing, not a measurement study.

### What remains open

1. A measurement of how a misbehaver's evidence splits across provider MAs, given real witness geometry and report survival, and what that does to the decision rule.
2. The effect of the cooperation model and its latency on whether and when revocation happens: none, forward-only, revocation requests per 5GAA (26), each with a deadline or none.
3. Policy shopping, where an attacker enrols with the most lenient provider, with the honest-revocation rate as the counter-metric.
4. A V2X-specific measure of the exposure window while a CTL change propagates. This is open but weak (see §2).

### Pitch corrections (must be fixed before any write-up)

- **"The draft 5GAA certificate policy says only that MAs 'should' cooperate"** is incomplete, and as framed it is wrong. Provider Requirements §2.2 makes report sharing a SHALL, and 5GAA (13) also says "shall". What is non-binding is the MA-level duty to supply linking information and to act on another MA's request ((24)–(26)). What is missing everywhere is a deadline and a format. The engine's own code says so: "no published MA-to-MA format" (crates/v2xw-proto/src/scms/run/mod.rs l.1286). A "no cooperation" arm is therefore a stress case ("sharing not implemented"), not the status quo.
- **"The USDOT plan wants more [providers]."** The plan's target is that "2 SCMS providers demonstrate interoperable security credentials management" by 2028 (01-deployment-and-governance.md item 10, citing the plan, pp. 10–13). The 2025 test CTL carries 5–6 roots (01 item 1), which supports "more are coming", but the plan itself does not say so.
- **"CTL Update Process still in development"** is correct (Provider Req. §1.1 p4; publications page today).

## 2. Feasibility in the engine

### Already built (most of the multi-provider part)

- **crates/v2xw-proto/src/scms/deployment.rs**
  - `Provider { ma_mode, crl_publication, fleet_share, on_ctl }` (l.295–312).
  - Presets: `UsProduction2025` has two providers at 50/50 with manual-review MAs; `UsTestCtl2025` has six providers (l.456–468).
  - `ma_report_sharing` defaults to true (l.433, 451).
  - `report_route` sends a report to the reporter's MA and then to the accused's MA (l.763–771).
  - Devices are split across providers by a seeded hash of `fleet_share` (l.715–735).
  - Per-region latency between MAs is modelled (`city_latency`, l.755).
- **crates/v2xw-engine/src/phase2.rs**
  - One MA host per provider, `ma: Vec<MaHost>` (l.1515–1516).
  - `ingest_arrivals` forwards foreign-certificate reports over the MA-to-MA link, or counts them as unresolvable when sharing is off (l.4650–4715).
  - Metrics already exist: `reports_shared`, `reports_foreign_unresolvable` and `devices_by_provider` (l.1184–1191).
- **crates/v2xw-proto/src/scms/run/mod.rs.** `shared_report_at_ma` and `shared_report_evidence` (l.1283–1330), labelled as modelled.
- **crates/v2xw-threat/src/ma_corroborate.rs.** The MA decision rule is evidence-based, not a report counter:
  - Events are correlated per pseudonym.
  - `event_min_reporters` = 2 and `min_events` = 3 within `window_s` = 60 s.
  - It dates evidence by observation time.
  - Its parameters are settable (phase2.rs l.286–326).
- **crates/v2xw-proto/src/scms/params.rs l.335–347.** `MaMode::ManualReview { review }` (8 h by assumption) or `Automatic`.
- **CTL backend.**
  - `ctl_publication_deadline` implements the SLAs: an addition within 3 business days, a removal within 48 h (governance.rs l.957).
  - CTLs are published during a run (phase2.rs l.3544).
  - `ctls_after` and `install_ctl` exist (governance.rs l.2141, l.2248).
  - The ETSI path has a per-device ECTL fetch that tracks sequence numbers (phase2.rs l.1079, 1255, 4913–4916).

### Must be built

| Item | Rough size |
|---|---|
| Per-provider MA decision parameters. Today all MAs are cloned from one config: `vec![ma; providers]` (phase2.rs l.2174, from `MaHost::from_scenario`, l.1908). | ~150–300 LoC plus schema, 1–3 days |
| Cooperation policy per provider: none, forward with delay d or a daily batch, immediate, and a "revocation request" mode in which the reporter's MA decides and the issuer executes (5GAA (26)) | ~200–300 LoC, 1–2 days |
| Pinning an attacker to a chosen provider, overriding the fleet hash | ~50–100 LoC, under 1 day |
| Per-subject evidence split by reporter provider (reporter ground truth exists) | ~100 LoC |
| CTL part: a US-path CTL refresh during RA top-ups, with per-device installed CTL sequence (today US devices install the CTL only in the enrolment `TrustBundle`, run/enrol.rs l.511–528) | ~300–500 LoC |
| CTL part: multi-day vehicle persistence | 1–2 weeks, as the pitch says |

### Engine limits that undermine parts of the direction

1. **The time scales do not match, and this is the biggest problem.** Runs are about 600 s and take about 2.4 h at about 450 vehicles (THOUGHTS.md, "Engine claims must match the engine"). In production, time to revocation is made of an 8 h review assumption, a CRL SLA of one business day, 5GAA's 24 h for CRL publication (49), and up to 5 days for ACA processing (261). Cooperation delays of interest run from hours to days. The engine therefore measures only whether the evidence suffices, and how many seconds to minutes it takes under `Automatic` mode. End-to-end time to revocation is that figure plus SLA terms added analytically, and the analytical terms dominate. The headline must be stated as evidence sufficiency, not days to revocation.
2. **Delayed sharing loses no evidence in this MA.** The MA dates evidence by observation time, so a forwarding delay mostly adds a constant shift. "Delayed vs immediate" is then close to trivially additive, unless a deadline interacts with something that expires. Today nothing does: linking across pseudonyms happens at the LAs after the decision.
3. **The split is a binomial thinning.** Providers are assigned by a hash that ignores geometry, so the witnesses' provider mix follows the market share. The engine contributes the distribution of witness counts per event and correlated report loss. Example: at 95/5 with the attacker in the 5% provider and sharing off, the deciding MA needs 2 or more same-provider witnesses per event, so revocation almost never happens. That is predictable from witness counts alone. The engine's contribution is real but modest, as the pitch itself concedes.
4. **Honest revocations may be a null result.** The corroboration defaults were tuned so that no pseudonym reached a second corroborated event at 1,500 veh/h without an attacker (ma_corroborate.rs, module doc). Differences in honest revocation across cooperation modes are therefore likely to be near zero unless benign-fault density or the MA settings are varied deliberately.
5. **Policy shopping is decided by the architecture.** Only the issuing provider's ACA and LAs can link its pseudonyms (phase2.rs l.4651–4657; Provider Req. §2.2). In forward-only cooperation, the issuer's MA alone decides, so an attacker always gets the policy of the provider it enrols with. The engine would confirm this rather than discover it. It only becomes an open question when the 5GAA (26) "revocation request" model is added, where a witness's MA decides.
6. **The CTL part is weak.** The encounter-weighted unverifiable share is roughly the new provider's share times the fraction of receivers still on the old CTL, under well-mixed traffic, so geometry adds little. The stale fraction depends entirely on the assumed top-up and refresh behaviour, for which there is no deployment data. It needs the multi-day persistence that has not been built. The concept is the web-PKI root-store lag (Ma et al. 2021).
7. **Replay exactness holds.** Changing MA policy or cooperation is backend-only, and revocation does not change driving (THOUGHTS.md), so the air replays exactly, as the pitch claims. Pinning an attacker to a provider changes only its certificates, not its mobility or radio streams, which are keyed per entity.

## 3. Recommendation

- **Keep, narrowed: "Who decides in a multi-provider SCMS".** Compare the forward-to-issuer cooperation model (what the engine does, matching Provider Req. §2.2) with the revocation-request model (5GAA (26)) and with no sharing, at 50/50, 80/20 and 95/5, with heterogeneous per-provider MA settings and an attacker that shops for a provider.
  - Measure evidence sufficiency, attacker revocation and honest revocation.
  - The non-obvious question is the trade-off. Revocation requests defeat policy shopping, because the strictest MA among the witnesses can act. But they let the most trigger-happy MA revoke other providers' customers.
  - That makes this the multi-provider version of the live "framing honest vehicles" candidate, and it should be a chapter of that work, not a standalone paper.
  - Build effort: about 1 week. It would supply requirement text the SCMS Manager could adopt: a binding format, a deadline, and who decides.
- **Drop the CTL half, or reduce it to a short analytical section.** It is weak in the engine, needs 1–2 weeks of persistence work, and its concept is borrowed from web-PKI studies.
- **Report time to revocation as engine-measured evidence time plus cited SLA terms**, with the SLA terms labelled as analytical.
