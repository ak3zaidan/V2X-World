# Search log: cluster `crypto`

Scope: post-quantum, hybrid and crypto-agile V2X security; threshold/distributed CAs; signature verification load, batch verification, verify-on-demand; cryptographic overhead; HSM timing.

Final records: **2189** (high 579, medium 855, low 755).

Notes: Run 2026-10-17. OpenAlex: the shared no-key daily budget was exhausted after the first query (HTTP 429, "Insufficient budget", resets at midnight UTC), so only one OpenAlex search ran and no OpenAlex enrichment was possible; open-access links come from arXiv and Semantic Scholar `openAccessPdf`. DBLP: every request returned a bot-challenge page (HTTP 202, Anubis), so DBLP could not be queried (its titles are covered by Crossref/S2). Semantic Scholar relevance search hit persistent 429s, so S2 was queried with the bulk boolean endpoint (`/paper/search/bulk`, unranked, up to 6,000-7,000 results per query; some broad queries such as bare `FROST`, `1609.2`, `SCMS`, `lattice` matched unrelated fields and were re-run as narrower queries). arXiv was queried with every free-text query (AND of terms over all fields). Crossref was queried with every free-text query (60 rows, relevance ranked). Screening: all candidates were deduplicated (DOI, arXiv id, S2 id, normalised title), enriched with S2 abstracts/citations/OA PDFs by DOI/arXiv id, then screened by a rule-based classifier on title+abstract (vehicular context x PQ/hybrid/threshold primitives, or x verification-load/overhead/HSM/1609.2 terms). Records without an abstract were screened on title only. Relevance labels are automatic and should be re-checked during summarisation. Generic VANET authentication schemes (CPPA, certificateless, aggregate signatures) are kept as medium/low background because they meet the protocol's inclusion criteria.

## Queries

| # | Source | Query | Date | Hits (total reported) | Retrieved | Kept (after dedup + screening, credited to every query that found it) |
|---|---|---|---|---|---|---|
| 1 | openalex | post-quantum V2X | 2026-10-17 | 1274 | 100 | 69 |
| 2 | arxiv | post-quantum V2X | 2026-10-17 | 11 | 11 | 11 |
| 3 | s2 | post-quantum V2X | 2026-10-17 | 24988 | 100 | 50 |
| 4 | arxiv | post-quantum vehicular communication | 2026-10-17 | 9 | 9 | 9 |
| 5 | arxiv | post-quantum VANET authentication | 2026-10-17 | 1 | 1 | 1 |
| 6 | arxiv | quantum-resistant V2X security | 2026-10-17 | 6 | 6 | 6 |
| 7 | arxiv | lattice-based signature vehicular ad hoc network | 2026-10-17 | 0 | 0 | 0 |
| 8 | arxiv | Dilithium V2X | 2026-10-17 | 3 | 3 | 3 |
| 9 | arxiv | Falcon signature vehicular | 2026-10-17 | 1 | 1 | 1 |
| 10 | arxiv | SPHINCS+ vehicular | 2026-10-17 | 2 | 2 | 2 |
| 11 | arxiv | Kyber vehicle key encapsulation | 2026-10-17 | 2 | 2 | 2 |
| 12 | arxiv | ML-DSA automotive | 2026-10-17 | 0 | 0 | 0 |
| 13 | arxiv | post-quantum IEEE 1609.2 | 2026-10-17 | 1 | 1 | 1 |
| 14 | arxiv | post-quantum C-V2X | 2026-10-17 | 2 | 2 | 2 |
| 15 | arxiv | post-quantum pseudonym certificates SCMS | 2026-10-17 | 2 | 2 | 2 |
| 16 | arxiv | post-quantum butterfly key expansion | 2026-10-17 | 2 | 2 | 1 |
| 17 | arxiv | hybrid post-quantum certificates vehicle | 2026-10-17 | 3 | 3 | 3 |
| 18 | arxiv | crypto-agility automotive | 2026-10-17 | 0 | 0 | 0 |
| 19 | arxiv | post-quantum cryptography automotive ECU | 2026-10-17 | 1 | 1 | 1 |
| 20 | arxiv | post-quantum C-ITS | 2026-10-17 | 38 | 38 | 1 |
| 21 | arxiv | post-quantum signature size V2X message fragmentation | 2026-10-17 | failed | 0 | 0 |
| 22 | arxiv | quantum computing threat connected vehicles security | 2026-10-17 | 4 | 4 | 4 |
| 23 | arxiv | post-quantum internet of vehicles | 2026-10-17 | 0 | 0 | 0 |
| 24 | arxiv | lattice-based authentication internet of vehicles | 2026-10-17 | 0 | 0 | 0 |
| 25 | arxiv | NTRU vehicular network | 2026-10-17 | 0 | 0 | 0 |
| 26 | arxiv | threshold signature certificate authority | 2026-10-17 | 3 | 3 | 2 |
| 27 | arxiv | FROST threshold Schnorr signatures | 2026-10-17 | 2 | 2 | 1 |
| 28 | arxiv | threshold ECDSA | 2026-10-17 | 9 | 9 | 3 |
| 29 | arxiv | distributed certificate authority vehicular network | 2026-10-17 | 6 | 6 | 4 |
| 30 | arxiv | threshold cryptography VANET | 2026-10-17 | 3 | 3 | 2 |
| 31 | arxiv | multi-party computation certificate authority PKI | 2026-10-17 | 0 | 0 | 0 |
| 32 | arxiv | threshold signature vehicular revocation | 2026-10-17 | 2 | 2 | 2 |
| 33 | arxiv | distributed key generation vehicular | 2026-10-17 | 39 | 39 | 4 |
| 34 | arxiv | batch verification VANET signatures | 2026-10-17 | 3 | 3 | 3 |
| 35 | arxiv | batch verification ECDSA | 2026-10-17 | 0 | 0 | 0 |
| 36 | arxiv | signature verification load V2X | 2026-10-17 | 0 | 0 | 0 |
| 37 | arxiv | verify on demand V2X | 2026-10-17 | 0 | 0 | 0 |
| 38 | arxiv | selective signature verification VANET | 2026-10-17 | 0 | 0 | 0 |
| 39 | arxiv | cryptographic overhead V2X performance | 2026-10-17 | 0 | 0 | 0 |
| 40 | arxiv | security overhead DSRC packet delivery ratio | 2026-10-17 | 0 | 0 | 0 |
| 41 | arxiv | authentication delay safety messages VANET | 2026-10-17 | 1 | 1 | 1 |
| 42 | arxiv | IEEE 1609.2 performance evaluation | 2026-10-17 | 2 | 2 | 2 |
| 43 | arxiv | ECDSA verification hardware acceleration V2X | 2026-10-17 | 0 | 0 | 0 |
| 44 | arxiv | hardware security module V2X | 2026-10-17 | 0 | 0 | 0 |
| 45 | arxiv | V2X security processing latency C-V2X | 2026-10-17 | 2 | 2 | 0 |
| 46 | arxiv | certificate digest omission V2X channel load | 2026-10-17 | 0 | 0 | 0 |
| 47 | arxiv | implicit certificates ECQV V2X | 2026-10-17 | 2 | 2 | 2 |
| 48 | arxiv | signature flooding denial of service VANET verification | 2026-10-17 | 0 | 0 | 0 |
| 49 | arxiv | vehicular public key infrastructure scalability performance | 2026-10-17 | 4 | 4 | 3 |
| 50 | arxiv | ETSI ITS security overhead CAM | 2026-10-17 | 0 | 0 | 0 |
| 51 | arxiv | post-quantum ISO 15118 vehicle-to-grid | 2026-10-17 | 0 | 0 | 0 |
| 52 | arxiv | post-quantum TLS embedded performance | 2026-10-17 | 3 | 3 | 1 |
| 53 | arxiv | hybrid signatures PKI migration post-quantum | 2026-10-17 | 1 | 1 | 0 |
| 54 | arxiv | quantum-safe X.509 hybrid certificates | 2026-10-17 | 0 | 0 | 0 |
| 55 | arxiv | lattice-based group signature vehicular | 2026-10-17 | 0 | 0 | 0 |
| 56 | arxiv | post-quantum CAN bus authentication | 2026-10-17 | 0 | 0 | 0 |
| 57 | arxiv | GPU signature verification vehicular | 2026-10-17 | 0 | 0 | 0 |
| 58 | arxiv | post-quantum platoon | 2026-10-17 | 1 | 1 | 0 |
| 59 | arxiv | SCMS security credential management system performance | 2026-10-17 | 7 | 7 | 5 |
| 60 | arxiv | quantum key distribution vehicular network | 2026-10-17 | 4 | 4 | 4 |
| 61 | arxiv | post-quantum certificate revocation vehicular | 2026-10-17 | 0 | 0 | 0 |
| 62 | arxiv | threshold signature internet of vehicles | 2026-10-17 | 0 | 0 | 0 |
| 63 | crossref | post-quantum V2X | 2026-10-17 | 1849957 | 60 | 20 |
| 64 | crossref | post-quantum vehicular | 2026-10-17 | 1939474 | 60 | 37 |
| 65 | crossref | quantum-resistant vehicular authentication | 2026-10-17 | 1204230 | 60 | 23 |
| 66 | crossref | post-quantum automotive | 2026-10-17 | 1936703 | 60 | 13 |
| 67 | crossref | lattice-based VANET | 2026-10-17 | 6846149 | 60 | 14 |
| 68 | crossref | threshold certificate authority vehicular | 2026-10-17 | 310426 | 60 | 29 |
| 69 | crossref | batch verification VANET | 2026-10-17 | 191436 | 60 | 22 |
| 70 | crossref | V2X security overhead latency | 2026-10-17 | 855385 | 60 | 4 |
| 71 | crossref | IEEE 1609.2 performance | 2026-10-17 | 6416266 | 60 | 0 |
| 72 | crossref | hardware security module V2X | 2026-10-17 | 982006 | 60 | 4 |
| 73 | crossref | post-quantum IEEE 1609.2 | 2026-10-17 | 5958010 | 60 | 3 |
| 74 | crossref | crypto agility vehicle | 2026-10-17 | 319259 | 60 | 19 |
| 75 | crossref | verify on demand V2X | 2026-10-17 | 186968 | 60 | 1 |
| 76 | crossref | post-quantum V2X | 2026-10-17 | 1849957 | 60 | 20 |
| 77 | crossref | post-quantum vehicular communication | 2026-10-17 | 3241725 | 60 | 28 |
| 78 | crossref | post-quantum VANET authentication | 2026-10-17 | 1902898 | 60 | 20 |
| 79 | crossref | quantum-resistant V2X security | 2026-10-17 | 1854951 | 60 | 13 |
| 80 | crossref | lattice-based signature vehicular ad hoc network | 2026-10-17 | 8187737 | 60 | 14 |
| 81 | crossref | Dilithium V2X | 2026-10-17 | 5505 | 60 | 0 |
| 82 | crossref | Falcon signature vehicular | 2026-10-17 | 178210 | 60 | 17 |
| 83 | crossref | SPHINCS+ vehicular | 2026-10-17 | 94585 | 60 | 4 |
| 84 | crossref | Kyber vehicle key encapsulation | 2026-10-17 | 786918 | 60 | 2 |
| 85 | crossref | ML-DSA automotive | 2026-10-17 | 143994 | 60 | 3 |
| 86 | crossref | post-quantum IEEE 1609.2 | 2026-10-17 | 5958010 | 60 | 3 |
| 87 | crossref | post-quantum C-V2X | 2026-10-17 | 1849957 | 60 | 20 |
| 88 | crossref | post-quantum pseudonym certificates SCMS | 2026-10-17 | 1852798 | 60 | 19 |
| 89 | crossref | post-quantum butterfly key expansion | 2026-10-17 | 2501310 | 60 | 2 |
| 90 | crossref | hybrid post-quantum certificates vehicle | 2026-10-17 | 2847111 | 60 | 15 |
| 91 | crossref | crypto-agility automotive | 2026-10-17 | 115470 | 60 | 18 |
| 92 | crossref | post-quantum cryptography automotive ECU | 2026-10-17 | 1979657 | 60 | 10 |
| 93 | crossref | post-quantum C-ITS | 2026-10-17 | 3830015 | 60 | 1 |
| 94 | crossref | post-quantum signature size V2X message fragmentation | 2026-10-17 | 2490918 | 60 | 12 |
| 95 | crossref | quantum computing threat connected vehicles security | 2026-10-17 | 3123131 | 60 | 2 |
| 96 | crossref | post-quantum internet of vehicles | 2026-10-17 | 2459368 | 60 | 31 |
| 97 | crossref | lattice-based authentication internet of vehicles | 2026-10-17 | 7396779 | 60 | 26 |
| 98 | crossref | NTRU vehicular network | 2026-10-17 | 1439521 | 60 | 5 |
| 99 | crossref | threshold signature certificate authority | 2026-10-17 | 292140 | 60 | 8 |
| 100 | crossref | FROST threshold Schnorr signatures | 2026-10-17 | 230615 | 60 | 27 |
| 101 | crossref | threshold ECDSA | 2026-10-17 | 113114 | 60 | 33 |
| 102 | crossref | distributed certificate authority vehicular network | 2026-10-17 | 1912837 | 60 | 28 |
| 103 | crossref | threshold cryptography VANET | 2026-10-17 | 163600 | 60 | 3 |
| 104 | crossref | multi-party computation certificate authority PKI | 2026-10-17 | 1813036 | 60 | 2 |
| 105 | crossref | threshold signature vehicular revocation | 2026-10-17 | 286005 | 60 | 15 |
| 106 | crossref | distributed key generation vehicular | 2026-10-17 | 1679719 | 60 | 1 |
| 107 | crossref | batch verification VANET signatures | 2026-10-17 | 261620 | 60 | 17 |
| 108 | crossref | batch verification ECDSA | 2026-10-17 | 186713 | 60 | 9 |
| 109 | crossref | signature verification load V2X | 2026-10-17 | 465469 | 60 | 2 |
| 110 | crossref | verify on demand V2X | 2026-10-17 | 186968 | 60 | 1 |
| 111 | crossref | selective signature verification VANET | 2026-10-17 | 632340 | 60 | 17 |
| 112 | crossref | cryptographic overhead V2X performance | 2026-10-17 | 2418098 | 60 | 3 |
| 113 | crossref | security overhead DSRC packet delivery ratio | 2026-10-17 | 1538385 | 60 | 0 |
| 114 | crossref | authentication delay safety messages VANET | 2026-10-17 | 1117992 | 60 | 9 |
| 115 | crossref | IEEE 1609.2 performance evaluation | 2026-10-17 | 8288448 | 60 | 0 |
| 116 | crossref | ECDSA verification hardware acceleration V2X | 2026-10-17 | 286818 | 60 | 2 |
| 117 | crossref | hardware security module V2X | 2026-10-17 | 982006 | 60 | 4 |
| 118 | crossref | V2X security processing latency C-V2X | 2026-10-17 | 2242650 | 60 | 4 |
| 119 | crossref | certificate digest omission V2X channel load | 2026-10-17 | 804907 | 60 | 11 |
| 120 | crossref | implicit certificates ECQV V2X | 2026-10-17 | 64705 | 60 | 13 |
| 121 | crossref | signature flooding denial of service VANET verification | 2026-10-17 | 1001717 | 60 | 2 |
| 122 | crossref | vehicular public key infrastructure scalability performance | 2026-10-17 | 4750399 | 60 | 2 |
| 123 | crossref | ETSI ITS security overhead CAM | 2026-10-17 | 2847098 | 60 | 3 |
| 124 | crossref | post-quantum ISO 15118 vehicle-to-grid | 2026-10-17 | 2422210 | 60 | 2 |
| 125 | crossref | post-quantum TLS embedded performance | 2026-10-17 | 4381049 | 60 | 46 |
| 126 | crossref | hybrid signatures PKI migration post-quantum | 2026-10-17 | 2983508 | 60 | 31 |
| 127 | crossref | quantum-safe X.509 hybrid certificates | 2026-10-17 | 1763334 | 60 | 9 |
| 128 | crossref | lattice-based group signature vehicular | 2026-10-17 | 7929595 | 60 | 22 |
| 129 | crossref | post-quantum CAN bus authentication | 2026-10-17 | 2915392 | 60 | 13 |
| 130 | crossref | GPU signature verification vehicular | 2026-10-17 | 330192 | 60 | 14 |
| 131 | crossref | post-quantum platoon | 2026-10-17 | 1848199 | 60 | 2 |
| 132 | crossref | SCMS security credential management system performance | 2026-10-17 | 10454069 | 60 | 10 |
| 133 | crossref | quantum key distribution vehicular network | 2026-10-17 | 3498679 | 60 | 4 |
| 134 | crossref | post-quantum certificate revocation vehicular | 2026-10-17 | 1953009 | 60 | 22 |
| 135 | crossref | threshold signature internet of vehicles | 2026-10-17 | 805767 | 60 | 22 |
| 136 | s2-bulk | ("post-quantum" | "postquantum" | "quantum-resistant" | "quantum-safe" | "quantum-secure") + (V2X | VANET | VANETs | vehicular | "connected vehicles" | "connected vehicle" | C-ITS | automotive | "internet of vehicles" | "vehicle-to-vehicle" | "vehicle-to-everything" | C-V2X | DSRC | "intelligent transportation") | 2026-10-17 | 406 | 406 | 301 |
| 137 | s2-bulk | (lattice | "lattice-based" | NTRU | "learning with errors" | "ring-LWE") + (V2X | VANET | VANETs | vehicular | "connected vehicles" | "connected vehicle" | C-ITS | automotive | "internet of vehicles" | "vehicle-to-vehicle" | "vehicle-to-everything" | C-V2X | DSRC | "intelligent transportation") | 2026-10-17 | 21666 | 6999 | 52 |
| 138 | s2-bulk | (Dilithium | "ML-DSA" | Falcon | SPHINCS | "SLH-DSA" | Kyber | "ML-KEM" | XMSS | "hash-based signature") + (V2X | VANET | VANETs | vehicular | "connected vehicles" | "connected vehicle" | C-ITS | automotive | "internet of vehicles" | "vehicle-to-vehicle" | "vehicle-to-everything" | C-V2X | DSRC | "intelligent transportation") | 2026-10-17 | 569 | 569 | 62 |
| 139 | s2-bulk | ("crypto-agility" | "cryptographic agility" | "crypto agility" | "hybrid signature" | "hybrid certificate" | "composite certificate") + (vehicle | vehicular | automotive | V2X | PKI) | 2026-10-17 | 44 | 44 | 29 |
| 140 | s2-bulk | ("quantum computing" | "quantum computer" | "quantum attack" | "quantum threat" | "quantum key distribution") + (V2X | VANET | VANETs | vehicular | "connected vehicles" | "connected vehicle" | C-ITS | automotive | "internet of vehicles" | "vehicle-to-vehicle" | "vehicle-to-everything" | C-V2X | DSRC | "intelligent transportation") | 2026-10-17 | 853 | 853 | 271 |
| 141 | s2-bulk | ("threshold signature" | "threshold signatures" | "threshold cryptography" | "threshold ECDSA" | "secret sharing" | "distributed key generation" | "multi-party computation") + (V2X | VANET | VANETs | vehicular | "connected vehicles" | "connected vehicle" | C-ITS | automotive | "internet of vehicles" | "vehicle-to-vehicle" | "vehicle-to-everything" | C-V2X | DSRC | "intelligent transportation") | 2026-10-17 | 304 | 304 | 164 |
| 142 | s2-bulk | ("distributed certificate authority" | "threshold certificate authority" | "distributed CA" | "decentralized PKI" | "threshold CA") | 2026-10-17 | 493 | 493 | 38 |
| 143 | s2-bulk | ("threshold ECDSA" | "threshold Schnorr" | FROST | "two-party ECDSA" | "multiparty ECDSA") | 2026-10-17 | 65417 | 6999 | 13 |
| 144 | s2-bulk | ("batch verification" | "batch verify" | "aggregate signature" | "aggregate signatures") + (V2X | VANET | VANETs | vehicular | "connected vehicles" | "connected vehicle" | C-ITS | automotive | "internet of vehicles" | "vehicle-to-vehicle" | "vehicle-to-everything" | C-V2X | DSRC | "intelligent transportation") | 2026-10-17 | 393 | 393 | 312 |
| 145 | s2-bulk | ("signature verification" | "verification overhead" | "verification delay" | "verification load" | "verify-on-demand" | "verify on demand" | "selective verification") + (V2X | VANET | VANETs | vehicular | "connected vehicles" | "connected vehicle" | C-ITS | automotive | "internet of vehicles" | "vehicle-to-vehicle" | "vehicle-to-everything" | C-V2X | DSRC | "intelligent transportation") | 2026-10-17 | 270 | 270 | 153 |
| 146 | s2-bulk | ("cryptographic overhead" | "security overhead" | "authentication overhead" | "authentication delay" | "computation overhead" | "processing delay") + (V2X | VANET | "IEEE 1609.2" | DSRC | C-ITS | "C-V2X") | 2026-10-17 | 673 | 673 | 225 |
| 147 | s2-bulk | ("IEEE 1609.2" | "1609.2" | "security credential management system" | SCMS | "butterfly key" | "pseudonym certificate" | "pseudonym certificates") | 2026-10-17 | 49102 | 6000 | 15 |
| 148 | s2-bulk | ("hardware security module" | HSM | "cryptographic accelerator" | "hardware accelerator" | FPGA | ASIC | GPU) + (V2X | VANET | "IEEE 1609.2" | C-ITS | ECDSA) + (vehicle | vehicular | V2X) | 2026-10-17 | 181 | 181 | 18 |
| 149 | s2-bulk | ("implicit certificate" | ECQV | "certificate digest" | "certificate omission" | "message size" | fragmentation) + (V2X | VANET | "IEEE 1609.2" | "C-V2X" | "vehicular") | 2026-10-17 | 357 | 357 | 46 |
| 150 | s2-bulk | ("denial of service" | DoS | flooding) + ("signature verification" | "certificate verification" | authentication) + (V2X | VANET | vehicular) | 2026-10-17 | 199 | 199 | 42 |
| 151 | s2-bulk | ("vehicular public key infrastructure" | VPKI | "vehicular PKI" | "V2X PKI" | "C-ITS PKI" | CCMS) | 2026-10-17 | 30840 | 6000 | 10 |
| 152 | s2-bulk | ("post-quantum" | "quantum-resistant") + ("controller area network" | "CAN bus" | ECU | "in-vehicle network" | "ISO 15118" | "vehicle-to-grid" | "electric vehicle charging") | 2026-10-17 | 53 | 53 | 52 |
| 153 | s2-bulk | ("post-quantum" | "quantum-resistant") + (certificate | PKI | X.509 | "public key infrastructure") + (migration | hybrid | performance | embedded) | 2026-10-17 | 271 | 271 | 111 |
| 154 | s2-bulk | ("threshold ECDSA" | "threshold Schnorr" | "two-party ECDSA" | "multiparty ECDSA" | "FROST threshold" | "threshold EdDSA" | "threshold BLS") | 2026-10-17 | 184 | 184 | 70 |
| 155 | s2-bulk | ("IEEE 1609.2" | "security credential management system" | "butterfly key" | "butterfly keys" | "pseudonym certificate" | "pseudonym certificates") | 2026-10-17 | 227 | 227 | 116 |
| 156 | s2-bulk | ("vehicular public key infrastructure" | VPKI | "vehicular PKI" | "V2X PKI" | "C-ITS PKI" | "C-ITS security") | 2026-10-17 | 83 | 83 | 35 |
| 157 | s2-bulk | ("lattice-based" | NTRU | "learning with errors" | "ring-LWE" | "module-LWE") + (V2X | VANET | VANETs | vehicular | "connected vehicles" | "connected vehicle" | C-ITS | automotive | "internet of vehicles" | "vehicle-to-vehicle" | "vehicle-to-everything" | C-V2X | DSRC | "intelligent transportation") | 2026-10-17 | 321 | 321 | 161 |
| 158 | s2-bulk | ("threshold" + (signature | ECDSA | Schnorr)) + (PKI | "certificate authority" | "certification authority") | 2026-10-17 | 80 | 80 | 17 |

## Snowballing (Semantic Scholar references + citations)

### Round 1: 448 seeds, 8064 neighbours screened, 634 new records kept

| Seed | Neighbours | Added |
|---|---|---|
| Post-Quantum Impacts on V2X Certificates – Already at The End of The Road | 15 | 1 |
| qSCMS: Post-quantum certificate provisioning process for V2X. | 16 | 0 |
| Post-Quantum Era in V2X Security: Convergence of Orchestration and Parallel Computation | 32 | 4 |
| Post-Quantum Tokenized Security for C-V2X | 0 | 0 |
| Post-Quantum Cryptography Challenges in Connected Vehicles with V2X | 3 | 0 |
| Post-Quantum Cryptography Migration on V2X Certificate using KpqC Algorithms* | 2 | 0 |
| Post-Quantum PHY-Layer Authentication for Secure Initial Access in V2X Communications | 3 | 0 |
| Secure V2X Communication in the Quantum Era: A Survey of Post-Quantum Authentication and Key Agreement (AKA) Protocols for Autonomous Vehicles | 53 | 6 |
| DIMA: A Post-Quantum and Privacy-Preserving Decentralized Identity Management Architecture for Secure V2X Authentication | 3 | 0 |
| Post-Quantum Authentication Against Cyber-Physical Attacks in V2X-Based Autonomous Vehicle Platoon | 21 | 3 |
| A Post-Quantum Aware SCMS for V2X Communications Using Selective Cryptographic Methodology | 0 | 0 |
| An Efficient Post-Quantum Cross-Domain Authentication Scheme for 6G V2X Using Accelerated FrodoKEM | 0 | 0 |
| Q-Auth5G: A Low-Overhead Post-Quantum Authentication Protocol for 5G-V2X Systems | 0 | 0 |
| AI-Driven Post-Quantum Cryptography for Cyber-Resilient V2X Communication in Transportation Cyber-Physical Systems | 118 | 6 |
| Deployment Feasibility Analysis of Post-Quantum Digital Signatures in Safety-Critical C-V2X Communication for Urban Mobility Scenario | 39 | 1 |
| Analysis of outage performance in a 6G‐V2X communications system utilising free‐space optical quantum key distribution | 35 | 3 |
| QuanReS: A Hybrid Quantum-Enabled Resilient and Secure V2X Communications Framework in Smart Cities | 1 | 1 |
| QSSNET: A quantum-safe distributed trust network architecture for V2X authentication and secure communication | 1 | 0 |
| PQ-V2X: A Novel Postquantum Cryptographic Dataset for Secure Vehicular Communications | 9 | 0 |
| Post-quantum cryptography for automotive systems | 40 | 7 |
| A Lightweight Key Agreement Protocol for V2X Communications Based on Kyber and Saber | 50 | 4 |
| A POST-QUANTUM BLOCKCHAIN FRAMEWORK FOR CONNECTED SMART VEHICLES | 1 | 0 |
| A Quantum-Resilient Sharded Blockchain Framework for Secure V2X and Federated Learning in Intelligent Transportation Systems | 1 | 0 |
| Quantum Safe AI Pipelines Securing Next Generation Data Flows Against Post Quantum Threats | 0 | 0 |
| PQCMC: Post-Quantum Cryptography McEliece-Chen Implicit Certificate Scheme | 25 | 0 |
| Hybrid Scheme of Post-Quantum Cryptography and Elliptic-Curve Cryptography for Certificates ─ A Case Study of Security Credential Management System in | 40 | 1 |
| Unsettled Topics Concerning the Impact of Quantum Technologies on Automotive Cybersecurity | 5 | 0 |
| Post-quantum cryptography for intelligent transportation systems: An implementation-focused review | 101 | 10 |
| Feasibility and Benchmarking of Post-Quantum Cryptography in the Cooperative ITS Ecosystem | 16 | 2 |
| A Security Credential Management System for V2X Communications | 362 | 31 |
| QR-SLICE: Quantum-Resistant Split Learning Architecture for Intelligent 6G-Enabled C-V2X Ecosystems | 0 | 0 |
| Assessing the Cost of Quantum Security for Automotive Over -The-Air Updates | 17 | 1 |
| PQAKA: Post Quantum Authentication and Key Agreement Protocol for Intelligent Internet of Vehicles Over 5G | 39 | 4 |
| Post-quantum cryptography enhanced IP address management framework for secure smart transportation networks | 0 | 0 |
| Quantum-Resistant Secure Data Provenance in Internet of Vehicles | 1 | 0 |
| Adaptive Quantum-Safe Cryptography for 6G Vehicular Networks via Context-Aware Optimization | 64 | 6 |
| MISP: An Efficient Quantum-Resistant Misbehavior Preventing Scheme With Self-Enforcement for Vehicle-to-Everything | 4 | 0 |
| A Zero-Knowledge Signature Framework for Efficient Post-Quantum Message Authentication in Cooperative Automated Driving | 32 | 1 |
| Threshold Cryptography-Based Secure Vehicle-to-Everything (V2X) Communication in 5G-Enabled Intelligent Transportation Systems | 34 | 2 |
| Quantum Resistant Authentication and Key Agreement Protocol (AKA) for Autonomous Vehichle | 2 | 0 |
| Quantum Safe Vehicle Networks Using ML and IoT for Traffic Congestion Prediction | 0 | 0 |
| A Lightweight Post-Quantum Anonymous Attestation Framework for Traceable and Comprehensive Privacy Preservation in VANETs | 31 | 3 |
| Lattice-Based Lightweight Multi-Authentication Scheme for Vehicle-to-Everything Communication | 0 | 0 |
| PUF-Based Post-Quantum CAN-FD Framework for Vehicular Security | 17 | 1 |
| Experimental Evaluation of Post-Quantum Homomorphic Encryption for Privacy-Preserving V2X Communication | 1 | 0 |
| Identify Post-Quantum Cryptographic Algorithms for Automotive Security: Performance-Driven Guidance for Secure Boot, OTA, and V2X Communication | 0 | 0 |
| On the Implementation of a Lattice-Based Daa for VANET System | 40 | 1 |
| A Survey of Post-Quantum Cryptography Migration in Vehicles | 27 | 5 |
| Post-Quantum enhanced Ascon for secure vehicular IoT data integrity | 34 | 3 |
| When Cryptography Needs a Hand: Practical Post-Quantum Authentication for V2V Communications | 24 | 1 |
| Post-Quantum Anonymous, Traceable and Linkable Authentication Scheme Based on Blockchain for Intelligent Vehicular Transportation Systems | 24 | 3 |
| A Quantum-Resistant Key Management Scheme Using Blockchain in C-V2X | 24 | 3 |
| Pseudonym Scheme Based on Hybrid Certificates for Security Credential Management System in Vehicular Communications | 42 | 2 |
| Quantum Vanguard: Server Optimized Privacy Fortified Federated Intelligence for Future Vehicles | 36 | 1 |
| Quantum-Secure Certificate-Less Conditional Privacy-Preserving Authentication for VANET | 47 | 11 |
| Preparing for Kyber in Securing Intelligent Transportation Systems Communications: A Case Study on Fault-Enabled Chosen-Ciphertext Attack | 1 | 1 |
| Quantum-Resistant Safety Message Authentication for NextG C-V2X: A Cross-Layer Approach | 0 | 0 |
| QuantumShield V2X: A Continuous-Variable Quantum Key Distribution Framework for Eavesdropping-Resistant V2X Communications | 1 | 0 |
| Observability and Self Healing of Quantum Secure V2X communication in Edge-Cloud Systems | 3 | 0 |
| qSCMS: post-quantum security credential management system for vehicular communications. | 79 | 0 |
| The Performance Analysis of Post-Quantum Cryptography for Vehicular Communications | 5 | 1 |
| Quantum-resilient blockchain-enabled secure communication framework for connected autonomous vehicles using post-quantum cryptography | 46 | 5 |
| DAME-IoV: Dynamic Adaptive Multi-Edge authentication protocol with post-quantum security for Internet of Vehicles | 7 | 2 |
| An Efficient Post-Quantum Lattice-Based Encryption Scheme for Secure Communication in Vehicular Ad-Hoc Networks | 1 | 0 |
| A Study of Post-Quantum Digital Signatures in the Automotive Industry | 1 | 0 |
| Correction: Post-Quantum enhanced Ascon for secure vehicular IoT data integrity | 3 | 1 |
| QR-LBTS: Quantum-Resilient Lattice-Based Threshold Signatures for Efficient Post-Quantum VANETs | 0 | 0 |
| Post-Quantum Access Control for Collaborative Perception in Vehicular Edge Networks | 0 | 0 |
| Utilization of Quantum Random Numbers in Crystal-Kyber Based Post Quantum Virtual Private Network for Vehicular Communication | 4 | 0 |
| Post quantum secure conditional privacy preserving authentication for edge based vehicular communication | 22 | 2 |
| Optimizing Post-Quantum Secure Communication via DL-Based KEM Selection in VANETs | 0 | 0 |
| A Survey on Post-Quantum Public-Key Signature Schemes for Secure Vehicular Communications | 58 | 7 |
| A post-quantum enabled signcryption scheme for securing Vehicular Ad-Hoc Networks | 0 | 0 |
| A quantum-secure certificateless aggregate signature protocol for vehicular ad hoc networks | 10 | 0 |
| Post-Quantum DAG-Based Blockchain for Securing Vehicular Sensor Networks | 1 | 0 |
| Enhancing quantum-resistant data privacy in vehicular cloud networks using NIST-qualified FALCON algorithm | 3 | 0 |
| TransLLM-H: A Hybrid Federated–Decentralized Post-Quantum Framework for Semantic Trust in 6G Vehicular Networks | 0 | 0 |
| PQACLA: Post-Quantum Anonymous and Robust Certificateless Identity Authentication for the Internet of Vehicles | 0 | 0 |
| Quantum-safe UAV-assisted blockchain authentication for secure vehicle communications in ITS | 0 | 0 |
| Quantum Secret Sharing Protocol Based on Mutually Unbiased Bases in Vehicular Networks | 6 | 0 |
| V-PISL:Post-quantum identity-based signature scheme over lattice for VANETs | 3 | 0 |
| Variational Quantum Reinforcement Learning for Joint Resource Allocation of Blockchain-Based Vehicular Edge Computing and Quantum Internet | 10 | 0 |
| QSAFE‐V: Quantum‐Enhanced Lightweight Authentication Protocol Design For Vehicular Tactile Wireless Networks | 32 | 2 |
| Post-Quantum Secure Authenticated Key Agreement Scheme for Vehicular Digital Twin | 5 | 0 |
| Authenticable <i>(t,n)</i> Threshold Quantum Secret Sharing Protocol for Secure Vehicular Communication | 0 | 0 |
| A Quantum-Resistant Authentication and Key Agreement Protocol for Fog-Assisted Vehicular Networks | 0 | 0 |
| A security-enhanced authentication scheme for quantum-key-distribution (QKD) enabled Internet of vehicles in multi-cloud environment | 20 | 5 |
| LAKAS-N: Quantum Attack-Resistant Lattice-Based Authentication and Key Agreement Using NIZK for Single and Cross-Domain VANETs | 0 | 0 |
| A Privacy Preserving Authentication Protocol Using Quantum Computing for V2I Authentication in Vehicular Ad Hoc Networks | 100 | 11 |
| DQ-VeDT: A Decentralized Quorum-Based Authentication for Quantum-Safe Vehicular Digital Twins | 27 | 2 |
| Anonymous lattice-based authentication protocol for vehicular communications | 48 | 4 |
| Quantum-resistant ring signature-based authentication scheme against secret key exposure for VANETs | 4 | 1 |
| A Lightweight Quantum-Secure Authentication and Key Agreement Protocol for Vehicular Ad-Hoc Networks | 0 | 0 |
| Quantum-Resilient Secure Onboard Communication (QRSecOC): Integrating Post-Quantum Cryptography for Robust Automotive Network Security | 32 | 1 |
| Strengthening Post-Quantum Security for Automotive Systems | 6 | 0 |
| Lightweight, Post-Quantum Secure Cryptography Based on Ascon: Hardware Implementation in Automotive Applications | 48 | 2 |
| Hybrid Post-Quantum IKEv2 on Embedded Automotive Platforms: Design, Implementation, and Evaluation | 5 | 1 |
| Secure Boot in Automotive Real-Time Systems in Context of Post-Quantum Cryptography | 2 | 0 |
| Authentication Protocol for Secure Automotive Systems: Benchmarking Post-Quantum Cryptography | 22 | 0 |
| On Post-Quantum Attribute-Based Encryption for Automotive Over-the-Air Software Updates | 1 | 0 |
| Lightweight Lattice-Based Signature for VANET | 0 | 0 |
| LBA-PAKE: Lattice-Based Anonymous Password Based Authentication Key Exchange Scheme for VANET | 13 | 1 |
| LbPV: Lattice-based Privacy-preserving mutual authentication scheme for VANET | 11 | 0 |
| Privacy Protection of VANET Based on Traceable Ring Signature on Ideal Lattice | 60 | 6 |
| Lattice-Based Certificateless Aggregate Authentication Scheme With Conditional Privacy-Preserving for VANET | 10 | 1 |
| A secure and efficient lattice-based conditional privacy-preserving authentication protocol for the VANET | 9 | 2 |
| LQSCPPA: Lightweight and Quantum-Secure Lattice-Based Conditional Privacy-Preserving Authentication Scheme for VANET | 2 | 0 |
| LB-CLAS: Lattice-based conditional privacy-preserving certificateless aggregate signature scheme for VANET | 37 | 8 |
| Enhancing VANET Security with Lattice-Based Cryptography and Dynamic Pseudonym Updates | 5 | 0 |
| Efficient Authorization Authority Certificate Distribution in VANETs | 35 | 1 |
| Pseudonym Certificate Validations under Heavy Vehicular Traffic Loads | 4 | 0 |
| Revised self-certified implicit certificate scheme for anonymous communications in vehicular networks | 1 | 0 |
| Certificate status information distribution and validation in vehicular networks | 47 | 5 |
| A New Approach To Pseudonym Certificate Management in V2X Communication | 3 | 0 |
| Schnorr-based conditional privacy-preserving authentication scheme with multisignature and batch verification in VANET | 0 | 0 |
| Dynamic Anonymous Quantum-Secure Batch-Verifiable Authentication Scheme for VANET | 17 | 2 |
| Automotive Security Solution Using Hardware Security Module (HSM) | 10 | 0 |
| Firmware Security Module | 22 | 1 |
| AVoD: Advanced Verify-on-Demand for Efficient Authentication against DoS Attacks in V2X Communication | 32 | 1 |
| Quantum privacy‐preserving service for secure lane change in vehicular networks | 28 | 0 |
| Post Quantum Blockchain Based Lightweight Edge Server Handover Authentication Key Agreement Protocol for Vehicular Communication System | 2 | 0 |
| An Efficient Security Scheme for Vehicular Communication Using a Quantum Secret Sharing Method | 10 | 0 |
| Post-Quantum Weighted Anonymous Authentication for Hybrid VANET MAC Protocol | 16 | 2 |
| Quantum-resistant blockchain-assisted certificateless authentication and three-party key agreement scheme for VANET | 3 | 1 |
| Efficient anti-quantum attack batch authentication and key agreement protocol based on NTRU in VANET | 3 | 0 |
| Quantum-resistant Lattice-based Authentication for V2X Communication in C-ITS | 7 | 0 |
| Secure Lattice-Based Aggregate Signature Scheme for Vehicular Ad Hoc Networks | 56 | 8 |
| RSA Threshold Signature Based Node Eviction in Vehicular Ad Hoc Network | 5 | 0 |
| Lattice-Based Conditional Privacy-Preserving Authentication Protocol for the Vehicular Ad Hoc Network | 53 | 10 |
| QR-LBAS: Lattice-Based Aggregate Signature Scheme With Quantum-Resistant Security for Vehicular Ad Hoc Networks | 0 | 0 |
| Lattice-based double-authentication-preventing ring signature for security and privacy in vehicular Ad-Hoc networks | 81 | 9 |
| FALCON-C: Flow-Based Analysis and Labeling for Connected Vehicular Network Cybersecurity | 11 | 0 |
| Using butterfly keys: A performance study of pseudonym certificates requests in C-ITS | 5 | 0 |
| ACPC: Efficient revocation of pseudonym certificates using activation codes | 68 | 6 |
| ESCORT: Efficient Status Check and Revocation Transparency for Linkage-Based Pseudonym Certificates in VANETs | 1 | 0 |
| A Hybrid Post-Quantum Cryptography and Machine Learning Framework for Intrusion Detection in VANETs | 0 | 0 |
| Enhancing Security and Privacy in Connected and Autonomous Vehicles: A Post-Quantum Revocable Ring Signature Approach | 0 | 0 |
| Forensics System for Internet of Vehicles Based on Post-Quantum Blockchain | 34 | 5 |
| Securing Internet of Vehicles with a provable secure post-quantum mutually authenticated protocol based on Small Integer Solution | 1 | 0 |
| Improving Security of Internet of Vehicles Based on Post-quantum Signatures with Systolic Divisions | 7 | 2 |
| An Efficient Post-Quantum Multi-Signature Scheme for the Internet of Vehicles | 1 | 0 |
| Post Quantum Digital Signature Change in IOTA to Reduce Latency in Internet of Vehicles (IoV) Environments | 25 | 1 |
| Applied Post-Quantum Secure Method for IoT Devices: A Case Study for Autonomous Vehicles Communication | 1 | 1 |
| Fortifying V2RSU Communication with Post Quantum Security in the Green Internet of Vehicles | 0 | 0 |
| Post-quantum lightweight identity-based two-party authenticated key exchange protocol for Internet of Vehicles with probable security | 64 | 13 |
| Module lattice based post quantum secure blockchain empowered vehicle to vehicle communication in the internet of vehicles | 21 | 3 |
| Post Quantum Secure Aggregate Signature Assisted Data Authentication for Internet of Vehicles | 0 | 0 |
| A hash-based post-quantum ring signature scheme for the Internet of Vehicles | 3 | 1 |
| Privacy Aware Post Quantum Secure Ant Colony Optimization Ad Hoc On-Demand Distance Vector Routing in Intent Based Internet of Vehicles for 5G Smart C | 77 | 3 |
| Quantum Secure Pairwise Key Agreement Scheme for Fog-Enabled Social Internet of Vehicles | 52 | 5 |
| Discussion on the Application of Quantum Teleportation Technology in the Internet of Vehicles | 26 | 1 |
| Quantum secure direct communication in Internet of Vehicles | 33 | 0 |
| A Secure Post-Quantum Authentication Scheme for VANETs | 0 | 0 |
| Lattice-Based Identity Authentication Protocol with Enhanced Privacy and Scalability for Vehicular Ad Hoc Networks | 21 | 2 |
| A Hardware Implementation of Blockchain Enabled Cross Domain Authentication Scheme in Internet of Vehicles | 16 | 3 |
| Key-Based Data Deduplication via Homomorphic NTRU for Internet of Vehicles | 16 | 1 |
| NTRU-CLS: Efficient quantum-resistant NTRU lattice-based certificateless signature scheme for VANETs | 8 | 1 |
| Quantum secure threshold password authenticated key exchange scheme for VANET | 75 | 1 |
| Revocation in Vehicular Public Key Infrastructures: Balancing privacy and efficiency | 8 | 0 |
| itAU: Incentive and Traceable Authentication for VANET via Weighted Threshold Signature | 0 | 0 |
| Distributed Key Generation for Secure Communications Between Different Actors in Service Oriented Highly Dense VANET | 5 | 0 |
| Beyond Computation: A Fragmentation-Aware Performance Analysis of Cryptographic Schemes for Real-Time C-V2X Cooperative Perception | 0 | 0 |
| On Handling of Certificate Digest in V2X Communication | 4 | 0 |
| Evaluation and Analysis of Standard Security Techniques in V2X Communication: Exploring the Cracking of ECQV Implicit Certificates | 19 | 0 |
| An experimental analysis of ECQV implicit certificates performance in VANETs | 34 | 1 |
| Fast Verification of Signatures With Shared ECQV Implicit Certificates | 8 | 0 |
| Efficient and dynamic elliptic curve qu‐vanstone implicit certificates distribution scheme for vehicular cloud networks | 9 | 0 |
| Privacy-Preserving Authentication Using Blockchain and Implicit Certificates for Internet of Vehicles* | 0 | 0 |
| Implicit certificates support in IEEE 1609 security services for Wireless Access in Vehicular Environment (WAVE) | 13 | 0 |
| Blockchain-based Conditional Privacy-Preserving Authentication Protocol with Implicit Certificates for Vehicular Edge Computing | 8 | 0 |
| Fast Signature Verification with Shared Implicit Certificates for Vehicular Communication | 4 | 0 |
| IOTA-VPKI: A DLT-Based and Resource Efficient Vehicular Public Key Infrastructure | 23 | 2 |
| Security Overhead and Its Impact in VANETs | 12 | 0 |
| A Post-Quantum Privacy-Preserving Payment Protocol in Vehicle to Grid Networks | 11 | 1 |
| LaRRS: Lattice-Based Revocable Ring Signature and Its Application for VANETs | 45 | 9 |
| Malicious signature localization in lattice-based batch-verifiable signature for VANETs | 30 | 1 |
| A forward-secure and efficient authentication protocol through lattice-based group signature in VANETs scenarios | 64 | 7 |
| Post-quantum blind signature-based authentication for intelligent transportation systems | 0 | 0 |
| QSVP: A Quantum Safe Mutual Authentication Protocol for Vehicular Platoon Communications | 3 | 0 |
| RSA-Based Anonymous Certificate for Security Credential Management System | 14 | 1 |
| Implementation and Performance Analysis of Security Credential Management System Based on IEEE 1609.2 and 1609.2.1 Standards | 18 | 2 |
| The Unified Butterfly Effect: Efficient Security Credential Management System for Vehicular Communications | 29 | 0 |
| Secure Vehicular Traffic Re-routing System using SCMS in Connected Cars | 18 | 0 |
| BLS-TAPS-PR: A Scalable Threshold Accountable Private Signature With Proactive Refresh for Secure Internet of Vehicles | 0 | 0 |
| Decentralized Anonymous Authentication and Traceability for Internet of Vehicles Using Threshold Key Generation | 0 | 0 |
| Privacy-Preserving Data Uploading SchemeBased on Threshold Secret Sharing Algorithm for Internet of Vehicles | 1 | 0 |
| Blockchain Empowered Quantum Safe Batch Aggregate Signature Algorithm for Authenticated Data Trading in Internet of Vehicles | 7 | 1 |
| Quantum resistant blockchain and deep learning revolutionize secure communications for autonomous vehicles | 50 | 2 |
| LCMS: Efficient Lattice-Based Conditional Privacy-Preserving Multi-Receiver Signcryption Scheme for Internet of Vehicles | 0 | 0 |
| Intelligent connected vehicle data security based on blockchain combining zero-knowledge proof and post-quantum cryptography | 4 | 0 |
| Post Quantum Based Identity Signature Scheme with Lattice Assumption for VANETs | 3 | 0 |
| Post Quantum Cryptography on Intravehicular Networks | 1 | 0 |
| Enhancing Intelligent Transport Systems through Quantum and blockchain security in vehicular networks | 0 | 0 |
| Secure and Privacy-Preserving Post-Quantum Attribute-Based Searchable Encryption for Edge-Driven Transportation Systems | 4 | 1 |
| Enhanced EAADE: a quantum-resilient and privacy-preserving authentication protocol for secure data exchange in vehicular social networks | 7 | 0 |
| Standardizing Post-Quantum Secure Semantic Communications for AI-Native Autonomous Vehicle and Intelligent Transportation Networks | 0 | 0 |
| Lattice-Based Lightweight Quantum Resistant Scheme in 5G-Enabled Vehicular Networks | 78 | 14 |
| Post-Quantum Era Privacy Protection for Intelligent Infrastructures | 391 | 22 |
| Quantum Key Distribution for Secure Vehicle-To-Vehicle Communication in Autonomous Electric Vehicle Networks | 0 | 0 |
| PostCrypt: Energy-Efficient Lattice-Based Post-Quantum Framework for Secure 5G/6G Network Slicing | 0 | 0 |
| QSKA: A Quantum Secured Privacy-Preserving Mutual Authentication Scheme for Energy Internet-Based Vehicle-to-Grid Communication | 17 | 3 |
| Demo: An Open-Source Hardware-in-the-Loop Testbed for Post-Quantum V2V Security Research | 4 | 0 |
| Evaluation of Post-Quantum Cryptography Signature Scheme on an Automotive Multicore Microcontroller for Real-Time Application | 1 | 0 |
| LcASS: Efficient Lattice-Based Certificateless Aggregation Signature Scheme With Blockchain for VANETs | 0 | 0 |
| Privacy-Preserving and Puncturable Bilateral Access Control From Lattices for Cloud-Assisted IoV | 1 | 1 |
| A blockchain-based quantum-secure protocol for efficient V2I handover authentication in vehicular Ad-Hoc networks | 2 | 0 |
| RAVEN: Robust Anonymous Vehicular End-to-End Encryption and Efficient Mutual Authentication for Post-Quantum Intelligent Transportation Systems | 0 | 0 |
| Quantum-Resistant Authentication Based on Digital Signatures: An Analysis and Study Case Under a Transition Scenario | 0 | 0 |
| Quantum-resistant blockchain architecture for secure vehicular networks: A ML-KEM-enabled approach with PoA and PoP consensus | 3 | 0 |
| GenAI-Driven Quantum-Resilient Consensus Framework for Blockchain-Enabled Vehicular Digital Twins | 22 | 0 |
| Lightweight Quantum-Resistant Blockchain-Based Key Agreement Scheme for IoV Communications | 36 | 7 |
| Post-Quantum Verifiable Decryption for Secure and Scalable V2I Communication in Consumer Driven-Intelligent Transportation Systems | 2 | 0 |
| Beyond Classical PKI: A Quantum-Safe Authentication Framework for Automotive OTA Updates | 0 | 0 |
| Blockchain-Envisioned Provably Secure Multivariate Identity-Based Multi-Signature Scheme for Internet of Vehicles Environment | 37 | 6 |
| Quantum-Resistant Microelectronic Cryptography for Securing Vehicular Networks | 0 | 0 |
| Collision Warning in Vehicular Networks Based on Quantum Secure Multiparty Computation | 11 | 1 |
| Quantum-Secure Predictive Maintenance Framework for Future VANET-Based Smart Transportation Systems | 3 | 0 |
| LRCPA: Lattice-Based Robust and Conditional Privacy-Preserving Authentication for VANETs | 13 | 0 |
| A Low-Latency Post-Quantum Group Authentication Framework for VANETs | 2 | 0 |
| ZebraCPA: Decentralized, Postquantum Conditional Privacy-Preserving Authentication for VANETs via Traceable ZK Ring Signatures | 66 | 9 |
| A Code-Based Anonymous Authentication Scheme for Vehicular Ad Hoc Networks | 2 | 1 |
| Quantum Resistant Cryptographic Protocols for Securing Autonomous Vehicle to Vehicle (V2V) Communication Networks | 11 | 0 |
| Combining Lightweight and Post-Quantum Cryptography With Physically Unclonable Functions for Secure In-Vehicle Communications | 1 | 0 |
| Quantum-Based Secure Authentication Mechanism Using Dilithium and Blockchain for Internet of Vehicle (IoV) Environment | 0 | 0 |
| NTRU-CLSC: Efficient Quantum-Resistant NTRU Lattice-Based Certificateless Signcryption Scheme for VANETs | 0 | 0 |
| On the use of code-based cryptography in automotive applications | 3 | 0 |
| Building Scalable and Quantum Attack Resistant Authenticated Message Delivery System for Internet of Vehicles With Blockchain Consensus Mechanism | 5 | 0 |
| A Federated Learning–Enabled Secure and Scalable SDN Framework for Energy‐Efficient VANETs | 5 | 1 |
| Autonomous UPQ-PAKE Quantum-Resistant Vehicular Network Scheme | 0 | 0 |
| PQ-AuthV: Post-Quantum Secure Authentication With Aggregated Signatures in IoT-Enabled Smart Vehicle Networks | 1 | 0 |
| Post-Quantum Authentication for Emergency Messaging in 5G Vehicular Fog Networks | 7 | 1 |
| Quantum-Resistant Privacy Preservation for Mobile Healthcare Services in Connected Transportation Systems via Deep Neural Architectures. | 2 | 0 |
| Design and Validation of a Secure and Context-Aware Vehicular Ad Hoc Network (VANET) Framework Using Trust-Based Multiobjective Analysis | 41 | 3 |
| Quantum Cryptography for Securing the Internet of Things: A Comprehensive Literature Survey | 0 | 0 |
| Quantum-Resilient Secure Context-Aware Trust-Based Routing with Preemptive Verifiable Key Handover for Vehicular Ad Hoc Networks | 46 | 1 |
| A Post-Quantum Certificateless Aggregate Signature Scheme for VANETs Resilient to Rogue-Key Attacks | 0 | 0 |
| Edge Computing in Internet of Things: Lattice-Based and Split Encryption for Post-Quantum Data Security | 8 | 2 |
| Comprehensive Survey of Post-Quantum Secure Lattice-Based Authentication Schemes for VANETs | 5 | 0 |
| HyQCAN: A Quantum-Classical Synergy for Secure and Low-Latency Emergency Communication in Vehicular Ad Hoc Networks | 0 | 0 |
| A Post-Quantum Decentralized Threshold Authenticated Key Agreement Scheme for Securing Vehicular Ad-Hoc Networks | 0 | 0 |
| Quantum-safe and provable secure vehicle to infrastructure authenticated key-agreement for VANETs | 2 | 0 |
| Enhancing Vehicle IoT Security with PQC: A Lightweight Approach for Encrypted Sensor Data Transmission | 0 | 0 |
| Post quantum secure and efficient data sharing scheme from attribute based encryption for VANETs over lattices | 0 | 0 |
| Integrating Elliptic Curve Encryption and Post‐Quantum Cryptography for Secure VANETs Using Fog Computing and SDN | 36 | 4 |
| NTRU Implementation of Efficient Privacy-Preserving Location-Based Querying in VANET | 56 | 6 |
| Post-Quantum Secure Lattice-Based Authentication Framework for Smart Vehicular Edge Networks in Consumer IoT | 0 | 0 |
| Privacy-Preserving Software Engineering Protocol for V2V Charging Using Lattice- Based Linkable Ring Signature | 45 | 6 |
| A Vehicle Forensics framework with Post-Quantum Cryptography and Blockchain | 1 | 0 |
| Secure EV Charging and Communication: A Hybrid GNN-IDS and Post-Quantum Cryptographic Approach | 1 | 0 |
| V-LDAA: A New Lattice-Based Direct Anonymous Attestation Scheme for VANETs System | 35 | 2 |
| LCPPA: Lattice‐based conditional privacy preserving authentication in vehicular communication | 45 | 2 |
| A comprehensive review of ECC-based certificateless aggregate signature for Vehicular Ad-Hoc Networks (VANETs) and cryptanalysis of quantum-secure CLA | 0 | 0 |
| Evaluation of additional Post-Quantum Signature Schemes on automotive ECUs | 0 | 0 |
| Hybrid Key Exchange and Signature Design for Quantum-safe C-ITS | 27 | 1 |
| Design and Evaluation of Secure Software Architectures for 5G-Enabled Vehicular Driving System | 1 | 0 |
| Secure Key Management Schemes for High-Mobility VANET Environments | 25 | 1 |
| Privacy-Preserving In-Vehicle Intrusion Detection via Nearest Centroid Classification Under Homomorphic Encryption | 43 | 0 |
| Impersonation Attack Using Quantum Shor’s Algorithm Against Blockchain-Based Vehicular Ad-Hoc Network | 20 | 1 |
| Research on Secure Communication on In-Vehicle Ethernet Based on Post-Quantum Algorithm NTRUEncrypt | 30 | 1 |
| LB-CLAA: An Anonymous and Efficient Postquantum Certificateless Aggregate Authentication Scheme for VANET | 0 | 0 |
| Lightweight Lattice-Based Multi-Domain Authentication Protocol with Real-Time Revocation and Aggregated Verification for Vehicular Communication | 0 | 0 |
| Post-Quantum Secure Over-the-Air Update of Automotive Systems | 7 | 2 |
| A Lightweight Double-Ring Hybrid Sparse NTRU (DRH-SNTRU) Scheme for Secure and Real-Time Communication in the Internet of Vehicles (IoV) | 0 | 0 |
| Post-Quantum Secure Architectures for Automotive Hardware Secure Modules | 8 | 0 |
| A quantum resistant chaos driven image encryption framework for secure visual data transmission in intelligent transportation systems | 0 | 0 |
| QS-BTrust: A Quantum-Secure Privacy-Preserving Protocol With Revocation for Trusted Broadcasting in Integrated Vehicular Networks | 0 | 0 |
| Migrating Time and Security-Critical PKIs to Post-Quantum Cryptography: SWIM and C-ITS | 22 | 2 |
| Enhancing ZTMAF through federated trust learning, post quantum cryptography integration and seamless cross domain authentication handover | 29 | 3 |
| Decentralized Trust Model for Vehicle Ad-Hoc Networks (VANETs) with 5G Integration: A Blockchain-Based Approach for Enhanced Security and Privacy in I | 0 | 0 |
| Quantum-Resilient Blockchain Framework for Intelligent Transportation Systems | 0 | 0 |
| Quantum safe drone security: Authenticated key exchange and privacy preserving intrusion detection for low altitude intelligent transportation systems | 0 | 0 |
| Quantum-resistant hybrid encryption framework for secure and intelligent Vehicle-to-Vehicle communication using deep representation learning models | 44 | 6 |
| Quantum-safe Trust for Vehicles | 1 | 0 |
| Guest Editorial Beyond Quantum Threats: Advancing Post-Quantum Cryptographic Strategies for Next-Generation Intelligent Transportation Systems | 0 | 0 |
| QSTMF: Quantum-Secured Trust Management Framework for VANETs in Web 3.0 and Metaverse | 6 | 0 |
| Q-Auth: Decentralized and Quantum-Secure Authentication Protocol for Vehicular Networks | 2 | 0 |
| Enhancing Transportation Cyber-Physical Systems Security: A Shift to Post-Quantum Cryptography | 15 | 3 |
| Quantum Secure Authentication and Key Exchange Protocol for UAV-Assisted VANETs | 44 | 7 |
| Research on Security for Software Updates of Intelligent Connected Vehicles Based on Post-Quantum Cryptography | 1 | 0 |
| Facing emerging challenges in connected vehicles: a formally proven, legislation compliant, and post-quantum ready security protocol | 4 | 0 |
| Quantum-Resistant Data Sharing Scheme With Auditability for Internet of Vehicles | 0 | 0 |
| Quantum Security Risks in Autonomous Vehicles and Smart Transportation Systems | 7 | 2 |
| Design of an Automotive OTA System with Zonal Gateways and PQC-Hybrid TLS | 0 | 0 |
| Q-CARE-VLC: Quantum-Secured Vehicular Visible-Light Communication with Key-Continuity-Aware Handover for Smart-Ambulance Healthcare | 0 | 0 |
| A Lattice-Based Ring Signcryption Scheme for Secure Communication in 6G-Enabled Vehicular Ad Hoc Networks Using Blockchain | 6 | 0 |
| A secure lattice-based certificateless aggregate signature scheme with conditional privacy preservation for VANETs | 0 | 0 |
| A lattice-based linkable authentication scheme for privacy-preserving vehicular systems | 1 | 0 |
| Lattice-Based Cryptography and Fog Computing Based Efficient Anonymous Authentication Scheme for 5G-Assisted Vehicular Communications | 66 | 9 |
| NTRU and Secret Sharing Based Secure Group Communication for IoT Applications | 38 | 2 |
| Lattice-based forward and backward secure group signature with selective linkability for VANETs | 0 | 0 |
| Traceable Ring Signature Scheme in Internet of Vehicles Based on Lattice Cryptography | 2 | 0 |
| LETA: A Lattice-Based Efficient and Traceable Privacy-Preserving Batch Authentication Scheme for Vehicle Platoon in VANETs | 7 | 1 |
| Design of Linkable Ring Signature Scheme Based on the MP‐LWE Problem and Its Application in IoV | 41 | 0 |
| Research on NTRU-based Anonymous Authentication and Key Negotiation Protocol for VANETs | 3 | 1 |
| CLLS: Efficient certificateless lattice-based signature in VANETs | 10 | 1 |
| RRSC: Revocable Ring Signature Scheme over CRYSTALS-Dilithium for VANETs | 3 | 0 |
| Quantum federated learning for autonomous vehicle cybersecurity: An analytical review of architectures and threat landscapes | 56 | 0 |
| Securing the Electric Vehicle Charging Infrastructure | 93 | 0 |
| An Overview of Cyber Security and Privacy on the Electric Vehicle Charging Infrastructure | 81 | 0 |
| QuantumCharge: Post-Quantum Cryptography for Electric Vehicle Charging | 17 | 1 |
| Crypto-Agile FPGA Architecture with Single-Cycle Switching for OFDM-Based Vehicular Networks | 31 | 3 |
| A Game-Theoretic Multi-Agent Reinforcement Learning Approach for Secure UAV-Assisted QKD in Vehicular Networks | 0 | 0 |
| Vehicle-to-Vehicle Quantum Key Distribution (V2V-QKD) | 1 | 0 |
| Revolutionizing Internet of Vehicles with Quantum Key Distribution on Blockchain for Unprecedented Security | 15 | 0 |
| Quantum-Resilient Authentication Protocol for Secure V2G Communications | 0 | 0 |
| Optimization and Safety Research on Autonomous Vehicles Based on Blockchain, Quantum Computing, and Artificial Intelligence | 13 | 0 |
| ConfidSPEC-V2X: A Quantum-Blockchain Intelligence for Mitigating Confidentiality Threats in Vehicle-to-Everything Networks | 41 | 2 |
| Research on Vehicle-to-Cloud Communication Based on Lightweight Authentication and Extended Quantum Key Distribution | 13 | 0 |
| Lattice-Based Anonymous Batch Verifiable Authentication for Fog-Assisted VANETs | 2 | 0 |
| LBCSC: Lattice‐based chameleon signcryption scheme for secure and privacy‐preserving vehicular communications | 2 | 0 |
| SVQCP: A Secure Vehicular Quantum Communication Protocol | 15 | 0 |
| Relay-Assisted Satellite QKD Systems using MMW Radio-over-FSO for Vehicular Networks | 0 | 0 |
| A hierarchical federated learning framework with FedNova, game-theoretic matching, and QKD-assisted privacy for the internet of vehicles | 69 | 0 |
| A secure lattice-based anonymous authentication scheme for VANETs | 29 | 2 |
| Drone- and Vehicle-Based Quantum Key Distribution | 64 | 4 |
| L-CPPA: Lattice-based conditional privacy-preserving authentication scheme for fog computing with 5G-enabled vehicular system | 118 | 15 |
| Quantum Leap for Mobility: Revolutionizing Transportation with Quantum Computing | 2 | 0 |
| Scalability Limits of Roadside Units in VANETs under Quantum Key Constrained Security Framework | 0 | 0 |
| HAP-Aided Relaying Satellite FSO/QKD Systems for Secure Vehicular Networks | 24 | 0 |
| Explainable Federated Learning and Quantum-based Secure Framework for V2X Communication | 0 | 0 |
| Enhancing autonomous vehicle security in intelligent transportation systems through quantum computing and optimisation based federated learning | 0 | 0 |
| Tetherless THz CV-QKD – Would It Work? | 0 | 0 |
| QBMA-BIV: Quantum-Key-Distribution (QKD)-Based Multi-Server Authentication Scheme for Blockchain-Enabled Internet of Vehicles | 9 | 1 |
| NTRU Lattice–Based Blockchain Consensus–Assisted Vehicle‐to‐Vehicle Authenticated Message Delivery in Internet of Vehicles | 33 | 9 |
| LBRAKA: Lattice-Based Robust Authenticated Key Agreement for VANETs | 12 | 2 |
| A Lattice-Based Certificateless Authentication and Key Agreement Protocol for VANETs | 0 | 0 |
| Privacy Preserving Multi-Party Computation With Secret Sharing for Trajectory Prediction in VANETs | 14 | 2 |
| Circulation of legitimate information over VANETs using threshold signature scheme | 6 | 0 |
| Anonymous Cluster-Based MANETs with Threshold Signature | 48 | 4 |
| Anonymous Multi-Party Computation With Verifiability for Edge-Enabled Green Internet of Vehicles (IoV) | 2 | 0 |
| Distributed Trust Based Authentication Scheme in a Clustered Environment Using Threshold Cryptography for Vehicular Ad Hoc Network | 6 | 0 |
| A VANET privacy protection scheme based on fair blind signature and secret sharing algorithm | 52 | 7 |
| An Efficient (t, n) Threshold Authentication Scheme for Vehicular Ad Hoc Networks | 0 | 0 |
| Infrastructure-Free V2V Key Repair for Threshold BLS Signatures in Autonomous Vehicle Platoons | 0 | 0 |
| An Efficient Privacy-Preserving Authentication Scheme Based on Shamir Secret Sharing for VANETs | 2 | 1 |
| Hierarchical Secure Federated Learning Based on Secret Sharing for Emergency Situations in IoV | 0 | 0 |
| BELIEVE: Privacy-Aware Secure Multi-Party Computation for Real-Time Connected and Autonomous Vehicles and Micro-Mobility Data Validation Using Blockch | 17 | 1 |
| Runtime Threshold Signature-Based Unmanned Aerial Vehicle Swarm Authentication With Autonomous Splitting Support | 0 | 0 |
| Research on Secure and Privacy-Preserving Scheme Based on Secure Multi-Party Computation for VANET | 5 | 1 |
| Blockchain-Oriented Certificateless Threshold Signature With Identifiable Abort for Federated Learning in Digital Twin-Assisted IoV | 5 | 0 |
| Privacy-Preserving Authentication Based on Pseudonyms and Secret Sharing for VANET | 2 | 1 |
| Single-Round Privacy-Preserving Emergency Alert Protocol for High-Speed V2X Communications | 0 | 0 |
| Efficient Privacy-Preserving Task Allocation With Secret Sharing for Vehicular Crowdsensing | 16 | 0 |
| A Multi-Secret Reputation Adjustment Method in the Secret Sharing for Internet of Vehicles | 37 | 2 |
| ED2S: efficient data sharing scheme for IoV via threshold signature based PBFT | 0 | 0 |
| Blockchain-Assisted Distributed Key Generation and Authentication for Smart Vehicles in Intelligent Transportation Systems | 0 | 0 |
| RSU authentication in vehicular ad hoc networks base on verifiable secret sharing | 0 | 0 |
| A group key agreement scheme for vehicular ad-hoc networks based on dynamic threshold and verifiable secret sharing | 0 | 0 |
| Incentive Mechanism for Privacy-Preserving Collaborative Routing Using Secure Multi-Party Computation and Blockchain | 51 | 1 |
| Blockchain-Assisted Message Reporting Scheme With Weighted Threshold Signature for Vehicular Ad-Hoc Networks | 1 | 0 |
| MPC-CSAS: Multi-Party Computation for Real-Time Privacy-Preserving Speed Advisory Systems | 45 | 2 |
| A lightweight privacy preserving distributed certificate-less aggregate based mutual authentication scheme for vehicular adhoc networks | 4 | 0 |
| Privacy-Preserving Threshold Announcements With Efficient Batch Verification for Cooperative Intelligent Transport Systems | 0 | 0 |
| A Data Sharing Scheme Based on Consortium Blockchain and Lattice Cryptography in Internet of Vehicles | 2 | 0 |
| A Multisignature-Based Secure and OBU-Friendly Emergency Reporting Scheme in VANET | 14 | 2 |
| Cross-Layer Trust-Based Authentication Framework for Preventing Collaborative Blackhole Attacks in VANETs | 0 | 0 |
| Verify-on-demand - A practical and scalable approach for broadcast authentication in vehicle-to-vehicle communication | 27 | 0 |
| Research on Communication Security Strategy for Intelligent Connected Vehicles Based on HSM | 2 | 0 |
| Efficient Signed Certificate Verification for IoT and V2V Messages via Blockchain Integration | 49 | 3 |
| Multi-Zone Authentication and Privacy-Preserving Protocol (MAPP) Based on the Bilinear Pairing Cryptography for 5G-V2X | 65 | 5 |
| A Secured Data Delivery and Validation Model for Information-Centric Vehicular Cloud Network | 10 | 0 |
| Design and technical verification of a new type of subscriber identity module card for internet of vehicles | 4 | 0 |
| Targeted Discreditation Attack against Trust Management in Connected Vehicles | 6 | 0 |
| Sybil Assault Detection in VANET with Efficient Network Analysis | 1 | 0 |
| EPA: An efficient and privacy-aware revocation mechanism for vehicular ad hoc networks | 81 | 13 |
| DEEP NEURAL BASED LIGHTWEIGHT ROUTING AND PROBABILITY PLOT CORRELATION FOR SECURE VANET COMMUNICATION | 0 | 0 |
| Fast-track message authentication protocol for DSRC using HMAC and group keys | 4 | 1 |
| Study and Implementation of Routing Protocols by using Security Method | 0 | 0 |
| A Multi-Tier Trust-Based Security Mechanism for Vehicular Ad-Hoc Network Communications | 75 | 4 |
| Anonymous and Revocable V2V Authentication and Key Agreement Scheme for VANETs | 0 | 0 |
| Roadside unit‐based pseudonym authentication in vehicular ad hoc network | 2 | 0 |
| On the Efficiency of Pairing-Based Authentication for Connected Vehicles: Time is Not on Our Side! | 12 | 6 |
| Edge-Intelligent Trust Orchestration Framework for Scalable and Resilient Vanet Communications | 0 | 0 |
| Reducing Security Overhead in the Context of Connected Vehicles | 3 | 1 |
| 5G NR-V2X Scheduling Approaches for CPM Variable Size Traffic | 9 | 0 |
| Agent-Based Secure Route Switching and QoS Management in VANETs Using the Wiedemann Car-Following Prediction Model | 49 | 9 |
| Secured Message Broadcasting in VANET using Blowfish Algorithm with Oppositional Deer Hunting Optimization | 34 | 1 |
| Comparative Experiments of V2X Security Protocol Based on Hash Chain Cryptography | 60 | 6 |
| Privacy-Preserving Method for Temporarily Linking/Revoking Pseudonym Certificates in VANETs | 15 | 1 |
| Detection of Sybil attack in vehicular ad hoc networks by analyzing network performance | 12 | 1 |
| Broadcast Authentication in Latency-Critical Applications: On the Efficiency of IEEE 1609.2 | 58 | 4 |
| Multi-Level Trust Evaluation Models with Emergency Message Dissemination for Connected Vehicle Ad Hoc Networks | 2 | 0 |
| Incorporating IEEE 1609.2-2016 Standard with Internet of Things-Based Low Power WAVE Devices | 1 | 0 |
| Critical Re-Engineering and Experimental Validation of Physical Layer Security Aware Geographic Routing in VANETs Using OMNeT++, INET, Veins, and SUMO | 0 | 0 |
| A Secure Interoperability Framework for Heterogeneous Internet of Things Devices in Intelligent Transportation Systems | 0 | 0 |
| Tradeoff Between Covertness and Transmission in NR V2X: Traditional and LLM-Assisted Cases | 2 | 0 |
| A Lightweight 5G-V2X Intra-Slice Intrusion Detection System Using Knowledge Distillation | 9 | 0 |
| Study on the privacy-preserving vehicular PKI in autonomous driving environments | 10 | 0 |
| Sustainable and Round-Optimized Group Authenticated Key Exchange in Vehicle Communication | 7 | 1 |
| SECMACE: Scalable and Robust Identity and Credential Management Infrastructure in Vehicular Communication Systems | 133 | 7 |
| Scalable Broadcast Authentication for V2V Communication | 1 | 0 |
| Automotive Network Security | 7 | 0 |
| Efficient FPGA Implementation of AES-CCM for IEEE 1609.2 Vehicle Communications Security | 0 | 0 |
| A Novel Framework for Autonomous Electric Vehicle Systems: 5G/6G Network Integration with Machine Learning and Multi-Access Edge Computing | 2 | 0 |
| A Lightweight Blockchain–Integrated VANET Prototype with Path Planning and Optimized Hardware for Secure V2V Networks in Challenging Environments | 0 | 0 |
| Secure V2I Authentication and Handover Protocol Leveraging Blockchain and Physically Unclonable Functions | 42 | 7 |
| Safeguarding the Electric Vehicle Ecosystem: A Comprehensive Analysis of Cybersecurity Threats | 0 | 0 |
| Cryptography Engine Design for IEEE 1609.2 WAVE Secure Vehicle Communication using FPGA | 3 | 1 |
| A Two-Factor Authentication Scheme for Moving Connected Vehicles | 5 | 1 |
| Evaluation and Analysis of Standard Security Technology in V2X Communication - Exploring ECQV Implicit Certificate Cracking | 0 | 0 |
| iVFAS: An Improved Vehicle-to-Fog Authentication System for Secure and Efficient Fog-Based Road Condition Monitoring | 16 | 3 |
| Hardware Limitations to Secure C-ITS: Experimental Evaluation and Solutions | 68 | 0 |
| Flexible Non-interactive Short-term Implicit Certificate Generation for VANETs | 27 | 2 |
| Schnorr-Based Implicit Certification: Improving the Security and Efficiency of Vehicular Communications | 14 | 1 |
| Reconfigurable hardware architecture of public key crypto processor for VANET and wireless sensor nodes | 1 | 0 |
| An Intelligent Hierarchical Security Framework for VANETs: Enhancing Trust, Authentication, and Privacy | 0 | 0 |
| Contribución a la ejecución segura y confiable de servicios distribuidos en redes ad-hoc vehiculares (VANET) mediante tecnologías Blockchain | 0 | 0 |
| An Efficient and Physically Secure Privacy-Preserving Key-Agreement Protocol for Vehicular Ad-Hoc Network | 24 | 2 |
| Robust adaptive multi-strategy routing in heterogeneous VANETs amidst dynamic adversarial conditions | 24 | 5 |
| RSSI and Public Key Infrastructure based Secure Communication in Autonomous Vehicular Networks | 40 | 3 |
| High-Quality in Data Authentication Dodging Massive Attack in VANETS | 29 | 0 |
| Post-Quantum Secure Boot on Vehicle Network Processors | 10 | 2 |
| Post-Quantum Secure System for Electric Vehicle Charging | 0 | 0 |
| Transitioning to Quantum-Resistant Infrastructure: A Comprehensive Evaluation of Lattice-Based Anonymous Authentication in Vehicle-to-Grid Networks | 33 | 8 |
| Lattice-Based Online/Offline Cross-Domain Authentication Scheme in Vehicle-to-Grid Network | 4 | 0 |
| Design-Space Mapping of Post-Quantum Cryptographic Artifact Transport on CAN-FD: A Discrete-Event Simulation Study | 0 | 0 |
| IEEE 1609.2 Security Services | 0 | 0 |
| Privacy‐preserving authentication scheme for on‐road on‐demand refilling of pseudonym in VANET | 20 | 6 |
| TLS Authentication using IEEE 1609.2 certificates | 1 | 0 |
| Implementation and Analysis of IEEE and ETSI Security Standards for Vehicular Communications | 62 | 3 |
| A Blockchain-Based Accident Forensics System for Smart Connected Vehicles | 15 | 0 |
| TLS Authentication using ETSI TS 103 097 and IEEE 1609.2 certificates | 0 | 0 |
| Provable Security Analysis of Butterfly Key Mechanism Protocol in IEEE 1609.2.1 Standard | 29 | 1 |
| Implementation of the IEEE 1609.2 wave Security Services Standard | 1 | 0 |
| Study on SCMS-Based Certificate and Electronic Signature Validity Verification in C-ITS (Cooperative-Intelligent Transport Systems) Environment | 0 | 0 |
| Simulation-Based Cybersecurity Testing and Evaluation Method for Connected Car V2X Application Using Virtual Machine | 39 | 0 |
| Misbehavior detection in vehicular networks with machine learning | 1 | 0 |
| SW Implementation on Security Algorithms in IEEE 1609.2 | 1 | 0 |
| The impact of ECDSA in a VANET routing service: Insights from real data traces | 6 | 1 |
| Location Verification Systems for VANETs in Rician Fading Channels | 65 | 2 |
| Signature Split Method for a PQC-DSA Compliant with V2V Communication Standards | 13 | 1 |
| Evaluating V2V Security on an SDR Testbed | 6 | 0 |
| Enhanced Position Verification for VANETs Using Subjective Logic | 50 | 0 |
| Performance Comparison of Security Credential Management Systems for V2X: North American Standard IEEE 1609.2.1 and European Standard ETSI TS 102 941 | 1 | 0 |
| Link Setup Time Reduction by FILS on IEEE 802.11-based Inter-vehicular Communications | 38 | 2 |
| SSOV: A Single Sign-on Protocol for Accessing Vehicular Application Services with the Support of Secret Credential Management System | 0 | 0 |
| Privacy Preserving Scheme for Pseudonym Refilling in VANET | 3 | 0 |
| V-PASS: Sybil-Resistant Pseudonym Self-Provisioning for V2X | 0 | 0 |
| V2X Credential Management System Comparison Based on IEEE 1609.2.1 and ETSI TS 102 941 | 22 | 0 |
| Efficient and Secure Location-Based Services Scheme in VANET | 36 | 1 |
| Efficient Lattice-Based Ring Signature for Message Authentication in VANETs | 58 | 6 |
| DiLSe: Lattice-Based Secure and Dependable Data Dissemination Scheme for Social Internet of Vehicles | 37 | 0 |
| LB-CLBS: Lattice-based certificateless blind signature scheme for vehicle sensing within intelligent transportation | 1 | 0 |
| Lattice-Based Conditional Privacy-Preserving Batch Authentication Protocol for Fog-Assisted Vehicular Ad Hoc Networks | 23 | 0 |
| Anti-Quantum Lattice-Based Ring Signature Scheme and Applications in VANETs | 49 | 2 |
| A lattice-based dual blockchain anonymous authentication scheme with forward security and revocability for VANETs | 1 | 1 |
| An Efficient Lattice-Based Authentication Protocol for the Vehicular Ad Hoc Network | 1 | 0 |
| A Lattice-Based Certificateless Secure Data Transmission Scheme for Internet of Vehicles Based-Blockchain | 12 | 1 |

### Round 2: 59 seeds, 2195 neighbours screened, 123 new records kept

| Seed | Neighbours | Added |
|---|---|---|
| Secure Authentication Mechanism for Cluster based Vehicular Adhoc Network (VANET): A Survey | 70 | 5 |
| DynamiQS: Quantum Secure Authentication for Dynamic Charging of Electric Vehicles | 36 | 1 |
| Quantum Computing for Advanced Driver Assistance Systems and Autonomous Vehicles: A Review | 153 | 1 |
| Cybersecurity in Electric Vehicle Infrastructure and the Impacts of Post-Quantum Encryption | 0 | 0 |
| A CARLA-Based Simulation Study of Quantum Key Distribution for Securing Vehicle-to-Infrastructure Communication | 0 | 0 |
| A Practical Implementation of Quantum-Derived Keys for Secure Vehicle-to-Infrastructure Communications | 55 | 1 |
| Securing in-vehicle communications through post-quantum cryptography and network segmentation | 10 | 0 |
| Machine Learning‐Based Intrusion Detection and Quantum Cryptography in Vehicular Networks | 3 | 0 |
| Drive (Quantum) Safe! – Towards Post-Quantum Security for V2V Communications ∗ | 13 | 0 |
| Zone Encryption with Anonymous Authentication for V2V Communication | 35 | 5 |
| A Cross-Layer MAC Aware Pseudonym (MAP) Scheme for the VANET | 0 | 0 |
| Efficient Adversarial Attacks Against DRL-Based Resource Allocation in Intelligent O-RAN for V2X | 18 | 0 |
| SPlatoon: Secure Platoon with Lightweight Authenticated Encryption | 54 | 3 |
| Blockchain-Assisted Meta-Learning for Efficient and Reliable Authentication in IoV | 0 | 0 |
| A resilient fog-enabled IoV architecture: Adaptive post-quantum security framework with generalized signcryption and blockchain-enhanced trust managem | 2 | 0 |
| A Trust‐Based Random Forest Approach for Detecting Blackhole Attacks in Vehicular Ad Hoc Networks | 27 | 2 |
| A Quantum-Enhanced Key Agreement and Signature Protocol for Securing Transportation Cyber-Physical Systems | 1 | 0 |
| Design, Implementation, and Evaluation of a Vehicular Hardware Security Module | 164 | 5 |
| Enhance Reliability and Security in VANET Using Clustering Based on ANT Colony Optimization and Fuzzy Logic | 0 | 0 |
| A HMAC-Based Token Authentication Protocol with Bloom Filter Revocation for Fog-Assisted Vehicular Networks | 38 | 4 |
| Quantum-resilient trust in motion: Smart-contract-driven authentication for next-gen vehicular networks | 45 | 6 |
| A Secure Energy Internet Scheme for IoV Based on Post-Quantum Blockchain | 5 | 0 |
| Module Lattice-Based Post Quantum Secure Blockchain Empowered Authentication Framework for Autonomous Truck Platooning | 62 | 1 |
| Q-ShielD: Quantum-Enhanced Secure Framework for Autonomous Vehicles Communication | 1 | 0 |
| ECQV-GDH-Based Group Key Exchange Protocol for CAN Bus | 12 | 1 |
| Advanced renewable electric vehicle charging system with optimized internet of things communication and quantum-secure data integrity | 0 | 0 |
| Design of an Efficient Fan-Shaped Clustered Trust-Based Routing Model with QoS & Security-Aware Side-Chaining for IoV Deployments | 28 | 3 |
| DMMQBIN: Design of an Efficient Triple Metaheuristic Model for QoS-Aware Blockchain deployments in IoV Networks | 0 | 0 |
| HFCO-EFL for Secure IoT-Enabled Vehicle-To-Vehicle Communication | 0 | 0 |
| An Enhanced Location-Aided Ant Colony Routing for Secure Communication in Vehicular Ad Hoc Networks | 69 | 9 |
| V-MARS: Velocity-Aware Multi-Attribute Reinforced Secure Routing for Efficient Data Transmission in Vehicular Networks | 31 | 0 |
| Secure Routing in VANET Systems Using Fog Computing and Software Defined Networks | 8 | 0 |
| Enhancing VANET Security Through ANFIS‐Based Intrusion Detection and Element Perturbation | 2 | 0 |
| A Bio-Inspired Cluster Optimization Schema for Efficient Routing in Vehicular Ad Hoc Networks (VANETs) | 118 | 2 |
| Post Quantum Secure Framework for IoT Based EV Battery Swapping Stations in Sustainable Transport Networks | 0 | 0 |
| A Hierarchical Privacy Preserving Pseudonymous Authentication Protocol for VANET | 106 | 26 |
| Traffic Prevention and Security Enhancement in VANET Using Deep Learning With Trusted Routing Aided Blockchain Technology | 13 | 2 |
| Adaptive neuro-fuzzy inference system and particle swarm optimization: A modern paradigm for securing VANETs | 38 | 1 |
| A Hybrid Approach to Trust Node Assessment and Management for VANETs Cooperative Data Communication: Historical Interaction Perspective | 117 | 2 |
| Secure Data Transmission Using Trust Based Multi Objective Gazelle Optimization Algorithm for VLC-VANET | 2 | 0 |
| ALI: Anonymous Lightweight Inter-Vehicle Broadcast Authentication With Encryption | 132 | 3 |
| Light and Efficient Authentication Mechanism for Connected Vehicles Using Unsupervised Detection | 2 | 0 |
| Cluster-Oriented Intelligent Secure Routing Protocol Design for Strengthened Communication in Vehicular Ad Hoc Networks | 3 | 0 |
| SROR: A Secure and Reliable Opportunistic Routing for VANETs | 59 | 0 |
| A Deep-Learning-Based Secure Routing Protocol to Avoid Blackhole Attacks in VANETs | 67 | 3 |
| ARPLR: An All-Round and Highly Privacy-Preserving Location-Based Routing Scheme for VANETs | 17 | 0 |
| TMAPS: A Trust-based Mutual Authentication and Privacy in VANET | 1 | 0 |
| A Secure Trust-aware Cross-layer Routing Protocol for Vehicular Ad hoc Networks | 24 | 1 |
| Toward the Design of an Efficient and Secure System Based on the Software-Defined Network Paradigm for Vehicular Networks | 93 | 3 |
| Election of MPR Nodes and Detection of Malicious Nodes Based on a Byzantine Fault in the OLSR Protocol Case of a Scale-Free Network | 25 | 0 |
| Developing an optimized routing protocol with rumor riding technique for detection of Sybil attack in VANET environment | 3 | 0 |
| Implementation of the WAVE 1609.2 Security Services Standard and Encountered Issues and Challenges | 3 | 0 |
| Vehicle-to-Nothing? Securing C-V2X Against Protocol-Aware DoS Attacks | 36 | 0 |
| Black hole attack detection in vehicular ad-hoc network using secure AODV routing algorithm | 141 | 13 |
| Secure Opportunistic Routing Protocol in VANETs | 22 | 0 |
| Secure Trust-Based Blockchain Architecture to Prevent Attacks in VANET | 151 | 10 |
| Practical SM2-Based Multisignature Scheme with Applications to Vehicular Networks | 25 | 7 |
| Learning Based Security for VANET with Blockchain | 34 | 3 |
| An Efficient and Privacy Preserved Session Sharing for Unmanned Aerial Vehicle Communication Using Lattice Based Ring Signature Scheme | 21 | 0 |

