# NetGPT: Generative Pretrained Transformer for Network Traffic
Xu-Ying Meng, Chun-Chen Lin, Yequan Wang, Yu-Jun Zhang · arXiv.org 2023 · https://arxiv.org/abs/2304.09513 · relevance: medium · basis: abstract

**Problem:** All data on the Internet are transferred by network traffic, thus accurately modeling network traffic can help improve network services quality and protect data privacy. Pretrained models for network traffic can utilize large-scale raw data to learn the essential characteristics of network traffic, and generate distinguishable results for input traffic without considering specific downstream tasks.
**Approach:** Considering the diverse demands and characteristics of network traffic and network tasks, it is non-trivial to build a pretrained model for network traffic and we face various challenges, especially the heterogeneous headers and payloads in the multi-pattern network traffic and the different dependencies for contexts of diverse downstream network tasks. To tackle these challenges, in this paper, we make the first attempt to provide a generative pretrained model NetGPT for both traffic understanding and generation tasks.
**Tools and scale:** Mentioned in abstract: GPT, field. (From the abstract only.)
**Results:** With diverse traffic datasets from encrypted software, DNS, private industrial protocols and cryptocurrency mining, expensive experiments demonstrate the effectiveness of our NetGPT in a range of traffic understanding and generation tasks on traffic datasets, and outperform state-of-the-art baselines by a wide margin.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for network traffic); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
