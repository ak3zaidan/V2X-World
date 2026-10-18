# Mobile-LLaMA: Instruction Fine-Tuning Open-Source LLM for Network Analysis in 5G Networks
Khen Bo Kan, Hyunsu Mun, Guohong Cao, Youngseok Lee · IEEE Network 2024 · https://doi.org/10.1109/mnet.2024.3421306 · relevance: medium · basis: abstract

**Problem:** In the evolving landscape of 5G networks, Network Data Analytics Function (NWDAF) emerges as a key component, interacting with core network elements to enhance data collection, model training, and analytical outcomes. Language Models (LLMs), with their state-of-the-art capabilities in natural language processing, have been successful in numerous fields.
**Approach:** In this paper, we present an open-source mobile network-specialized LLM, Mobile-LLaMA, which is an instruction-fine-tuned variant of the LLaMA 2 13B model. We build Mobile-LLaMA by instruction fine-tuning LLaMA 2 13B with our own network analysis data collected from publicly available, real-world 5G network datasets, and expanded its capabilities through a self-instruct framework utilizing OpenAI’s pre-trained models (PMs).
**Tools and scale:** Mentioned in abstract: 5G, LLM, GPT, real-world, field. (From the abstract only.)
**Results:** In particular, LLMs enhanced through instruction fine-tuning have demonstrated their effectiveness by employing sets of instructions to precisely tailor the model’s responses and behavior. Mobile-LLaMA has three main functions: packet analysis, IP routing analysis, and performance analysis, enabling it to provide network analysis and contribute to the automation and artificial intelligence (AI) required for 5G network management and data analysis. Our evaluation demonstrates Mobile-LLaMA’s proficiency in network analysis code generation, achieving a score of 247 out of 300, surpassing GPT-3.5’s score of 209.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM agents for network configuration/operations); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
