# Demo: Efficient and Timely Revocation of V2X Credentials
Gianluca Scopelliti, Christoph Baumann, Fritz Alder, Eddy Truyen, Jan Tobias Mühlberg · Proceedings Symposium on Vehicle Security &amp; Privacy 2024 · https://doi.org/10.14722/vehiclesec.2024.25002 · relevance: high · basis: full text (abstract, introduction, evaluation and conclusion excerpts)

**Problem:** V2X revocation lacks bounded revocation time under strong network attackers.
**Approach:** Interactive demo of a formally verified self-revocation scheme with upper bound on revocation time; revocation authority sends periodic messages.
**Tools and scale:** Simulated V2X scenario with virtual vehicles, attackers, delays and disruptions.
**Results:** Proof-of-concept; guaranteed upper bound on revocation time.
**Limitations and future work (as stated by the authors):**
- Demo only.
**Overlap with our engine:** strong — Bounded time to revoke misbehaving pseudonyms in simulation, directly overlapping our time-to-neutralise direction.
**Novelty implications:** Bounded revocation time via self-revocation exists; SCMS CRL/linkage-based time-to-neutralise with safety outcome still differs.
