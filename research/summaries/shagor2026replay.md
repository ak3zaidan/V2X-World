# Replay-Resistant Admission and Permissioned Quorum Consensus in Connected Vehicle Networks: A Fail-Closed, Implementation-Grounded Security Study
Md Shahanur Islam Shagor · Research Square 2026 · https://doi.org/10.21203/rs.3.rs-11047001/v1 · relevance: low · basis: abstract

**Problem:** Connected-vehicle security requires more than message authentication: a receiver must also decide whether an identity is admitted, whether a message is fresh, and whether a vote originates from a currently authorized validator.
**Approach:** This article isolates those controls from the broader OmniGuard V2X platform and evaluates them as a permissioned coordination subsystem. The design combines fail-closed registry-gated vehicle admission, authenticated session-to-identity binding, bounded timestamp-and-nonce replay state, fixed epoch-scoped validator membership, HMAC-SHA-256 authentication of canonical vote records, and a configurable quorum whose default threshold is ceil(2N/3).
**Tools and scale:** Not stated in the abstract.
**Results:** Security analysis shows that the subsystem constrains outsider and replay-driven influence but is deliberately not a Byzantine-fault-tolerant protocol: an authorized coalition meeting quorum can still approve a malicious proposal.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** partial — shares topic terms (v2x, threshold) with the engine, but the abstract gives no indication of an integrated traffic-radio-PKI simulation, closed-loop revocation, or counterfactual reruns.
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.
