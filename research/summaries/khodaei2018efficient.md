# Efficient, Scalable, and Resilient Vehicle-Centric Certificate Revocation List Distribution in VANETs
Mohammad Khodaei, Panos Papadimitratos · ACM WiSec, Stockholm, Sweden, June 2018, pp. 172--183 2018 · https://doi.org/10.1145/3212480.3212481 · relevance: high · basis: full text (extracted sections)

**Problem:** Efficient, private CRL distribution for pseudonyms under intermittent connectivity and polluting peers.
**Approach:** Vehicle-centric CRL distribution by region and trip duration with CRL-piece fingerprints (conference version of khodaei2020scalable).
**Tools and scale:** LuST dataset: 50x50 km, 138,259 vehicles, 287,939 trips, 4 h, 100 RSUs, 5.89 GHz, 18 Mbps; pseudonym lifetime 30-600 s; revocation rate 1%.
**Results:** With at most 25 KB/s, latest CRL reaches 95% of vehicles in a region within 15 s (journal reports >40x faster than state of the art).
**Limitations and future work (as stated by the authors):**
- Future: optimal CRL interval based on revocation frequency to narrow the vulnerability window.
**Overlap with our engine:** partial (near strong) - city-scale simulation of revocation dissemination with pseudonym lifetimes and RSUs; no traffic-safety coupling or US linkage-based CRL.
**Novelty implications:** City-scale CRL dissemination is studied; the 'vulnerability window' framing is stated by the authors as future work and relates to our 'time to neutralise' direction, which must cite this.
