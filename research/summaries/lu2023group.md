# Group Signature Authentication Scheme with Credit Evaluation Mechanism in VANET
Yanfei Lu, Suzhen Cao, Qi-Zhi He, Zixuan Fang et al. · International Conference on Computer Supported Cooperative Work in Design 2023 · https://doi.org/10.1109/cscwd57460.2023.10152641 · relevance: low · basis: abstract

**Problem:** Aiming at the problems of privacy disclosure and false information provided by malicious vehicles in vehicular ad-hoc network (VANET) communication, this paper proposes a group signature authentication scheme with credit evaluation mechanism combining certificateless public key cryptography and group signature technology.
**Approach:** In this scheme, the problem of certificate management and key escrow were solved by using certificateless public key cryptography; secondly, used group signature technology, any member of the group could sign the message anonymously on behalf of the group to protect the private information of the vehicle; finally, a single factor weight evaluation mechanism and a reward and punishment mechanism were introduced to evaluate the reliability of shared information and encourage users to share real information.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Based on the computational Diffie-Hellman difficult problem, the scheme is proved to satisfy the security of signature unforgeability under the random oracle model. Compared with the existing schemes, the experimental results show that the scheme reduces the time of vehicle signature by 10.82%~45.95%, and the time of group administrator verification by 4.87%~30.09%, which proves that the scheme is more effective.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
