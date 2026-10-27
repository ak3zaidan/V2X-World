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
