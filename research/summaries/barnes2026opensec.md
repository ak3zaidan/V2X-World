# OpenSec: Measuring Incident Response Agent Calibration Under Adversarial Evidence
Jarrod Barnes · arXiv 2026 · https://arxiv.org/abs/2601.21083 · relevance: medium · basis: abstract

**Problem:** As large language models (LLMs) improve, so do their offensive applications: frontier agents now generate working exploits for under $50 in compute (Heelan, 2026). Defensive incident response (IR) agents must keep pace, but existing benchmarks conflate action execution with correct execution, hiding calibration failures when agents process adversarial ...
**Approach:** We introduce OpenSec, a dual-control reinforcement learning (RL) environment that evaluates IR agents under realistic prompt injection scenarios with execution-based scoring: time-to-first-containment (TTFC), evidence-gated action rate (EGAR), blast radius, and per-tier injection violation rates.
**Tools and scale:** Not stated in the abstract.
**Results:** Claude Sonnet 4.5 shows partial calibration (62.5% containment, 45% FP, TTFC of 10.6), suggesting calibration is not reliably present across frontier models. Code available at https://github.com/jbarnes850/opensec-env.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
