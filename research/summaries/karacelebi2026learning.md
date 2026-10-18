# Learning Ad Hoc Network Dynamics via Graph-Structured World Models
Can Karacelebi, Yusuf Talha Sahin, Elif Surer, Ertan Onur · arXiv 2026 · https://arxiv.org/abs/2604.14811 · relevance: medium · basis: abstract

**Problem:** Ad hoc network dynamics are hard to model; model-free RL needs online interaction.
**Approach:** G-RSSM graph-structured recurrent state-space world model; policy trained by imagined rollouts.
**Tools and scale:** 27 scenarios across MANET, VANET, FANET, WSN, tactical; 30 to 1000 nodes.
**Results:** Policy trained at N=50 keeps high connectivity at other sizes.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — graph world model including VANETs (direction 3), but for topology/clustering, not security or safety.
**Novelty implications:** A learned VANET world model is claimed; claims of the "first VANET world model" are ruled out.
