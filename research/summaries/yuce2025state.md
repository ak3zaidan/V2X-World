# State of the Art and First Synthetic Dataset for Misbehavior Detection with Collective Perception in [C-]V2X Networks*
Mehmet Fatih Yuce, Mehmet Ali Erturk, M. A. Aydın, Zeynep Gurkas-Aydin · IEEE International Symposium on Personal, Indoor and Mobile Radio Communications 2025 · https://doi.org/10.1109/pimrc62392.2025.11274651 · relevance: high · basis: abstract and dataset record (the paper is paywalled)

**Problem:** No public dataset for MBD in collective perception.
**Approach:** First synthetic CPM misbehaviour dataset with ST-GNN baseline and survey.
**Tools and scale:** Synthetic C-V2X CP scenarios; open-source CP framework integration discussed.
**Results:** Dataset and pipeline released (no figures in abstract).
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** strong — a public synthetic V2X MBD benchmark overlaps direction 3 (benchmark), though without counterfactual ground truth or PKI.
**Novelty implications:** We cannot claim first CPM MBD dataset; counterfactual and cross-layer ground truth remains open.
**Reviewer note (check-datasets-2026):** The Mendeley record 10.17632/n975twtvx9.2 says the GNN and tabular CPM sets are "generated from BurST-ADMA" by "adding clustering information". BurST-ADMA uses a traffic simulator only, with no network simulator, and contains only faulty-behaviour attacks (guven2025creating §2). So it has no radio layer. jerbi2026poster's abstract calls the only CPM misbehaviour dataset unreproducible and lacking precise message-level labels.
