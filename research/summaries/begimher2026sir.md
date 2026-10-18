# SIR-Bench: Evaluating Investigation Depth in Security Incident Response Agents
Daniel Begimher, Cristian Leo, Jack Huang, Pat Gaw et al. · arXiv 2026 · https://arxiv.org/abs/2604.12040 · relevance: medium · basis: abstract

**Problem:** We present SIR-Bench, a benchmark of 794 test cases for evaluating autonomous security incident response agents that distinguishes genuine forensic investigation from alert parroting. Derived from 129 anonymized incident patterns with expert-validated ground truth, SIR-Bench measures not only whether agents reach correct triage decisions, but whether they ...
**Approach:** We present SIR-Bench, a benchmark of 794 test cases for evaluating autonomous security incident response agents that distinguishes genuine forensic investigation from alert parroting.
**Tools and scale:** Evaluating our SIR agent on the benchmark demonstrates 97.1% true positive (TP) detection, 73.4% false positive (FP) rejection, and 5.67 novel key findings per case, establishing a baseline against which future investigation agents can be measured.
**Results:** Our evaluation methodology introduces three complementary metrics: triage accuracy (M1), novel finding discovery (M2), and tool usage appropriateness (M3), assessed through an adversarial LLM-as-Judge that inverts the burden of proof -- requiring concrete forensic evidence to ... Evaluating our SIR agent on the benchmark demonstrates 97.1% true positive (TP) detection, 73.4% false positive (FP) rejection, and 5.67 novel key findings per case, establishing a baseline against which future investigation agents can be measured.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
