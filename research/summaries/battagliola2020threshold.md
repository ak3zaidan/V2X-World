# Threshold ECDSA with an Offline Recovery Party
Michele Battagliola, Riccardo Longo, Alessio Meneghetti, Massimiliano Sala · Mediterr. J. Math. (2022) 19:4 2020 · https://doi.org/10.1007/s00009-021-01886-3 · relevance: low · basis: abstract

**Problem:** Under standard assumptions on the underlying algebraic and geometric problems (e.g. the Discrete Logarithm Problem for an elliptic curve and the computation of eth\documentclass[12pt]{minimal} \usepackage{amsmath} \usepackage{wasysym} \usepackage{amsfonts} \usepackage{amssymb} \usepackage{amsbsy} \usepackage{mathrsfs} \usepackage{upgreek} \setlength{\oddsidemargin}{-69pt} \begin{document}$$e\mathrm{th}$$\end{document} root on semi-prime residue rings), we prove our scheme secure against adaptive malicious adversaries.
**Approach:** Starting from the threshold scheme for the ECDSA signature due to Gennaro and Goldfeder, we present the first protocol that supports multiparty signatures with an offline participant during the key-generation phase and that does not rely on a trusted third party.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- "Our goal is to produce digital signatures that are compatible with an existing centralized signature scheme: the key-generation and signature algorithms are replaced by a communication protocol between the players, but the verification algorithm remains identical to that of a signature issued using "
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
