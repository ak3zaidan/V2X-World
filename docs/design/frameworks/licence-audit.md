# Licence audit: copyleft sources named in the tree

Status: WP-01 (adoption plan §2, wave 1), 2026-10-23. This is an engineering audit, not
legal advice. It covers every file under `crates/` and `ui/` (not `ui/node_modules`) whose
comments name SUMO, Veins, Artery, OpenCV2X, F2MD, INET, Vanetza, SimuLTE, ns-3 or srsRAN
as the source of code or data.

How the files were found:

- `git grep -niE "ported from|transcribed from|field for field"` over `crates/` and `ui/`;
- origin verbs (`transcri|translat|ported|copied|verbatim|lifted|adapted from|derived
  from|based on`) on a line that names one of the sources above;
- references to a source file (`*.cc`, `*.cpp`, `*.h`, `*.ned`, `nist_per.py`) or to a
  `Class::method` of one of those frameworks.

Verdicts: **rewritten** (the code was a translation and was re-written from the paper or
standard), **re-cited** (the code was already independent, and only its comment or card
named the wrong origin), **behaviour reference** (the framework was read for behaviour or
a constant, and the file says so; allowed by adoption plan §1), **data** (numeric values or
formats with attribution, allowed by §1), **open** (needs further work, listed at the end).

## The four files WP-01 names

| File | Before | Verdict | What changed |
|---|---|---|---|
| `crates/v2xw-mobility/src/lanechange/lc2013.rs` | "Ported from SUMO `MSLCM_LC2013.cpp`"; the speed-gain and keep-right blocks followed the structure of `_wantsChange` closely, and the docs named SUMO's member variables | **rewritten** | Header cites Erdmann 2015 (Lecture Notes in Mobility, doi:10.1007/978-3-319-15024-6_7) and the SUMO vType defaults as facts. `step` re-structured from the paper's four motivations: `relative_gain`, `full_speed_horizon_s` (time at full speed before braking for the lane end or a slower leader), `RoadContext::acceptance_s`, separate right and left steps. A temporary differential test drove old and new over 800 000 random steps: every decision and every accumulator bit identical, so no golden moves. Unit tests unchanged and passing. |
| `crates/v2xw-msg/tests/artery_cam.rs` → `cam_en302637.rs` | "Artery's `CaService`, field for field", a transcription of `checkTriggeringConditions` / `sendCam` / `genCamDcc` | **rewritten** | Oracle re-written from EN 302 637-2 V1.3.2 §6.1.3 (conditions 1 and 2, `N_GenCam`, low-frequency container, `T_GenCam_DCC` bounds). Artery's counts kept as data (`EXPECTED`: 181/159/104/60/60 CAMs per DCC interval, LF 89/89/89/104/60/60). New `the_oracle_can_tell_a_wrong_rule` changes each of six rules and requires a mismatch (the fault check). |
| `crates/v2xw-radio/src/dcc.rs` (`regate`) | "Transcribed from Vanetza's `LimericBudget::update`" | **rewritten** | Derived in the doc comment from TS 102 687 V1.2.1 Annex B, B.1 and B.2; body re-written in the standard's terms (fraction still to run, time waited). Same arithmetic, so the test values (75, 150, 160, 100 ms) are unchanged; Vanetza's 75 ms kept as a cross-check value. |
| `crates/v2xw-radio/src/per.rs` | distance-spectrum coefficients "transcribed from `nist_per.py` / ns-3 `NistErrorRateModel`", card source `SourceKind::Code` | **re-cited** | Cited to Miller, NIST 2003, Tables 3.1.1-3.1.3 (US-government work), punctured rates after Haccoun and Bégin 1989; card source now `Paper`. Values unchanged; the 24-entry SNR table test still holds them. |

## Other files found by the grep

| File | Before | Verdict |
|---|---|---|
| `crates/v2xw-mobility/src/engine.rs` (LC2013 integration, card) | `currentDistDisallows(...)`, `_wantsChange`, `informLeader`, `LaneQ::occupation`, quoted SUMO debug strings; card `SourceKind::Code` "SUMO MSLCM_LC2013.cpp" | **re-cited**: comments describe the rule from Erdmann 2015; card source is the paper plus SUMO's vType documentation. The code is the engine's own (MOBIL safety, its own route model). |
| `crates/v2xw-netval/src/report.rs`, `docs/validation/network.md` | PHY-14 "copied from ns-3's `NistErrorRateModel`" | **re-cited** to Miller's tables. |
| `crates/v2xw-threat/src/ts103759.rs` | calibration plan "read the predicate out of the F2MD source (`ExperiChecks.cc`)"; limitation "not ported from the F2MD source" | **re-cited**: the plan now says re-implement from the published description, never copy; the limitation says no F2MD source was used. |
| `crates/v2xw-node/src/rxval.rs` | "transcribed in Vanetza, `verification.cpp`" | **re-cited**: the values are the C2C-CC Basic System Profile's RS_BSP_168; Vanetza named as a behaviour reference only. |
| `crates/v2xw-radio/src/sidelink.rs` | TBS table "Transcribed from the srsRAN 4G `tbs_tables.h` copy" (AGPL-3.0) | **data**: the values are 3GPP TS 36.213 Table 7.1.7.2.1-1; srsRAN named only as the copy compared against. See open item D-1. |
| `crates/v2xw-energy/src/hbefa3.rs` | coefficient table "transcribed by script from SUMO's published table" | **data** (HBEFA3 fits with attribution, as sumo.md §2 allows). |
| `crates/v2xw-energy/src/phemlight.rs`, `cite.rs` | "Data transcribed from SUMO's data/emissions/PHEMlight"; algorithm "re-implemented from `CEP.cpp`'s published description" | **data** for the EU4 tables; the algorithm is **open** (E-1). |
| `crates/v2xw-msg/src/generator.rs` | `tests/artery_cam.rs` and `CaService::genCamDcc` named | **re-cited** to the new oracle; the ns-3 `BsmApplication` 10 ms is a constant (**behaviour reference**). |
| `crates/v2xw-mobility/src/carfollowing/{krauss,eidm,acc,w99}.rs`, `lateral/sl2015.rs`, `vru/social_force.rs`, `coop.rs`, `classes.rs`, `intersection/signal_fixed_time.rs`, `views.rs`, `engine.rs` (stop speed, link approach, pedestrian blocking, best lanes) | SUMO files or functions named, "read only, never vendored" | **behaviour reference**: each cites its paper and takes constants or documented behaviour; not line-reviewed in this package. |
| `crates/v2xw-energy/src/ev.rs` | `HelpersEnergy.cpp` air density | **behaviour reference** (a constant). |
| `crates/v2xw-demand/src/choice/gawron.rs` | `ROGawronCalculator` | **behaviour reference** (Gawron 1998 is the source; options and defaults from the duarouter docs). |
| `crates/v2xw-threat/src/{ma_corroborate,attack_ext}.rs` | `F2MDParameters.h` | **data** (thresholds, allowed by §1). |
| `crates/v2xw-record/src/dataset/leakage.rs` | two F2MD field spellings "verbatim" | **data** (format names). |
| `crates/v2xw-net/src/stack/mib.rs` | Vanetza `geonet/mib.hpp` as cross-check | **behaviour reference**; the source is EN 302 636-4-1 Table H.1. |
| `crates/v2xw-energy/src/moves.rs` | tables "transcribed from the MOVESTAR repository" | **data** (EPA MOVES rates; not a framework in §1). See open item D-2. |
| `crates/v2xw-world/src/sumo.rs`, `crates/v2xw-interop/*`, `crates/v2xw-cli/src/export_sumo.rs`, `ui/apps/studio/src/lib/schema.ts` | SUMO `net.xml`/`.rou.xml` import, export, TraCI | **data** (file formats and protocol, allowed by §1). |
| `docs/validation/radio-fidelity.md` §7.2-7.3 | described the transcriptions | updated to the standard-derived oracle and regate. |

Not about copyleft, and left alone: "ported from `legacy/scms_sim_ref`" (this project's own
earlier code), "transcribed from" a standard, paper figure or the project's own protocol
documents, and "field for field" used for round-trip tests.

After the change, `git grep -niE "ported from|transcribed from|field for field" -- crates ui`
returns no line that names a GPL, LGPL or EPL source.

## Open

- **E-1 PHEMlight algorithm** (`crates/v2xw-energy/src/phemlight.rs`): `power_kw` and
  `decel_coast` follow SUMO's `CEP.cpp` closely (it reproduces a code-level quirk, the
  `(f_k·v)^k` higher terms). Re-derive from TU Graz's PHEMlight documentation and, per
  adoption plan §1, load the EPL-distributed vehicle tables from the user's SUMO
  installation at run time instead of vendoring the two EU4 samples.
- **D-1 TBS table** (`crates/v2xw-radio/src/sidelink.rs`): re-check the entries against the
  TS 36.213 PDF, so that the AGPL copy is not the only source.
- **D-2 MOVESTAR** (`crates/v2xw-energy/src/moves.rs`): confirm the MOVESTAR repository's
  licence for the two distilled tables.
- **Miller report**: the NIST report is not cached; check the coefficient list against its
  Tables 3.1.1-3.1.3 page by page (the card says UNVERIFIED at the page level).
- The **behaviour reference** files were not line-reviewed against SUMO in this package.
