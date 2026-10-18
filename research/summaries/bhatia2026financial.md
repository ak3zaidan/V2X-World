# A Financial System Digital Twin for Cross-Market Stress Propagation: Real-Time Network Simulation Using Graph Attention Networks
Chandni Bhatia, Harry Vadalkar, Sunil Bhatia · unknown venue 2026 · https://doi.org/10.2139/ssrn.6871578 · relevance: low · basis: abstract

**Problem:** This paper proposes and demonstrates a financial system digital twin — a continuously-updating directed network simulation that replicates cross-market stress propagation dynamics in real time.
**Approach:** Existing approaches to systemic risk monitoring rely on static or periodically retrained models that cannot capture the velocity of contagion during acute market dislocations. The framework presented here formally defines four properties that distinguish a digital twin from a conventional simulation — state synchronisation, bidirectionality, counterfactual tractability, and crisis fidelity — and demonstrates each property through an empirical ...
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** The framework presented here formally defines four properties that distinguish a digital twin from a conventional simulation — state synchronisation, bidirectionality, counterfactual tractability, and crisis fidelity — and demonstrates each property through an empirical implementation spanning 17 global equity indices across 2000–2024. The implementation combines daily DCC-GARCH(1,1) correlation estimation with rolling Granger causality tests to ...
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: misbehaviour/intrusion detection in a vehicular context. Low relevance background for direction 1 (closed-loop misbehaviour to revocation); it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
