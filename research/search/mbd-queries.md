# Cluster mbd: search log

Scope: V2X/VANET misbehaviour detection (local, global, plausibility, ML/DL, graph, transformer, federated, self-supervised), datasets and benchmarks, attacks on V2X, adversarial robustness of detectors, trust and reputation.

Date: 2026-10-17. Sources: OpenAlex (2 pages x 200, relevance order), arXiv API (top 200), DBLP (up to 1000 hits), Crossref (top 200), Semantic Scholar search (top 200).
Hit count = total reported by the source (OpenAlex and Crossref totals are fuzzy full-text matches and very large); returned = records fetched; kept = fetched records that passed inclusion and dedup.

Inclusion screen: title/abstract must name a vehicular/V2X context and a security/misbehaviour term; in-vehicle CAN-bus-only work, UAV, camera/LiDAR perception attacks and smart-grid work excluded. Relevance assigned by keyword rules (high = misbehaviour detection, plausibility, VeReMi/F2MD, Sybil, position falsification, ghost vehicles; medium = V2X intrusion/anomaly detection, trust/reputation, jamming, spoofing, replay, DoS; low = other vehicular security background). Records without abstract were enriched from OpenAlex by DOI before screening.

Result: 3593 records kept (high 920, medium 2363, low 310); 1580 first found by snowballing.

Snowballing (Semantic Scholar references + citations, limit 1000 each): round 1 from 679 high-relevance seeds (13996 candidates); round 2 from 300 new high-relevance seeds (top by citations, cap 300; 9064 candidates).

| # | Source | Query | Date | Hits | Returned | Kept |
|---|---|---|---|---|---|---|
| 1 | arXiv | `all:misbehavior AND all:vehicular` | 2026-10-17 | 24 | 24 | 21 |
| 2 | arXiv | `all:misbehaviour AND all:vehicular` | 2026-10-17 | 2 | 2 | 2 |
| 3 | arXiv | `all:misbehavior AND all:V2X` | 2026-10-17 | 10 | 10 | 9 |
| 4 | arXiv | `all:misbehaviour AND all:V2X` | 2026-10-17 | 1 | 1 | 1 |
| 5 | arXiv | `all:VeReMi` | 2026-10-17 | 12 | 12 | 12 |
| 6 | arXiv | `all:VANET AND all:attack` | 2026-10-17 | 59 | 59 | 43 |
| 7 | arXiv | `all:VANET AND all:detection` | 2026-10-17 | 57 | 57 | 28 |
| 8 | arXiv | `all:V2X AND all:attack` | 2026-10-17 | 66 | 66 | 36 |
| 9 | arXiv | `all:V2X AND all:intrusion` | 2026-10-17 | 13 | 13 | 7 |
| 10 | arXiv | `all:"connected vehicles" AND all:attack AND all:detection` | 2026-10-17 | 41 | 41 | 28 |
| 11 | arXiv | `all:sybil AND all:vehicular` | 2026-10-17 | 17 | 17 | 10 |
| 12 | arXiv | `all:VANET AND all:trust` | 2026-10-17 | 32 | 32 | 19 |
| 13 | arXiv | `all:"internet of vehicles" AND all:intrusion` | 2026-10-17 | 24 | 24 | 20 |
| 14 | arXiv | `all:V2X AND all:anomaly` | 2026-10-17 | 7 | 7 | 3 |
| 15 | arXiv | `all:"vehicular networks" AND all:anomaly` | 2026-10-17 | 14 | 14 | 9 |
| 16 | arXiv | `all:platoon AND all:attack` | 2026-10-17 | 53 | 53 | 35 |
| 17 | arXiv | `all:"collective perception" AND all:attack` | 2026-10-17 | 3 | 3 | 2 |
| 18 | arXiv | `all:V2X AND all:spoofing` | 2026-10-17 | 10 | 10 | 8 |
| 19 | arXiv | `all:V2V AND all:attack` | 2026-10-17 | 25 | 25 | 14 |
| 20 | arXiv | `all:"vehicular" AND all:jamming` | 2026-10-17 | 80 | 80 | 18 |
| 21 | DBLP | `misbehavior detection` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 22 | DBLP | `misbehaviour detection` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 23 | DBLP | `misbehavior vehicular` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 24 | DBLP | `misbehavior VANET` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 25 | DBLP | `misbehavior V2X` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 26 | DBLP | `VeReMi` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 27 | DBLP | `Sybil VANET` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 28 | DBLP | `Sybil vehicular` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 29 | DBLP | `intrusion detection VANET` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 30 | DBLP | `intrusion detection vehicular` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 31 | DBLP | `trust VANET` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 32 | DBLP | `reputation VANET` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 33 | DBLP | `attack V2X` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 34 | DBLP | `anomaly V2X` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 35 | DBLP | `jamming vehicular` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 36 | DBLP | `plausibility vehicular` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 37 | DBLP | `falsification vehicular` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 38 | DBLP | `trust vehicular networks` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 39 | DBLP | `intrusion internet of vehicles` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 40 | DBLP | `attack VANET detection` | 2026-10-17 | blocked (anti-bot challenge, HTTP 202) | 0 | 0 |
| 41 | Crossref | `misbehavior detection vehicular` | 2026-10-17 | 1649002 | 200 | 127 |
| 42 | Crossref | `misbehaviour detection V2X` | 2026-10-17 | 1561913 | 200 | 50 |
| 43 | Crossref | `VeReMi misbehavior dataset` | 2026-10-17 | 551472 | 200 | 31 |
| 44 | Crossref | `Sybil attack VANET detection` | 2026-10-17 | 1639043 | 200 | 110 |
| 45 | Crossref | `position falsification VANET` | 2026-10-17 | 205707 | 200 | 27 |
| 46 | Crossref | `trust management vehicular ad hoc networks` | 2026-10-17 | 5919538 | 200 | 57 |
| 47 | Crossref | `intrusion detection vehicular ad hoc network` | 2026-10-17 | 3081166 | 200 | 65 |
| 48 | Crossref | `F2MD misbehavior detection framework` | 2026-10-17 | 2400236 | 200 | 94 |
| 49 | Crossref | `misbehavior reporting cooperative intelligent transport systems` | 2026-10-17 | 5786974 | 200 | 17 |
| 50 | Crossref | `plausibility check vehicular` | 2026-10-17 | 135126 | 200 | 13 |
| 51 | SemanticScholar | `misbehavior detection vehicular networks` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 52 | SemanticScholar | `misbehaviour detection V2X` | 2026-10-17 | 1509 | 100 | 63 |
| 53 | SemanticScholar | `misbehavior detection VANET machine learning` | 2026-10-17 | 489 | 200 | 166 |
| 54 | SemanticScholar | `misbehavior detection connected vehicles deep learning` | 2026-10-17 | 19497 | 200 | 63 |
| 55 | SemanticScholar | `VeReMi dataset misbehavior` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 56 | SemanticScholar | `VeReMi extension` | 2026-10-17 | 1533342 | 100 | 28 |
| 57 | SemanticScholar | `F2MD framework misbehavior detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 58 | SemanticScholar | `plausibility checks V2X messages` | 2026-10-17 | 131 | 100 | 32 |
| 59 | SemanticScholar | `global misbehavior detection misbehavior authority` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 60 | SemanticScholar | `misbehavior reporting V2X` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 61 | SemanticScholar | `local misbehavior detection CAM BSM` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 62 | SemanticScholar | `position falsification attack VANET detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 63 | SemanticScholar | `Sybil attack detection VANET` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 64 | SemanticScholar | `Sybil attack vehicular ad hoc networks` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 65 | SemanticScholar | `ghost vehicle attack V2X` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 66 | SemanticScholar | `replay attack vehicular communication` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 67 | SemanticScholar | `jamming attack detection vehicular networks` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 68 | SemanticScholar | `denial of service attack VANET detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 69 | SemanticScholar | `intrusion detection system VANET` | 2026-10-17 | 61190 | 100 | 96 |
| 70 | SemanticScholar | `intrusion detection internet of vehicles deep learning` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 71 | SemanticScholar | `anomaly detection V2X messages` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 72 | SemanticScholar | `trust management VANET` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 73 | SemanticScholar | `reputation system vehicular ad hoc networks` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 74 | SemanticScholar | `trust model connected vehicles data trust` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 75 | SemanticScholar | `federated learning misbehavior detection vehicular` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 76 | SemanticScholar | `graph neural network misbehavior detection vehicular` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 77 | SemanticScholar | `transformer misbehavior detection V2X` | 2026-10-17 | 139 | 100 | 67 |
| 78 | SemanticScholar | `self-supervised anomaly detection vehicular network` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 79 | SemanticScholar | `adversarial attacks misbehavior detection vehicular` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 80 | SemanticScholar | `adversarial robustness intrusion detection connected vehicles` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 81 | SemanticScholar | `C-ITS misbehaviour detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 82 | SemanticScholar | `cooperative awareness message attack detection` | 2026-10-17 | 899 | 100 | 36 |
| 83 | SemanticScholar | `basic safety message falsification detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 84 | SemanticScholar | `false data injection connected vehicles` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 85 | SemanticScholar | `collective perception misbehavior detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 86 | SemanticScholar | `cooperative adaptive cruise control attack platoon` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 87 | SemanticScholar | `platooning attack detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 88 | SemanticScholar | `GPS spoofing detection vehicular` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 89 | SemanticScholar | `data-centric misbehavior detection VANET` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 90 | SemanticScholar | `node-centric misbehavior detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 91 | SemanticScholar | `malicious vehicle detection VANET` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 92 | SemanticScholar | `V2X attack simulation framework` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 93 | SemanticScholar | `misbehavior detection dataset vehicular benchmark` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 94 | SemanticScholar | `LSTM misbehavior detection vehicular` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 95 | SemanticScholar | `cellular V2X security attacks` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 96 | SemanticScholar | `black hole attack VANET detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 97 | SemanticScholar | `pseudonym change misbehavior detection Sybil` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 98 | SemanticScholar | `revocation misbehaving vehicles detection` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 99 | SemanticScholar | `trust-based misbehavior detection connected autonomous vehicles` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 100 | SemanticScholar | `V2X security attack impact traffic safety` | 2026-10-17 | HTTP 429 (shared unauthenticated rate limit) | 0 | 0 |
| 101 | OpenAlex | `misbehavior detection vehicular networks` | 2026-10-17 | 2752 | 400 | 318 |
| 102 | OpenAlex | `misbehaviour detection V2X` | 2026-10-17 | 297 | 297 | 161 |
| 103 | OpenAlex | `misbehavior detection VANET machine learning` | 2026-10-17 | 1065 | 400 | 264 |
| 104 | OpenAlex | `misbehavior detection connected vehicles deep learning` | 2026-10-17 | 1532 | 400 | 195 |
| 105 | OpenAlex | `VeReMi dataset misbehavior` | 2026-10-17 | 269 | 269 | 183 |
| 106 | OpenAlex | `VeReMi extension` | 2026-10-17 | 518 | 400 | 107 |
| 107 | OpenAlex | `F2MD framework misbehavior detection` | 2026-10-17 | 35 | 35 | 25 |
| 108 | OpenAlex | `plausibility checks V2X messages` | 2026-10-17 | 548 | 400 | 133 |
| 109 | OpenAlex | `global misbehavior detection misbehavior authority` | 2026-10-17 | 3084 | 400 | 108 |
| 110 | OpenAlex | `misbehavior reporting V2X` | 2026-10-17 | 640 | 400 | 183 |
| 111 | OpenAlex | `local misbehavior detection CAM BSM` | 2026-10-17 | 67 | 67 | 25 |
| 112 | OpenAlex | `position falsification attack VANET detection` | 2026-10-17 | 789 | 400 | 246 |
| 113 | OpenAlex | `Sybil attack detection VANET` | 2026-10-17 | 2436 | 400 | 347 |
| 114 | OpenAlex | `Sybil attack vehicular ad hoc networks` | 2026-10-17 | 3500 | 400 | 287 |
| 115 | OpenAlex | `ghost vehicle attack V2X` | 2026-10-17 | 182 | 182 | 53 |
| 116 | OpenAlex | `replay attack vehicular communication` | 2026-10-17 | 7515 | 400 | 149 |
| 117 | OpenAlex | `jamming attack detection vehicular networks` | 2026-10-17 | 8157 | 400 | 215 |
| 118 | OpenAlex | `denial of service attack VANET detection` | 2026-10-17 | 4139 | 400 | 326 |
| 119 | OpenAlex | `intrusion detection system VANET` | 2026-10-17 | 6154 | 400 | 313 |
| 120 | OpenAlex | `intrusion detection internet of vehicles deep learning` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 121 | OpenAlex | `anomaly detection V2X messages` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 122 | OpenAlex | `trust management VANET` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 123 | OpenAlex | `reputation system vehicular ad hoc networks` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 124 | OpenAlex | `trust model connected vehicles data trust` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 125 | OpenAlex | `federated learning misbehavior detection vehicular` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 126 | OpenAlex | `graph neural network misbehavior detection vehicular` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 127 | OpenAlex | `transformer misbehavior detection V2X` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 128 | OpenAlex | `self-supervised anomaly detection vehicular network` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 129 | OpenAlex | `adversarial attacks misbehavior detection vehicular` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 130 | OpenAlex | `adversarial robustness intrusion detection connected vehicles` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 131 | OpenAlex | `C-ITS misbehaviour detection` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 132 | OpenAlex | `cooperative awareness message attack detection` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 133 | OpenAlex | `basic safety message falsification detection` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 134 | OpenAlex | `false data injection connected vehicles` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 135 | OpenAlex | `collective perception misbehavior detection` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 136 | OpenAlex | `cooperative adaptive cruise control attack platoon` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 137 | OpenAlex | `platooning attack detection` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 138 | OpenAlex | `GPS spoofing detection vehicular` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 139 | OpenAlex | `data-centric misbehavior detection VANET` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 140 | OpenAlex | `node-centric misbehavior detection` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 141 | OpenAlex | `malicious vehicle detection VANET` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 142 | OpenAlex | `V2X attack simulation framework` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 143 | OpenAlex | `misbehavior detection dataset vehicular benchmark` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 144 | OpenAlex | `LSTM misbehavior detection vehicular` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 145 | OpenAlex | `cellular V2X security attacks` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 146 | OpenAlex | `black hole attack VANET detection` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 147 | OpenAlex | `pseudonym change misbehavior detection Sybil` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 148 | OpenAlex | `revocation misbehaving vehicles detection` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 149 | OpenAlex | `trust-based misbehavior detection connected autonomous vehicles` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 150 | OpenAlex | `V2X security attack impact traffic safety` | 2026-10-17 | HTTP 429 (daily credit quota exhausted) | 0 | 0 |
| 151 | Crossref | `MisbehaviorX dataset V2X misbehavior` | 2026-10-17 | 556112 | 200 | 39 |
| 152 | Crossref | `VeReMi NextGen misbehavior dataset` | 2026-10-17 | 552877 | 200 | 22 |
| 153 | Crossref | `BurST-ADMA misbehavior` | 2026-10-17 | 35563 | 200 | 8 |
| 154 | Crossref | `SPADE V2X misbehavior dataset` | 2026-10-17 | 557288 | 200 | 33 |
| 155 | Crossref | `PQ-V2X post-quantum misbehavior` | 2026-10-17 | 1854886 | 200 | 24 |
| 156 | Crossref | `DARE dataset V2X attack` | 2026-10-17 | 649030 | 200 | 27 |
| 157 | Crossref | `VASP V2X attack simulation platform` | 2026-10-17 | 1328992 | 200 | 31 |
| 158 | arXiv | `all:VASP AND all:V2X` | 2026-10-17 | 0 | 0 | 0 |
| 159 | arXiv | `all:"misbehavior detection" AND all:dataset` | 2026-10-17 | 11 | 11 | 7 |
| 160 | arXiv | `all:BurST-ADMA` | 2026-10-17 | 0 | 0 | 0 |
| 161 | arXiv | `all:"ghost vehicle"` | 2026-10-17 | 2 | 2 | 0 |
| 162 | Crossref | `misbehavior authority revocation V2X time` | 2026-10-17 | 1917511 | 200 | 40 |

## Source availability notes (2026-10-17)

- OpenAlex: daily credit quota for this IP was exhausted mid-run (HTTP 429, retry-after about 3.8 h, shared with parallel cluster agents). 19 of 50 queries completed; the remaining 31 are logged as failed and should be rerun after the quota resets. DOI enrichment used Semantic Scholar batch and Crossref instead.
- DBLP: every request was answered with an anti-bot challenge page (HTTP 202, also on the uni-trier and dagstuhl mirrors); all 20 DBLP queries failed. Rerun from a browser session or later.
- Semantic Scholar: unauthenticated shared pool returned HTTP 429 most of the time; 8 of 50 search queries completed. Snowballing used Semantic Scholar where it answered (524 successful reference/citation calls) and OpenCitations (references and citations by DOI, 723 seeds, about 12,000 linked DOIs, metadata from Crossref) where it did not.
- arXiv (20 queries + 4 targeted) and Crossref (10 queries + 8 targeted) completed fully.
- Relevance labels are keyword-rule assignments, not manual judgements; summarisers should re-grade. About 24% of records lack an abstract (mostly Crossref-only metadata).
