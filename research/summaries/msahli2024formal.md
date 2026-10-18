# Formal Analysis of C-ITS PKI Protocols
Mounira Msahli, Pascal Lafourcade, Dhekra Mahmoud · International Conference on Security and Cryptography 2024 · https://doi.org/10.5220/0012766100003767 · relevance: high · basis: abstract

**Problem:** C-ITS PKI protocols (ETSI) were not formally verified.
**Approach:** ProVerif symbolic model of ETSI C-ITS PKI enrolment, authorisation and validation; UKano for enrolment privacy. (summarised from the abstract only)
**Tools and scale:** Formal verification.
**Results:** Found authentication attacks in Authorisation and Validation with a dishonest Authorisation Authority; proposed fixes.
**Limitations and future work (as stated by the authors):**
- Not stated in the available abstract.
**Overlap with our engine:** partial — European C-ITS PKI protocols correspond to our SCMS provisioning flows; they verify, we simulate.
**Novelty implications:** Formal results apply to ETSI PKI; a formal analysis of US SCMS butterfly provisioning would be a separate question to check elsewhere.
