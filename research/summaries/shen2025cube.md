# CUBE-PUF-Based Anonymous Mutual Authentication Protocol for Internet of Vehicles
Ao Shen, Weijie Tan, Chunzhi Jia, Yuling Chen et al. · IEEE Internet of Things Journal 2025 · https://doi.org/10.1109/jiot.2025.3586869 · relevance: low · basis: abstract

**Problem:** However, data interactions between IoV entities involve personal privacy, and once maliciously attacked, they may threaten the stable operation of the entire transportation system.
**Approach:** To address these challenges, this article proposes a lightweight mutual authentication protocol based on physical unclonable function (PUF), which not only ensures vehicle anonymity and traceability but also supports a pseudonym update function after authentication. Additionally, to counter the exposure of a large number of challenge-response pair in traditional PUF-based authentication—making them susceptible to machine learning (ML)-based modeling attacks—this article introduces a CUBE-PUF scheme based on digital Rubik’s Cube and random numbers.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** We conduct both formal and informal security analyses of the proposed protocol and rigorously verify its security using the ProVerif verification tool. Furthermore, comparative evaluations with existing protocols demonstrate that our approach significantly reduces communication and computational overhead while offering enhanced security.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
