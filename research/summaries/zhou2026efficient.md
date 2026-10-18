# Efficient DoS Resilient Signature batch Verification in VANETs
Zhichao Zhou, Panos Papadimitratos · 2026 IEEE Vehicular Networking Conference (VNC) 2026 · https://doi.org/10.1109/vnc69225.2026.11629124 · relevance: medium · basis: abstract

**Problem:** In Vehicular Communication (VC) systems, vehicles broadcast authenticated safety messages that receivers validate by verifying digital signatures. Batch verification reduces the average validation cost by checking multiple signatures in a single operation.
**Approach:** To address this challenge, we propose a DoS-resilient signature batch verification scheme that leverages lightweight message pre-validation. We evaluate our scheme through a large-scope simulation, and our results show that our scheme is highly resilient to clogging DoS.
**Tools and scale:** We evaluate our scheme through a large-scope simulation, and our results show that our scheme is highly resilient to clogging DoS.
**Results:** Batch verification reduces the average validation cost by checking multiple signatures in a single operation. The average message waiting time is less than 100ms and it is nearly unaffected by the attack, with all benign messages are processed before expiration even under a strong DoS attack.
**Limitations and future work (as stated by the authors):**
- To address this challenge, we propose a DoS-resilient signature batch verification scheme that leverages lightweight message pre-validation.
**Overlap with our engine:** partial — DoS against signature (batch) verification; relates to verification-load attacks (runner-up NR sidelink attacks with verification load).
**Novelty implications:** Does not remove any of our claims; it is a related point of comparison. We cannot claim to be first on the specific mechanism it covers; the joint closed-loop, multi-layer evaluation remains open as far as this abstract shows.
