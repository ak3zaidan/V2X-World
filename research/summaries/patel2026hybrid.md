# Hybrid Adaptive Post-Quantum Cryptographic Framework (HybridMedPQC) with Optimized SPHINCS+ for Long-Term Medical Image Security
Rucha Patel, Divyangna Gandhi · International Journal of Drug Delivery Technology 2026 · https://doi.org/10.25258/ijddt.16.58s.126 · relevance: low · basis: abstract

**Problem:** Quantum-capable adversaries threaten the long-term integrity of medical images exchanged through PACS/DICOM workflows, while post-quantum signatures introduce storage and latency trade-offs. This paper proposes HybridMedPQC, a tier-adaptive hybrid signing framework that combines CNN-and DICOM-assisted Image Sensitivity Classification with a policy-driven Adaptive Algorithm ...
**Approach:** This paper proposes HybridMedPQC, a tier-adaptive hybrid signing framework that combines CNN-and DICOM-assisted Image Sensitivity Classification with a policy-driven Adaptive Algorithm Selection Engine.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** For each image, Layered Signature Generation produces an inner archival anchor (SPHINCS+ or Dilithium/Falcon) and an outer transport signature (Dilithium or Falcon) bound with an RFC3161 timestamp to enable outer-first fast verification. Projected evaluation on 1024×1024 images shows up to 91.9% signature overhead reduction for the Standard tier and 81.2% for the Sensitive tier, versus standalone SPHINCS+, while ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
