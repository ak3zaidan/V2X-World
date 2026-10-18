# How Can Driving World Models Do Counterfactual Prediction?
Jiaru Zhang, Can Cui, Yi Xu, Xin Ye, Ruqi Zhang, Ziran Wang · arXiv 2026 · https://arxiv.org/abs/2608.11601 · relevance: high · basis: full text (extracted sections)

**Problem:** Driving world models treat action-conditioned prediction as counterfactual prediction, ignoring the observed factual continuation.
**Approach:** Formalises the gap with abduction-action-prediction; builds a controlled simulation benchmark with factual and matched counterfactual outcomes; training-free evidence-transport pipeline.
**Tools and scale:** Vista and DrivingWorld world models; controlled simulation benchmark.
**Results:** Direct predictions fail to match counterfactual ground truth; simple pipeline substantially raises recovered fraction.
**Limitations and future work (as stated by the authors):**
- Short horizon where alternative ego action does not alter other agents.
**Overlap with our engine:** strong (conceptual) - builds a simulation benchmark with matched factual/counterfactual ground truth to test world models, the core idea of direction 3, but for driving video, not V2X networking or PKI.
**Novelty implications:** We cannot claim that a matched counterfactual-ground-truth simulation benchmark for world models is new in general. Open: networked, multi-agent, long-horizon counterfactuals where interventions propagate (their stated limitation) across traffic, radio and PKI.
