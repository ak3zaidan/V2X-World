# CARBUROS: A Formally Verified Authentication Protocol for V2X Communications
Marco De Vincenzi, Chiara Bodei, Gabriele Costa, Ilaria Matteucci · IEEE Access 2025 · https://doi.org/10.1109/access.2025.3644958 · relevance: low · basis: abstract

**Problem:** The growing adoption of Vehicle-to-Everything (V2X) communications in Intelligent Transportation Systems (ITS) demands robust authentication protocols to ensure secure and reliable interaction among road actors.
**Approach:** To address this gap, this work presents CARBUROS, an MFA protocol specifically designed for ITS scenarios. It introduces a dual-channel mechanism that combines a standard digital communication channel with a physical proximity-based channel, ensuring that authentication reflects real-world co-location in space and time.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** CARBUROS was formally modeled and verified using ProVerif, an automatic cryptographic protocol verifier.
**Limitations and future work (as stated by the authors):**
- "In this context, Multi-Factor Authentication (MFA) can serve as an important building block; however, integrating MFA into vehicular environments remains nontrivial."
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
