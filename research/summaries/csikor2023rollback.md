# RollBack: A New Time-Agnostic Replay Attack Against the Automotive Remote Keyless Entry Systems
Levente Csikor, Hoon Wei Lim, Jun Wen Wong, Soundarya Ramesh et al. · ACM Transactions on Cyber-Physical Systems 2023 · https://doi.org/10.1145/3627827 · relevance: medium · basis: abstract

**Problem:** Automotive Keyless Entry (RKE) systems provide car owners with a degree of convenience, allowing them to lock and unlock their car without using a mechanical key. Today’s RKE systems implement disposable rolling codes, making every key fob button press unique, effectively preventing simple replay attacks.
**Approach:** We introduce RollBack, a new replay-and-resynchronize attack against most of today’s RKE systems. In particular, we show that even though the one-time code becomes invalid in rolling code systems, replaying a few previously captured signals consecutively can trigger a rollback-like mechanism in the RKE system.
**Tools and scale:** Car-Hacking/CAN dataset (mentioned in abstract)
**Results:** Our ongoing analysis (with crowd-sourced data) against different vehicle makes and models has revealed that ∼ 50% of the examined vehicles in the Asian region are vulnerable to RollBack, whereas the impact tends to be smaller in other regions, such as Europe and North America.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Addresses V2X attacks or misbehaviour/trust evaluation that our engine can execute and detect in closed loop; it does not model the SCMS, revocation or joint traffic/radio/PKI ground truth.
**Novelty implications:** Does not rule out our candidate directions; any claim about this specific attack/detection/trust mechanism must cite it. Left open: end-to-end detection-report-revocation evaluation with safety outcomes.
