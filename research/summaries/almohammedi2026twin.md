# Twin-Fidelity-Aware Resolution of Direct xApp Conflicts in Open RAN
Akram Almohammedi, Mohammed Balfaqih, Sam Darshi et al. · arXiv 2026 · https://arxiv.org/abs/2607.22857 · relevance: low · basis: abstract

**Problem:** Open Radio Access Network (O-RAN) allows independently developed xApps to control RAN functions through the Near-Real-Time RAN Intelligent Controller (Near-RT RIC).
**Approach:** When xApps with conflicting objectives operate concurrently, they may issue incompatible actions that degrade network performance.
**Tools and scale:** Mentions: 5G.
**Results:** System-level 5G evaluations show that it achieves the closest throughput-power trade-off to the optimum across operator energy priorities, yielding normalized utility regret of $0.017 \pm 0.006$, versus $0.159 \pm 0.052$ for a COMIX-style twin-based selector. Under severe NDT drift (10 dB), it reduces utility regret from $11.19 \pm 3.58$ to $0.55 \pm 0.25$.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
