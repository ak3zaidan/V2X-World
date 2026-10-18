# Blockchain-based Truststore and CA Management for Intelligent Transportation Systems
Hu Liu, Salabat Khan, Mansoor Khan, Liu Yuan et al. · International Conference on Computer and Communications (ICCC) 2025 · https://doi.org/10.1109/iccc68654.2025.11438157 · relevance: high · basis: abstract

**Problem:** US and EU VPKIs inherit X.509 hierarchical trust that fits ITS poorly; CA updates and revocation are slow.
**Approach:** Blockchain-based truststore and CA management that replaces signature verification of trust anchors with a hash lookup. (summarised from the abstract only)
**Tools and scale:** Comparison with US and EU VPKI models on verification operations, time and storage.
**Results:** Reduced CA update and revocation time (smaller vulnerability window), efficient and scalable versus standard VPKIs (no numbers in abstract).
**Limitations and future work (as stated by the authors):**
- Not stated.
**Overlap with our engine:** partial — compares against the US SCMS trust model (root/elector, CA revocation) that our engine implements with real trust lists.
**Novelty implications:** Minor; blockchain alternative to SCMS trust lists, not a simulation of it.
