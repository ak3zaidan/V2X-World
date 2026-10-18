# Edge Computing-Based VANETs' Anonymous Message Authentication
Cheng-Jun Yang, Jian-Sheng Peng, Yong Xu, Qing-Jin Wei et al. · Symmetry 2022 · https://doi.org/10.3390/sym14122662 · relevance: low · basis: abstract

**Problem:** In order to solve the computing power asymmetry between vehicles and road side units (RSUs) in VANETs under high-density traffic, accelerate the processing speed of message authentication, and solve the problems of high computational overhead and long message authentication time caused by the use of bilinear pairing encryption technology in similar message-batch-authentication schemes, we propose introducing the concept of edge computing (EC) into VANETs and using idle nodes’ resources to assist the RSU in quickly authenticating messages to achieve computing power load balancing under multiple traffic flows.
**Approach:** We propose introducing the idea of edge computing (EC) into VANETs and using idle nodes’ resources to assist RSUs in quickly authenticating messages. We performed a security analysis of the scheme to prove its security properties and compared it with other schemes in terms of performance.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The resources of the TEN are used to reduce the computational burden of RSUs and message authentication time, thereby improving the efficiency of system authentication of messages. The experimental results show that our scheme has a transmission overhead of 2400 bytes when there are four TENs, and the number of verification message requests reaches 20, which outperforms other methods.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
