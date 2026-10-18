# GIA: LLM-Enabled Generative Intent Abstraction to Enhance Adaptability for Intent-Driven Networks
Shiwen Kou, Chungang Yang, Mohan Gurusamy · IEEE Transactions on Cognitive Communications and Networking 2025 · https://doi.org/10.1109/tccn.2025.3545589 · relevance: medium · basis: abstract

**Problem:** Conventional intent-driven networks (IDNs) rely on static templates to translate declarative intents into imperative network configurations. However, the dynamic nature of network environments and evolving user requirements pose significant challenges to these inflexible approaches.
**Approach:** To overcome these limitations, we introduce a Generative Intent Abstraction (GIA) framework, which leverages LLMs with knowledge-enhanced prompting techniques to dynamically generate adaptable intent graphs by compelling the LLMs to write Python codes. Furthermore, we reformulate the intent negotiation into a subgraph isomorphism problem (SIP), ensuring the generated graphs continuously comply with the network capabilities.
**Tools and scale:** Mentioned in abstract: LLM. (From the abstract only.)
**Results:** Experimental results on the network intent dataset from easy and medium levels to hard and previously unseen levels demonstrate the effectiveness and adaptability of the proposed GIA framework. It significantly outperforms traditional methods without knowledge augmentation, achieving more than 30% higher F1 score for medium-level intents and a significant 180% increase for previously unseen intents.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM agents for network configuration/operations); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
