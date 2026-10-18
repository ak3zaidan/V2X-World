# A Robust Framework for Sybil Attack Detection in Vehicular Ad Hoc Networks
Md. Sadmin Tahmid Khan, Md. Saim Ahmmed Utsho, Mosarrat Jahan · arXiv (Cornell University) 2026 · https://arxiv.org/abs/2606.11667 · relevance: high · basis: full text

**Problem:** Sybil detection suffers high FPR, manual calibration, RSU dependence and overhead.
**Approach:** RSU-verified GPS trajectories clustered with DBSCAN to find Sybil vehicles.
**Tools and scale:** SUMO on a 35 km x 25 km OpenStreetMap area of Dhaka; 160-200 vehicles dense, 30-60 sparse; 10-20% malicious with 1-10 Sybils each.
**Results:** FPR reduced by almost 72% in dense and sparse regions; FNR down 52% in sparse with 99% detection; detection time down about 76% in dense.
**Limitations and future work (as stated by the authors):**
- Extend to untrusted RSUs.
- More advanced location verification and better modelling of complex movement patterns.
**Overlap with our engine:** partial — Sybil detection on real map with SUMO.
**Novelty implications:** No impact.
