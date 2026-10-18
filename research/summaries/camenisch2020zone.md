# Zone Encryption with Anonymous Authentication for V2V Communication
J. Camenisch, Manu Drijvers, Anja Lehmann, G. Neven, Patrick Towa · European Symposium on Security and Privacy 2020 · https://doi.org/10.1109/eurosp48549.2020.00033 · relevance: high · basis: abstract

**Problem:** V2V beacons are cleartext and pseudonymously signed, a privacy problem.
**Approach:** Zone encryption with keys tied to geographic zones, plus dynamic group signatures with attributes for anonymous authentication.
**Tools and scale:** Cryptographic design.
**Results:** 224 bytes overhead per message; 216-byte signatures at 128-bit security; unlimited local pseudonyms.
**Limitations and future work (as stated by the authors):**
- None stated.
**Overlap with our engine:** partial - alternative to pseudonym certificates for linkability (runner-up cross-layer linkability).
**Novelty implications:** Alternatives to SCMS pseudonyms exist; quantifying linkability under real SCMS in a cross-layer sim remains open.
