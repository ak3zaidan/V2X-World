# Lightweight, Post-Quantum Secure Cryptography Based on Ascon: Hardware Implementation in Automotive Applications
Hai Phong Nguyen, Yuhua Chen · Electronics 2024 · https://doi.org/10.3390/electronics13224550 · relevance: high · basis: abstract

**Problem:** In-vehicle sensors/ECUs are constrained; need PQ-resistant lightweight crypto on CAN.
**Approach:** Ascon cipher suite in hardware integrated with CAN on Artix-7 FPGA, with a DSP block for CAN message alignment.
**Tools and scale:** FPGA (Artix-7) at 100 MHz; CAN bus.
**Results:** About 100x speedup over software implementations; low resource usage.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — In-vehicle CAN symmetric crypto; not V2X PKI.
**Novelty implications:** No effect on our directions.
