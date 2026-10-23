# VEhicular Secure Network Open Simulator (VESNOS): A Cyber-security oriented co-simulation platform for connected and automated driving
Ahmed Abdo, Guoyuan Wu, Nael Abu-Ghazaleh · Proceedings of the 38th ACM SIGSIM Conference on Principles of Advanced Discrete Simulation 2024, pp. 108–118 · https://doi.org/10.1145/3615979.3656054 · relevance: high · basis: code (github.com/avmon/VESNOS at `997913b5`, the user manual v0.1, and a diff against VENTOS `60c5d62a`). Full text not obtained: ACM and ResearchGate return 403, and no repository copy exists. Full analysis with file and line evidence: `synthesis/vesnos-full-read.md`

**Problem:** Traffic simulators lack connected-vehicle features and the US message-security system.

**Approach:** VENTOS (SUMO plus OMNeT++/Veins over TraCI) with about 2,450 added lines. Each vehicle signs its beacon with ECDSA P-256 in a hand-built IEEE 1609.2 layout. Each receiver reconstructs the signer's implicit (ECQV) key and verifies the signature.
- The ECC is written by hand on `boost::cpp_int`, ported from the CAMP Python reference. OpenSSL supplies only SHA-256, and Crypto++ supplies AES for butterfly expansion.
- One hard-coded test-PKI pseudonym certificate (`v2xpca-test.ghsiss.com`, i = 0x7A, expired 2017-05-16) and its key are shared by every vehicle.

**What is not there:**
- **No SCMS backend.** There is no RA, PCA/ACA, ECA, LA or MA, no root, electors or CTL, no issuance, linkage values, misbehaviour reports, RA shuffle or multiple providers.
- **No pseudonym change.**
- **The CA and CRL modules are VENTOS's dummy CRL-piece broadcaster**, and vehicles discard the pieces.

**Tools and scale:** SUMO through VENTOS's SUMO fork, OMNeT++ 5.4.1 with Veins 4-era 802.11p, free-space path loss in the examples, and no C-V2X despite the manual. The shipped security examples have about 10 vehicles. There are no tests and no performance figures.

**Results:** The paper's case study, known from an excerpt only, shows a receiver rejecting a false message by its signature and pseudonym certificate. No released configuration reproduces it.

**Limitations and future work (as stated by the authors):**
- None available: the full text was not read.

**Limitations found in the code:**
- The signed payload is the 8-byte address of the beacon object, not a BSM.
- The receiver trusts the PCA public key carried in the message, and checks no chain, validity, region, PSID, CRL or replay.
- A verification result is only logged and never reaches the applications. So the CACC and platoon logic lose V2V input once vehicles sign.
- RSUs throw a runtime error on receiving a BSM.
- The falsification and replay attacks act only on the unsigned beacon type, which vehicles no longer send. Jamming is a boolean flag.
- Security adds no bytes to any frame and no simulated time.
- No licence file (GPL-derived from VENTOS). 9 commits; inactive since 2024-05.

**Overlap with our engine:** strong on the claim, weak on substance. It is the only prior open co-simulator that signs with SCMS-format certificates, but it runs no SCMS entity or flow.

**Novelty implications:**
- **Dead:** "first simulator integrating an SCMS with traffic and network simulation", and "first butterfly keys or implicit certificates in a network simulator".
- **Survives, now with code evidence:** the first open simulator in which the SCMS backend itself (issuance, linkage, MBR→MA, linkage-seed revocation, CTL) runs inside the traffic and radio loop, validated against official vectors and the real trust lists.
- **Unaffected:** all directions in `final-three-approaches.md` and `research-plan.md`.
- **Engineering only:** the engine's determinism, its crypto libraries and its finer 802.11p PHY count as engineering against VESNOS, not as novelty.
