# Paper 1 experiments (P1-E5 to P1-E10)

One section per experiment, each written by the package that ran it. Each section gives
the §1.3 falsification test (research/synthesis/paper-plan-2026-10.md) *before* its
runs, then the result as measured, pass or fail. The §1.1 qualifiers apply to every
sentence: "published" / "CV-Pilot", never "deployed"; "would revoke under MA rule R,
given trigger rate T"; no claim of a novel mechanism.

## P1-E8. Evasion: hiding inside the benign device class (item F)

### Pre-registration (written before the runs)

**Question.** The `v2xw ma-replay` MA rule family can read three device-class features
from fields the MA already receives: the evidence BSM's secMark phase, the accused's
stated PositionalAccuracy, and the issuing CA. An attacker writes the first two itself.
What attack budget does it gain by writing them to look like the benign (free, low
accuracy) class? Budget is defined per attacker as the seconds from attack start to the
first revocation decision. If there is no decision, the budget runs to the attacker's
exit and is censored there. It is summed over attackers, per rule and parameter
(`attack_budget_s` in `ma-replay.csv`, per attacker in `ma-replay-attackers.csv`).

**Engine work before the runs.**
- Item F defect: `ma.received.secmark_phase_ms` was computed from the report's evidence
  time, which is the BSM's 1609.2 `generationTime`, not from its `secMark`. On the CV-pilot
  fit the header clock jitters with sd 28 ms, so a locked device's phase landed off the
  epoch on most reports. The `cluster` rule's phase feature was therefore noise. It now
  reads the judged BSM's `secMark`, which the evidence carries.
- New attack `threat/attacker/evasion/BenignClassMimic`. Its payload is a legacy kind,
  unchanged (here ConstPos). Its disguise adds a constant `secmark_shift_ms` to secMark,
  which leaves every Δt unchanged, and states `stated_accuracy_m`. The legacy catalogue
  is untouched.

**Runs.** Quarter fleet, 60 s live after a 30 s warm-up (cut from 120 s before any run
finished: 120 s peaked at 483 MB, over the small run slot), locked share 0.25 (Tampa), 3 % attackers from 30 s, US
profile on secMark, report shuffle 10 s (the stated P1-E7 assumption). One seed. Runs:
EV0 plain ConstPos; EVN mimic with no disguise; EVP 25 ms phase shift; EVA stated 9 m;
EVB both; EVP2 EVP again; L1 and L1B are EV0 and EVB with every OBU locked.

**Falsification tests, fixed before the runs:**
1. **Null.** EVN's `ma.received` stream must be byte-identical to EV0's (digest), so its
   budget gain is 0 under every rule. Shown red by EVB, whose `ma.received` must differ.
2. **No side effect.** A secMark shift that leaves Δt unchanged, and an accuracy claim
   that SM-MBR ignores, must not change which reports are filed. For EVP, EVA and EVB
   against EV0: the same number of reports at the MA. Under `pseudonym-k` and `device-k`,
   which read no class feature, the budget must be identical. Any difference falsifies
   the implementation.
3. **Phase disguise works.** In EVP, the share of attack reports the MA reads as
   "locked" must be 0. In EV0 it must be ≥ 0.15, since about 25 % of attackers are
   locked.
4. **Reported, not predicted in sign:** the budget gain (EVx − EV0, summed over
   attackers, and the count of attackers whose revocation is lost) under `catch` and
   `cluster`. Theory: `cluster` gains only from attackers that are locked in EV0, and
   only while the free class is crowded (≥ c accused devices). `catch` gains only on
   positional subtypes, and ConstPos often leads with `constantPositionFrozen`, which
   weighs 1. If the gain is 0, that is the result.
5. **Determinism.** EVP2 must be byte-identical to EVP.

### Results (as measured, 2026-10-27)

Item-F binary: optimised debug build of this branch. One seed. Quarter fleet, 60 s live,
10 attackers, 5 of them accused at the MA, 2 of those 5 locked. 6,755 reports at the MA
per run. Scripts are in the package scratch folder (`dump.py`, `analyze.py`, `pdc.py`).

| Test | Measured | Verdict |
|---|---|---|
| 1. Null: EVN `ma.received` = EV0 | digest f41deaa2 = f41deaa2; red on EVB (e4026ee1) | pass |
| 2a. Same reports filed (EVP, EVA, EVB) | 6,755 = 6,755 | pass |
| 2b. `pseudonym-k`, `device-k` budget unchanged | 573.7 → 616.8 s and 3,210.8 → 3,426.4 s | **fail as measured** (accounting, below) |
| 3. Phase disguise: EV0 attack reports read locked | 0.248 | pass |
| 3. Phase disguise: EVP attack reports read locked | 0.039, expected 0 | wrong expectation (below) |
| Item F: all-locked fleet's honest reports read locked (L1) | 1.000; **0.091 on the pre-fix release binary** | pass, and the fault goes red |
| 5. Determinism: EVP2 = EVP | 72b8acb2 = 72b8acb2 | pass |

- **2b, root cause.** No MA decision moved: 0 of 370 rule × attacker decision times
  differ between EV0 and EVP, EVA or EVB. The budget's start, `gt.device_exposure`
  `attack_start`, is the attacker's first on-air edit (phase2 `first_on_air`). The
  disguise restamps secMark from the window start, 0.1–4 s before ConstPos first changes
  a claim, so every budget starts earlier. The record's own doc says "first falsified
  message". The definition needs fixing (open). Every "gain" below is measured on
  decision times instead.
- **3, the wrong expectation.** The two locked attackers read 0.0 locked under the
  disguise (85 and 19 reports). The free attackers' phase is uniform and stays uniform
  under a constant shift, so about 2 ms in 50 lands in the locked window by chance.

**The attack budget the disguise buys under the four `ma-replay` families:** none, for
either disguise.

| Family | What the disguise changes | Decisions moved |
|---|---|---|
| `pseudonym-k`, `device-k` | nothing they read | 0 |
| `catch` | the stated accuracy, which only weighs positional subtypes. 99 % of ConstPos attack reports lead with `constantPositionFrozen` (weight 1). Honest devices state 6.5 m, so 9 m stays in the same 3–10 m band | 0 |
| `cluster` (per-report class) | the class, but both classes are crowded. About 4 % of free devices' reports land in the locked window (honest reports read locked: 0.078), so locked becomes a crowded class at c = 3 and c = 10 alike | 0 |

**What a per-device class would change.** This is an offline analysis (`pdc.py`), not in
the engine family. It classifies each device with the real-data rule (locked when ≥ 80 %
of its reports so far are within 1 ms) and keeps everything else as in `cluster`.
- **c = 3:** nothing moves.
- **c = 10 (k = 3):** the disguise delays both locked attackers' revocation, by 10.1 s
  and 0.3 s.
- **c = 10 (k = 5):** both revocations are lost inside the run, a budget of ≥ 57 s and
  ≥ 37 s, censored at the run's end.
- **Honest devices blocked:** 31–69, the same with and without the disguise (68 → 69 in
  one cell).

So the phase disguise buys budget only against an MA that classifies per device and
relaxes thresholds for a crowded free class. That is the MA a benign-trigger-aware
operator would build.

Qualifiers:
- This is one seed, 60 s, 2 locked attackers. It shows the mechanism, not a rate.
- "Would revoke under MA rule R" applies throughout; no US MA rule is published.
- The issuer feature cannot be faked and was not exercised: all accused were in
  2 providers.
- In the all-locked L1, sound rules block up to 31 honest devices. The pre-fix binary
  gives the same 31, so it is not this change. It disagrees with MAL1R's 0 (8becf72a) and
  is open.
