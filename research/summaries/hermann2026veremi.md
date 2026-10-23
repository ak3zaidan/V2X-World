# VeReMi NextGen: A Dataset for Evaluating Misbehavior Detection Systems in VANETs
Artur Hermann, Jan-Niklas Remmers, Dennis Eisermann, Benjamin Erb, Frank Kargl · IEEE Vehicular Networking Conference 2026 · https://doi.org/10.18725/oparu-60763 · relevance: high · basis: full text

**Problem:** Existing MBD datasets have outdated scenarios (LuST), limited attack diversity and no train/val/test splits.
**Approach:** Eclipse MOSAIC 25.0 + SUMO 1.22 + OMNeT++ 5.5.1 on InTAS (Ingolstadt), urban and highway, 2 AM/7 AM densities, three driver profiles (80/10/10%), the VeReMi Extension sensor-error and pseudonym models, 15 attacks (6 new) at 20% attackers, **injected in post-processing** of a baseline run (§V). The per-message `attacker` flag is a **significance-threshold label** (0 for modified messages below the threshold, §V).
**Tools and scale:** 180 subsets, 197 GB, JSON per vehicle (Listing 1: `sender_id`, `sender_alias`, embedded receiver/sender noise, `distance_to_road_edge`), Zenodo 19665762, CC BY 4.0.
**Results:** CaTch, tuned for F1 per scenario: Feigned Braking F1 0.03–0.07, Acceleration Multiplication 0.05–0.12, Sudden Stop falls from 0.69 (Extension) to 0.02 (Tables II–III).
**Limitations and future work (as stated by the authors):**
- Synthetic; hypothetical attack models; simplified sensor-error model (§VIII-B).
- "cannot capture feedback loops"; MDSs with feedback "must be evaluated in running simulations"; closed-loop extension planned (§VIII-B, §IX).
- No RSSI ("not available in the current MOSAIC version", §IX); VRUs future.
**Overlap with our engine:** strong — the field's current reference benchmark; open-loop, with no radio, PKI or impact labels.
**Novelty implications:** We cannot claim the first split/extensible MBD dataset. Muzammil (muzammil2026code §X) finds the alias leak fixed but attacker `sender_id`s > 1,000,078 (disjoint from benign ones), benign identities that never rotate and message-count AUC 0.972–0.996. The text and Table III disagree on the comparison attack (Random Position vs Random Speed Offset). See synthesis/check-datasets-2026.md §1.2.
