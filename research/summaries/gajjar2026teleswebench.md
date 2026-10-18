# TeleSWEBench: A Commit-Driven Benchmark for Evaluating LLM-Powered Software Engineering in Telecommunications
Pranshav Gajjar, Ali Mamaghani, Dinesh Bharadia, V. K. Shah · arXiv.org 2026 · https://arxiv.org/abs/2606.05001 · relevance: medium · basis: abstract

**Problem:** With the telecommunications field embracing zero touch management alongside novel O-RAN and AI-RAN frameworks, contemporary telecom networks now function as immensely intricate and heavily softwareized codebases. While automated software engineering (ASE) tools and Software Engineering (SWE) Agents hold the potential to alleviate the critical code ...
**Approach:** In this paper, we introduce TeleSWEBench, the first commit-driven benchmark specifically designed to measure an agent's performance in the telecom domain.
**Tools and scale:** While automated software engineering (ASE) tools and Software Engineering (SWE) Agents hold the potential to alleviate the critical code generation bottleneck in this domain, their ability to navigate and modify specialized, mathematically rigorous wireless stacks like srsRAN 5G remains unverified.
**Results:** Using this benchmark, we evaluate AIDER, OpenHands, and the ClaudeCode frameworks, powered by state-of-the-art reasoning LLMs, including Qwen3, GPT OSS, Gemma 4, Kimi, and Qwencoder 2.5. Our two-stage evaluation reveals that models suffer from a lack of both localization accuracy and functional correctness, with the strongest ASE tools achieving up to 25% of shippable changes.
**Limitations and future work (as stated by the authors):**
- Our two-stage evaluation reveals that models suffer from a lack of both localization accuracy and functional correctness, with the strongest ASE tools achieving up to 25% of shippable changes.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
