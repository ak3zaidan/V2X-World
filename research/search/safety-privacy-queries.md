# safety-privacy: search log

Cluster scope: physical/safety impact of V2X attacks and defences (CACC/platooning, intersections/SPaT/I-SIG, cooperative perception, VRU, surrogate safety, closed loop), safety cost of security, ISO/SAE 21434 / TARA; V2X location privacy (pseudonym change, mix zones, silent periods, tracking/linking, linkability metrics, cross-layer identifiers); C-V2X / NR sidelink security and the DSRC to C-V2X transition.

Date of all queries: 2026-10-17. User-Agent `v2x-lit-review/1.0`, no email or name in any request.

## Source availability on the day (important for coverage)

- **Crossref** (`query.bibliographic`, 40 rows/query): worked for all queries. Main source.
- **DataCite** (`publisher:arXiv`, all words ANDed, 100 rows/query): used as the arXiv substitute while the arXiv API was rate-limited; covers arXiv records with DOI 10.48550 and abstracts.
- **arXiv API** (`all:` terms ANDed, 60 results): HTTP 429 for most of the session (other review agents share the IP); run again late in the session, see per-query rows (`ok=false` means the call failed after retries).
- **Semantic Scholar** search and references/citations: HTTP 429 throughout (unauthenticated shared pool). No S2 results. Snowballing was done with **OpenCitations COCI** instead (citations and references by DOI), with candidate metadata resolved via Crossref DOI-filter batches.
- **OpenAlex** search: daily credit budget exhausted (HTTP 429, Retry-After about 3.8 h). The remaining 8 credits were spent on DOI-filter batches to add abstracts, OA PDF links and citation counts to 795 records, high relevance first.
- **DBLP**: blocked by a bot challenge page (HTTP 202 HTML) on dblp.org and both mirrors. No DBLP results.

A follow-up pass with Semantic Scholar, OpenAlex search and DBLP is advisable when their limits reset, to close possible gaps (mainly arXiv-only and workshop papers).

## Inclusion and relevance

Rule-based screening on title and abstract (scripts kept in the session scratchpad): a record needs a vehicular term and a security/privacy term. **high** = title matches a cluster topic (platoon/CACC attack, signal/intersection attack, cooperative perception attack/trust, VRU+security, safety impact of attacks, safety cost of security, V2X TARA/21434, pseudonym change/location privacy/tracking, cross-layer identifiers, C-V2X/sidelink security). **medium** = cluster topic only in abstract, or specific V2X attack/misbehaviour detection titles, or secondary variants (control-theoretic resilient platoon control, pseudonym authentication schemes, non-V2X automotive TARA). **low** = general vehicular security/privacy (e.g. privacy-preserving authentication), kept only from direct queries, not from snowballing. In-vehicle-only (CAN/ECU), UAV, marine and front-matter records were excluded.

## Snowballing

- Round 1: 342 high-relevance seeds with DOI; 6186 new candidate DOIs from OpenCitations citations+references; 2002 passed screening.
- Round 2: 503 new high-relevance seeds; 12665 new candidate DOIs; 2713 passed screening.
- After merging and dropping low-relevance snowball hits: 1507 records in the file came only from snowballing ({'high': 551, 'medium': 956}).

## Queries

| # | Query | Source | Hits | Kept (first found) | Status |
|---|---|---|---|---|---|
| 1 | CACC attack cooperative adaptive cruise control | arXiv | 15 | 3 | ok |
| 1 | CACC attack cooperative adaptive cruise control | DataCite-arXiv | 10 | 9 | ok |
| 1 | CACC attack cooperative adaptive cruise control | Crossref | 40 | 6 | ok |
| 1 | CACC attack cooperative adaptive cruise control | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 1 | CACC attack cooperative adaptive cruise control | OpenAlex | - | 0 | budget exhausted (not run) |
| 1 | CACC attack cooperative adaptive cruise control | DBLP | - | 0 | bot challenge (not run) |
| 2 | platooning security attack | arXiv | 30 | 6 | ok |
| 2 | platooning security attack | DataCite-arXiv | 6 | 5 | ok |
| 2 | platooning security attack | Crossref | 40 | 11 | ok |
| 2 | platooning security attack | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 2 | platooning security attack | OpenAlex | - | 0 | budget exhausted (not run) |
| 2 | platooning security attack | DBLP | - | 0 | bot challenge (not run) |
| 3 | vehicle platoon cyber attack string stability | arXiv | 2 | 0 | ok |
| 3 | vehicle platoon cyber attack string stability | DataCite-arXiv | 1 | 1 | ok |
| 3 | vehicle platoon cyber attack string stability | Crossref | 40 | 10 | ok |
| 3 | vehicle platoon cyber attack string stability | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 3 | vehicle platoon cyber attack string stability | OpenAlex | - | 0 | budget exhausted (not run) |
| 3 | vehicle platoon cyber attack string stability | DBLP | - | 0 | bot challenge (not run) |
| 4 | platoon jamming falsification attack mitigation | arXiv | 0 | 0 | ok |
| 4 | platoon jamming falsification attack mitigation | DataCite-arXiv | 0 | 0 | ok |
| 4 | platoon jamming falsification attack mitigation | Crossref | 40 | 6 | ok |
| 4 | platoon jamming falsification attack mitigation | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 4 | platoon jamming falsification attack mitigation | OpenAlex | - | 0 | budget exhausted (not run) |
| 4 | platoon jamming falsification attack mitigation | DBLP | - | 0 | bot challenge (not run) |
| 5 | resilient CACC misbehavior detection | arXiv | 1 | 0 | ok |
| 5 | resilient CACC misbehavior detection | DataCite-arXiv | 1 | 0 | ok |
| 5 | resilient CACC misbehavior detection | Crossref | 40 | 18 | ok |
| 5 | resilient CACC misbehavior detection | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 5 | resilient CACC misbehavior detection | OpenAlex | - | 0 | budget exhausted (not run) |
| 5 | resilient CACC misbehavior detection | DBLP | - | 0 | bot challenge (not run) |
| 6 | platoon Sybil attack | arXiv | 0 | 0 | ok |
| 6 | platoon Sybil attack | DataCite-arXiv | 0 | 0 | ok |
| 6 | platoon Sybil attack | Crossref | 40 | 9 | ok |
| 6 | platoon Sybil attack | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 6 | platoon Sybil attack | OpenAlex | - | 0 | budget exhausted (not run) |
| 6 | platoon Sybil attack | DBLP | - | 0 | bot challenge (not run) |
| 7 | cooperative driving attack simulation Plexe | arXiv | 0 | 0 | ok |
| 7 | cooperative driving attack simulation Plexe | DataCite-arXiv | 0 | 0 | ok |
| 7 | cooperative driving attack simulation Plexe | Crossref | 40 | 3 | ok |
| 7 | cooperative driving attack simulation Plexe | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 7 | cooperative driving attack simulation Plexe | OpenAlex | - | 0 | budget exhausted (not run) |
| 7 | cooperative driving attack simulation Plexe | DBLP | - | 0 | bot challenge (not run) |
| 8 | traffic signal control attack connected vehicles | arXiv | 16 | 5 | ok |
| 8 | traffic signal control attack connected vehicles | DataCite-arXiv | 4 | 4 | ok |
| 8 | traffic signal control attack connected vehicles | Crossref | 40 | 4 | ok |
| 8 | traffic signal control attack connected vehicles | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 8 | traffic signal control attack connected vehicles | OpenAlex | - | 0 | budget exhausted (not run) |
| 8 | traffic signal control attack connected vehicles | DBLP | - | 0 | bot challenge (not run) |
| 9 | I-SIG attack intelligent traffic signal | arXiv | 1 | 0 | ok |
| 9 | I-SIG attack intelligent traffic signal | DataCite-arXiv | 0 | 0 | ok |
| 9 | I-SIG attack intelligent traffic signal | Crossref | 40 | 3 | ok |
| 9 | I-SIG attack intelligent traffic signal | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 9 | I-SIG attack intelligent traffic signal | OpenAlex | - | 0 | budget exhausted (not run) |
| 9 | I-SIG attack intelligent traffic signal | DBLP | - | 0 | bot challenge (not run) |
| 10 | SPaT message spoofing attack | arXiv | 0 | 0 | ok |
| 10 | SPaT message spoofing attack | DataCite-arXiv | 0 | 0 | ok |
| 10 | SPaT message spoofing attack | Crossref | 40 | 0 | ok |
| 10 | SPaT message spoofing attack | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 10 | SPaT message spoofing attack | OpenAlex | - | 0 | budget exhausted (not run) |
| 10 | SPaT message spoofing attack | DBLP | - | 0 | bot challenge (not run) |
| 11 | intersection movement assist attack V2X | arXiv | 0 | 0 | ok |
| 11 | intersection movement assist attack V2X | DataCite-arXiv | 0 | 0 | ok |
| 11 | intersection movement assist attack V2X | Crossref | 40 | 9 | ok |
| 11 | intersection movement assist attack V2X | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 11 | intersection movement assist attack V2X | OpenAlex | - | 0 | budget exhausted (not run) |
| 11 | intersection movement assist attack V2X | DBLP | - | 0 | bot challenge (not run) |
| 12 | traffic light manipulation attack vehicular network | arXiv | 0 | 0 | ok |
| 12 | traffic light manipulation attack vehicular network | DataCite-arXiv | 0 | 0 | ok |
| 12 | traffic light manipulation attack vehicular network | Crossref | 40 | 3 | ok |
| 12 | traffic light manipulation attack vehicular network | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 12 | traffic light manipulation attack vehicular network | OpenAlex | - | 0 | budget exhausted (not run) |
| 12 | traffic light manipulation attack vehicular network | DBLP | - | 0 | bot challenge (not run) |
| 13 | cooperative perception attack | arXiv | 14 | 5 | ok |
| 13 | cooperative perception attack | DataCite-arXiv | 4 | 2 | ok |
| 13 | cooperative perception attack | Crossref | 40 | 5 | ok |
| 13 | cooperative perception attack | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 13 | cooperative perception attack | OpenAlex | - | 0 | budget exhausted (not run) |
| 13 | cooperative perception attack | DBLP | - | 0 | bot challenge (not run) |
| 14 | collective perception message misbehavior | arXiv | 2 | 0 | ok |
| 14 | collective perception message misbehavior | DataCite-arXiv | 1 | 0 | ok |
| 14 | collective perception message misbehavior | Crossref | 40 | 10 | ok |
| 14 | collective perception message misbehavior | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 14 | collective perception message misbehavior | OpenAlex | - | 0 | budget exhausted (not run) |
| 14 | collective perception message misbehavior | DBLP | - | 0 | bot challenge (not run) |
| 15 | cooperative perception trust adversarial | arXiv | 2 | 0 | ok |
| 15 | cooperative perception trust adversarial | DataCite-arXiv | 1 | 0 | ok |
| 15 | cooperative perception trust adversarial | Crossref | 40 | 4 | ok |
| 15 | cooperative perception trust adversarial | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 15 | cooperative perception trust adversarial | OpenAlex | - | 0 | budget exhausted (not run) |
| 15 | cooperative perception trust adversarial | DBLP | - | 0 | bot challenge (not run) |
| 16 | V2X perception data fabrication attack | arXiv | 1 | 0 | ok |
| 16 | V2X perception data fabrication attack | DataCite-arXiv | 1 | 1 | ok |
| 16 | V2X perception data fabrication attack | Crossref | 40 | 4 | ok |
| 16 | V2X perception data fabrication attack | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 16 | V2X perception data fabrication attack | OpenAlex | - | 0 | budget exhausted (not run) |
| 16 | V2X perception data fabrication attack | DBLP | - | 0 | bot challenge (not run) |
| 17 | adversarial attack multi-agent cooperative perception | arXiv | 2 | 0 | ok |
| 17 | adversarial attack multi-agent cooperative perception | DataCite-arXiv | 0 | 0 | ok |
| 17 | adversarial attack multi-agent cooperative perception | Crossref | 40 | 2 | ok |
| 17 | adversarial attack multi-agent cooperative perception | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 17 | adversarial attack multi-agent cooperative perception | OpenAlex | - | 0 | budget exhausted (not run) |
| 17 | adversarial attack multi-agent cooperative perception | DBLP | - | 0 | bot challenge (not run) |
| 18 | V2X attack safety impact simulation | arXiv | 4 | 0 | ok |
| 18 | V2X attack safety impact simulation | DataCite-arXiv | 0 | 0 | ok |
| 18 | V2X attack safety impact simulation | Crossref | 40 | 4 | ok |
| 18 | V2X attack safety impact simulation | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 18 | V2X attack safety impact simulation | OpenAlex | - | 0 | budget exhausted (not run) |
| 18 | V2X attack safety impact simulation | DBLP | - | 0 | bot challenge (not run) |
| 19 | VANET attack traffic impact collision | arXiv | 0 | 0 | ok |
| 19 | VANET attack traffic impact collision | DataCite-arXiv | 0 | 0 | ok |
| 19 | VANET attack traffic impact collision | Crossref | 40 | 12 | ok |
| 19 | VANET attack traffic impact collision | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 19 | VANET attack traffic impact collision | OpenAlex | - | 0 | budget exhausted (not run) |
| 19 | VANET attack traffic impact collision | DBLP | - | 0 | bot challenge (not run) |
| 20 | surrogate safety measures cyber attack connected vehicles | arXiv | 0 | 0 | ok |
| 20 | surrogate safety measures cyber attack connected vehicles | DataCite-arXiv | 0 | 0 | ok |
| 20 | surrogate safety measures cyber attack connected vehicles | Crossref | 40 | 19 | ok |
| 20 | surrogate safety measures cyber attack connected vehicles | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 20 | surrogate safety measures cyber attack connected vehicles | OpenAlex | - | 0 | budget exhausted (not run) |
| 20 | surrogate safety measures cyber attack connected vehicles | DBLP | - | 0 | bot challenge (not run) |
| 21 | time to collision false data injection connected vehicles | arXiv | 0 | 0 | ok |
| 21 | time to collision false data injection connected vehicles | DataCite-arXiv | 1 | 1 | ok |
| 21 | time to collision false data injection connected vehicles | Crossref | 40 | 14 | ok |
| 21 | time to collision false data injection connected vehicles | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 21 | time to collision false data injection connected vehicles | OpenAlex | - | 0 | budget exhausted (not run) |
| 21 | time to collision false data injection connected vehicles | DBLP | - | 0 | bot challenge (not run) |
| 22 | closed-loop evaluation misbehavior detection V2X | arXiv | 0 | 0 | ok |
| 22 | closed-loop evaluation misbehavior detection V2X | DataCite-arXiv | 0 | 0 | ok |
| 22 | closed-loop evaluation misbehavior detection V2X | Crossref | 40 | 16 | ok |
| 22 | closed-loop evaluation misbehavior detection V2X | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 22 | closed-loop evaluation misbehavior detection V2X | OpenAlex | - | 0 | budget exhausted (not run) |
| 22 | closed-loop evaluation misbehavior detection V2X | DBLP | - | 0 | bot challenge (not run) |
| 23 | position falsification attack safety application | arXiv | 1 | 0 | ok |
| 23 | position falsification attack safety application | DataCite-arXiv | 0 | 0 | ok |
| 23 | position falsification attack safety application | Crossref | 40 | 13 | ok |
| 23 | position falsification attack safety application | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 23 | position falsification attack safety application | OpenAlex | - | 0 | budget exhausted (not run) |
| 23 | position falsification attack safety application | DBLP | - | 0 | bot challenge (not run) |
| 24 | false data injection connected automated vehicles | arXiv | 4 | 0 | ok |
| 24 | false data injection connected automated vehicles | DataCite-arXiv | 3 | 0 | ok |
| 24 | false data injection connected automated vehicles | Crossref | 40 | 1 | ok |
| 24 | false data injection connected automated vehicles | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 24 | false data injection connected automated vehicles | OpenAlex | - | 0 | budget exhausted (not run) |
| 24 | false data injection connected automated vehicles | DBLP | - | 0 | bot challenge (not run) |
| 25 | VRU safety V2X security | arXiv | 0 | 0 | ok |
| 25 | VRU safety V2X security | DataCite-arXiv | 0 | 0 | ok |
| 25 | VRU safety V2X security | Crossref | 40 | 7 | ok |
| 25 | VRU safety V2X security | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 25 | VRU safety V2X security | OpenAlex | - | 0 | budget exhausted (not run) |
| 25 | VRU safety V2X security | DBLP | - | 0 | bot challenge (not run) |
| 26 | pedestrian V2X attack | arXiv | 2 | 1 | ok |
| 26 | pedestrian V2X attack | DataCite-arXiv | 0 | 0 | ok |
| 26 | pedestrian V2X attack | Crossref | 40 | 6 | ok |
| 26 | pedestrian V2X attack | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 26 | pedestrian V2X attack | OpenAlex | - | 0 | budget exhausted (not run) |
| 26 | pedestrian V2X attack | DBLP | - | 0 | bot challenge (not run) |
| 27 | V2X security latency safety trade-off | arXiv | 1 | 0 | ok |
| 27 | V2X security latency safety trade-off | DataCite-arXiv | 1 | 1 | ok |
| 27 | V2X security latency safety trade-off | Crossref | 40 | 1 | ok |
| 27 | V2X security latency safety trade-off | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 27 | V2X security latency safety trade-off | OpenAlex | - | 0 | budget exhausted (not run) |
| 27 | V2X security latency safety trade-off | DBLP | - | 0 | bot challenge (not run) |
| 28 | signature verification overhead safety V2X | arXiv | 1 | 0 | ok |
| 28 | signature verification overhead safety V2X | DataCite-arXiv | 0 | 0 | ok |
| 28 | signature verification overhead safety V2X | Crossref | 40 | 2 | ok |
| 28 | signature verification overhead safety V2X | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 28 | signature verification overhead safety V2X | OpenAlex | - | 0 | budget exhausted (not run) |
| 28 | signature verification overhead safety V2X | DBLP | - | 0 | bot challenge (not run) |
| 29 | message authentication delay cooperative awareness safety | arXiv | 0 | 0 | ok |
| 29 | message authentication delay cooperative awareness safety | DataCite-arXiv | 0 | 0 | ok |
| 29 | message authentication delay cooperative awareness safety | Crossref | 40 | 5 | ok |
| 29 | message authentication delay cooperative awareness safety | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 29 | message authentication delay cooperative awareness safety | OpenAlex | - | 0 | budget exhausted (not run) |
| 29 | message authentication delay cooperative awareness safety | DBLP | - | 0 | bot challenge (not run) |
| 30 | ISO/SAE 21434 threat analysis risk assessment | arXiv | 5 | 0 | ok |
| 30 | ISO/SAE 21434 threat analysis risk assessment | DataCite-arXiv | 5 | 4 | ok |
| 30 | ISO/SAE 21434 threat analysis risk assessment | Crossref | 40 | 17 | ok |
| 30 | ISO/SAE 21434 threat analysis risk assessment | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 30 | ISO/SAE 21434 threat analysis risk assessment | OpenAlex | - | 0 | budget exhausted (not run) |
| 30 | ISO/SAE 21434 threat analysis risk assessment | DBLP | - | 0 | bot challenge (not run) |
| 31 | TARA automotive simulation quantitative | arXiv | 0 | 0 | ok |
| 31 | TARA automotive simulation quantitative | DataCite-arXiv | 0 | 0 | ok |
| 31 | TARA automotive simulation quantitative | Crossref | 40 | 3 | ok |
| 31 | TARA automotive simulation quantitative | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 31 | TARA automotive simulation quantitative | OpenAlex | - | 0 | budget exhausted (not run) |
| 31 | TARA automotive simulation quantitative | DBLP | - | 0 | bot challenge (not run) |
| 32 | V2X risk assessment attack feasibility | arXiv | 0 | 0 | ok |
| 32 | V2X risk assessment attack feasibility | DataCite-arXiv | 0 | 0 | ok |
| 32 | V2X risk assessment attack feasibility | Crossref | 40 | 0 | ok |
| 32 | V2X risk assessment attack feasibility | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 32 | V2X risk assessment attack feasibility | OpenAlex | - | 0 | budget exhausted (not run) |
| 32 | V2X risk assessment attack feasibility | DBLP | - | 0 | bot challenge (not run) |
| 33 | automotive cybersecurity attack tree vehicle-to-everything | arXiv | 0 | 0 | ok |
| 33 | automotive cybersecurity attack tree vehicle-to-everything | DataCite-arXiv | 0 | 0 | ok |
| 33 | automotive cybersecurity attack tree vehicle-to-everything | Crossref | 40 | 21 | ok |
| 33 | automotive cybersecurity attack tree vehicle-to-everything | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 33 | automotive cybersecurity attack tree vehicle-to-everything | OpenAlex | - | 0 | budget exhausted (not run) |
| 33 | automotive cybersecurity attack tree vehicle-to-everything | DBLP | - | 0 | bot challenge (not run) |
| 34 | pseudonym change strategy vehicular | arXiv | 2 | 0 | ok |
| 34 | pseudonym change strategy vehicular | DataCite-arXiv | 0 | 0 | ok |
| 34 | pseudonym change strategy vehicular | Crossref | 40 | 36 | ok |
| 34 | pseudonym change strategy vehicular | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 34 | pseudonym change strategy vehicular | OpenAlex | - | 0 | budget exhausted (not run) |
| 34 | pseudonym change strategy vehicular | DBLP | - | 0 | bot challenge (not run) |
| 35 | mix zone vehicular network | arXiv | 5 | 0 | ok |
| 35 | mix zone vehicular network | DataCite-arXiv | 0 | 0 | ok |
| 35 | mix zone vehicular network | Crossref | 40 | 7 | ok |
| 35 | mix zone vehicular network | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 35 | mix zone vehicular network | OpenAlex | - | 0 | budget exhausted (not run) |
| 35 | mix zone vehicular network | DBLP | - | 0 | bot challenge (not run) |
| 36 | silent period pseudonym change | arXiv | 1 | 0 | ok |
| 36 | silent period pseudonym change | DataCite-arXiv | 1 | 1 | ok |
| 36 | silent period pseudonym change | Crossref | 40 | 2 | ok |
| 36 | silent period pseudonym change | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 36 | silent period pseudonym change | OpenAlex | - | 0 | budget exhausted (not run) |
| 36 | silent period pseudonym change | DBLP | - | 0 | bot challenge (not run) |
| 37 | location privacy VANET | arXiv | 5 | 1 | ok |
| 37 | location privacy VANET | DataCite-arXiv | 2 | 2 | ok |
| 37 | location privacy VANET | Crossref | 40 | 34 | ok |
| 37 | location privacy VANET | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 37 | location privacy VANET | OpenAlex | - | 0 | budget exhausted (not run) |
| 37 | location privacy VANET | DBLP | - | 0 | bot challenge (not run) |
| 38 | vehicle tracking attack pseudonym linking | arXiv | 1 | 0 | ok |
| 38 | vehicle tracking attack pseudonym linking | DataCite-arXiv | 0 | 0 | ok |
| 38 | vehicle tracking attack pseudonym linking | Crossref | 40 | 17 | ok |
| 38 | vehicle tracking attack pseudonym linking | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 38 | vehicle tracking attack pseudonym linking | OpenAlex | - | 0 | budget exhausted (not run) |
| 38 | vehicle tracking attack pseudonym linking | DBLP | - | 0 | bot challenge (not run) |
| 39 | linkability V2X pseudonym | arXiv | 0 | 0 | ok |
| 39 | linkability V2X pseudonym | DataCite-arXiv | 0 | 0 | ok |
| 39 | linkability V2X pseudonym | Crossref | 40 | 10 | ok |
| 39 | linkability V2X pseudonym | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 39 | linkability V2X pseudonym | OpenAlex | - | 0 | budget exhausted (not run) |
| 39 | linkability V2X pseudonym | DBLP | - | 0 | bot challenge (not run) |
| 40 | privacy metric vehicular network tracking | arXiv | 0 | 0 | ok |
| 40 | privacy metric vehicular network tracking | DataCite-arXiv | 0 | 0 | ok |
| 40 | privacy metric vehicular network tracking | Crossref | 40 | 20 | ok |
| 40 | privacy metric vehicular network tracking | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 40 | privacy metric vehicular network tracking | OpenAlex | - | 0 | budget exhausted (not run) |
| 40 | privacy metric vehicular network tracking | DBLP | - | 0 | bot challenge (not run) |
| 41 | cooperative pseudonym change | arXiv | 3 | 1 | ok |
| 41 | cooperative pseudonym change | DataCite-arXiv | 1 | 1 | ok |
| 41 | cooperative pseudonym change | Crossref | 40 | 3 | ok |
| 41 | cooperative pseudonym change | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 41 | cooperative pseudonym change | OpenAlex | - | 0 | budget exhausted (not run) |
| 41 | cooperative pseudonym change | DBLP | - | 0 | bot challenge (not run) |
| 42 | pseudonym change evaluation simulation PREXT | arXiv | 0 | 0 | ok |
| 42 | pseudonym change evaluation simulation PREXT | DataCite-arXiv | 0 | 0 | ok |
| 42 | pseudonym change evaluation simulation PREXT | Crossref | 40 | 2 | ok |
| 42 | pseudonym change evaluation simulation PREXT | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 42 | pseudonym change evaluation simulation PREXT | OpenAlex | - | 0 | budget exhausted (not run) |
| 42 | pseudonym change evaluation simulation PREXT | DBLP | - | 0 | bot challenge (not run) |
| 43 | MAC address randomization vehicular | arXiv | 0 | 0 | ok |
| 43 | MAC address randomization vehicular | DataCite-arXiv | 0 | 0 | ok |
| 43 | MAC address randomization vehicular | Crossref | 40 | 0 | ok |
| 43 | MAC address randomization vehicular | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 43 | MAC address randomization vehicular | OpenAlex | - | 0 | budget exhausted (not run) |
| 43 | MAC address randomization vehicular | DBLP | - | 0 | bot challenge (not run) |
| 44 | RSSI fingerprinting vehicle tracking | arXiv | 0 | 0 | ok |
| 44 | RSSI fingerprinting vehicle tracking | DataCite-arXiv | 0 | 0 | ok |
| 44 | RSSI fingerprinting vehicle tracking | Crossref | 40 | 4 | ok |
| 44 | RSSI fingerprinting vehicle tracking | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 44 | RSSI fingerprinting vehicle tracking | OpenAlex | - | 0 | budget exhausted (not run) |
| 44 | RSSI fingerprinting vehicle tracking | DBLP | - | 0 | bot challenge (not run) |
| 45 | physical layer fingerprinting V2X privacy | arXiv | 0 | 0 | ok |
| 45 | physical layer fingerprinting V2X privacy | DataCite-arXiv | 0 | 0 | ok |
| 45 | physical layer fingerprinting V2X privacy | Crossref | 40 | 6 | ok |
| 45 | physical layer fingerprinting V2X privacy | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 45 | physical layer fingerprinting V2X privacy | OpenAlex | - | 0 | budget exhausted (not run) |
| 45 | physical layer fingerprinting V2X privacy | DBLP | - | 0 | bot challenge (not run) |
| 46 | C-V2X privacy layer 2 identifier | arXiv | 0 | 0 | ok |
| 46 | C-V2X privacy layer 2 identifier | DataCite-arXiv | 0 | 0 | ok |
| 46 | C-V2X privacy layer 2 identifier | Crossref | 40 | 4 | ok |
| 46 | C-V2X privacy layer 2 identifier | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 46 | C-V2X privacy layer 2 identifier | OpenAlex | - | 0 | budget exhausted (not run) |
| 46 | C-V2X privacy layer 2 identifier | DBLP | - | 0 | bot challenge (not run) |
| 47 | semi-persistent scheduling privacy tracking | arXiv | 0 | 0 | ok |
| 47 | semi-persistent scheduling privacy tracking | DataCite-arXiv | 0 | 0 | ok |
| 47 | semi-persistent scheduling privacy tracking | Crossref | 40 | 0 | ok |
| 47 | semi-persistent scheduling privacy tracking | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 47 | semi-persistent scheduling privacy tracking | OpenAlex | - | 0 | budget exhausted (not run) |
| 47 | semi-persistent scheduling privacy tracking | DBLP | - | 0 | bot challenge (not run) |
| 48 | CAM tracking privacy C-ITS | arXiv | 0 | 0 | ok |
| 48 | CAM tracking privacy C-ITS | DataCite-arXiv | 0 | 0 | ok |
| 48 | CAM tracking privacy C-ITS | Crossref | 40 | 1 | ok |
| 48 | CAM tracking privacy C-ITS | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 48 | CAM tracking privacy C-ITS | OpenAlex | - | 0 | budget exhausted (not run) |
| 48 | CAM tracking privacy C-ITS | DBLP | - | 0 | bot challenge (not run) |
| 49 | connected vehicle trajectory privacy BSM | arXiv | 0 | 0 | ok |
| 49 | connected vehicle trajectory privacy BSM | DataCite-arXiv | 0 | 0 | ok |
| 49 | connected vehicle trajectory privacy BSM | Crossref | 40 | 11 | ok |
| 49 | connected vehicle trajectory privacy BSM | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 49 | connected vehicle trajectory privacy BSM | OpenAlex | - | 0 | budget exhausted (not run) |
| 49 | connected vehicle trajectory privacy BSM | DBLP | - | 0 | bot challenge (not run) |
| 50 | C-V2X security attack | arXiv | 11 | 0 | ok |
| 50 | C-V2X security attack | DataCite-arXiv | 6 | 4 | ok |
| 50 | C-V2X security attack | Crossref | 40 | 9 | ok |
| 50 | C-V2X security attack | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 50 | C-V2X security attack | OpenAlex | - | 0 | budget exhausted (not run) |
| 50 | C-V2X security attack | DBLP | - | 0 | bot challenge (not run) |
| 51 | LTE-V2X sidelink jamming attack | arXiv | 0 | 0 | ok |
| 51 | LTE-V2X sidelink jamming attack | DataCite-arXiv | 0 | 0 | ok |
| 51 | LTE-V2X sidelink jamming attack | Crossref | 40 | 1 | ok |
| 51 | LTE-V2X sidelink jamming attack | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 51 | LTE-V2X sidelink jamming attack | OpenAlex | - | 0 | budget exhausted (not run) |
| 51 | LTE-V2X sidelink jamming attack | DBLP | - | 0 | bot challenge (not run) |
| 52 | NR sidelink security | arXiv | 3 | 0 | ok |
| 52 | NR sidelink security | DataCite-arXiv | 2 | 2 | ok |
| 52 | NR sidelink security | Crossref | 40 | 0 | ok |
| 52 | NR sidelink security | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 52 | NR sidelink security | OpenAlex | - | 0 | budget exhausted (not run) |
| 52 | NR sidelink security | DBLP | - | 0 | bot challenge (not run) |
| 53 | semi-persistent scheduling attack C-V2X | arXiv | 1 | 0 | ok |
| 53 | semi-persistent scheduling attack C-V2X | DataCite-arXiv | 1 | 1 | ok |
| 53 | semi-persistent scheduling attack C-V2X | Crossref | 40 | 0 | ok |
| 53 | semi-persistent scheduling attack C-V2X | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 53 | semi-persistent scheduling attack C-V2X | OpenAlex | - | 0 | budget exhausted (not run) |
| 53 | semi-persistent scheduling attack C-V2X | DBLP | - | 0 | bot challenge (not run) |
| 54 | resource exhaustion attack cellular V2X | arXiv | 0 | 0 | ok |
| 54 | resource exhaustion attack cellular V2X | DataCite-arXiv | 0 | 0 | ok |
| 54 | resource exhaustion attack cellular V2X | Crossref | 40 | 0 | ok |
| 54 | resource exhaustion attack cellular V2X | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 54 | resource exhaustion attack cellular V2X | OpenAlex | - | 0 | budget exhausted (not run) |
| 54 | resource exhaustion attack cellular V2X | DBLP | - | 0 | bot challenge (not run) |
| 55 | 5G V2X security vulnerabilities | arXiv | 5 | 1 | ok |
| 55 | 5G V2X security vulnerabilities | DataCite-arXiv | 3 | 2 | ok |
| 55 | 5G V2X security vulnerabilities | Crossref | 40 | 8 | ok |
| 55 | 5G V2X security vulnerabilities | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 55 | 5G V2X security vulnerabilities | OpenAlex | - | 0 | budget exhausted (not run) |
| 55 | 5G V2X security vulnerabilities | DBLP | - | 0 | bot challenge (not run) |
| 56 | DSRC C-V2X coexistence security | arXiv | 0 | 0 | ok |
| 56 | DSRC C-V2X coexistence security | DataCite-arXiv | 0 | 0 | ok |
| 56 | DSRC C-V2X coexistence security | Crossref | 40 | 1 | ok |
| 56 | DSRC C-V2X coexistence security | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 56 | DSRC C-V2X coexistence security | OpenAlex | - | 0 | budget exhausted (not run) |
| 56 | DSRC C-V2X coexistence security | DBLP | - | 0 | bot challenge (not run) |
| 57 | C-V2X misbehavior detection | arXiv | 2 | 0 | ok |
| 57 | C-V2X misbehavior detection | DataCite-arXiv | 2 | 0 | ok |
| 57 | C-V2X misbehavior detection | Crossref | 40 | 4 | ok |
| 57 | C-V2X misbehavior detection | SemanticScholar | - | 0 | HTTP 429 (not obtained) |
| 57 | C-V2X misbehavior detection | OpenAlex | - | 0 | budget exhausted (not run) |
| 57 | C-V2X misbehavior detection | DBLP | - | 0 | bot challenge (not run) |
| 58 | connected vehicles platoon cyberattack detection | arXiv | 4 | 1 | ok |
| 58 | connected vehicles platoon cyberattack detection | DataCite-arXiv | 1 | 1 | ok |
| 58 | connected vehicles platoon cyberattack detection | Crossref | 40 | 7 | ok |
| 59 | vehicular platoon denial of service attack | arXiv | 0 | 0 | ok |
| 59 | vehicular platoon denial of service attack | DataCite-arXiv | 3 | 2 | ok |
| 59 | vehicular platoon denial of service attack | Crossref | 40 | 11 | ok |
| 60 | CACC spoofing attack radar V2V | arXiv | 0 | 0 | ok |
| 60 | CACC spoofing attack radar V2V | DataCite-arXiv | 0 | 0 | ok |
| 60 | CACC spoofing attack radar V2V | Crossref | 40 | 3 | ok |
| 61 | platoon insider attack controller | arXiv | 1 | 0 | ok |
| 61 | platoon insider attack controller | DataCite-arXiv | 0 | 0 | ok |
| 61 | platoon insider attack controller | Crossref | 40 | 2 | ok |
| 62 | attack-resilient cooperative adaptive cruise control observer | arXiv | 1 | 0 | ok |
| 62 | attack-resilient cooperative adaptive cruise control observer | DataCite-arXiv | 1 | 0 | ok |
| 62 | attack-resilient cooperative adaptive cruise control observer | Crossref | 40 | 4 | ok |
| 63 | secure platooning protocol | arXiv | 7 | 0 | ok |
| 63 | secure platooning protocol | DataCite-arXiv | 2 | 2 | ok |
| 63 | secure platooning protocol | Crossref | 40 | 9 | ok |
| 64 | platoon message falsification collision | arXiv | 1 | 0 | ok |
| 64 | platoon message falsification collision | DataCite-arXiv | 0 | 0 | ok |
| 64 | platoon message falsification collision | Crossref | 40 | 2 | ok |
| 65 | connected vehicle traffic signal vulnerability | arXiv | 8 | 0 | ok |
| 65 | connected vehicle traffic signal vulnerability | DataCite-arXiv | 1 | 1 | ok |
| 65 | connected vehicle traffic signal vulnerability | Crossref | 40 | 3 | ok |
| 66 | signal phase and timing security | arXiv | 0 | 0 | ok |
| 66 | signal phase and timing security | DataCite-arXiv | 8 | 1 | ok |
| 66 | signal phase and timing security | Crossref | 40 | 0 | ok |
| 67 | congestion attack intelligent traffic signal system | arXiv | 5 | 1 | ok |
| 67 | congestion attack intelligent traffic signal system | DataCite-arXiv | 1 | 0 | ok |
| 67 | congestion attack intelligent traffic signal system | Crossref | 40 | 1 | ok |
| 68 | green light optimal speed advisory attack | arXiv | 0 | 0 | ok |
| 68 | green light optimal speed advisory attack | DataCite-arXiv | 0 | 0 | ok |
| 68 | green light optimal speed advisory attack | Crossref | 40 | 0 | ok |
| 69 | cooperative intersection management attack | arXiv | 0 | 0 | ok |
| 69 | cooperative intersection management attack | DataCite-arXiv | 0 | 0 | ok |
| 69 | cooperative intersection management attack | Crossref | 40 | 2 | ok |
| 70 | collective perception security C-ITS | arXiv | 0 | 0 | ok |
| 70 | collective perception security C-ITS | DataCite-arXiv | 0 | 0 | ok |
| 70 | collective perception security C-ITS | Crossref | 40 | 0 | ok |
| 71 | cooperative perception spoofing LiDAR V2V | arXiv | 0 | 0 | ok |
| 71 | cooperative perception spoofing LiDAR V2V | DataCite-arXiv | 0 | 0 | ok |
| 71 | cooperative perception spoofing LiDAR V2V | Crossref | 40 | 4 | ok |
| 72 | trust-aware cooperative perception | arXiv | 6 | 1 | ok |
| 72 | trust-aware cooperative perception | DataCite-arXiv | 2 | 1 | ok |
| 72 | trust-aware cooperative perception | Crossref | 40 | 5 | ok |
| 73 | ghost vehicle attack V2X perception | arXiv | 0 | 0 | ok |
| 73 | ghost vehicle attack V2X perception | DataCite-arXiv | 0 | 0 | ok |
| 73 | ghost vehicle attack V2X perception | Crossref | 40 | 5 | ok |
| 74 | sybil attack traffic congestion VANET simulation | arXiv | 1 | 1 | ok |
| 74 | sybil attack traffic congestion VANET simulation | DataCite-arXiv | 0 | 0 | ok |
| 74 | sybil attack traffic congestion VANET simulation | Crossref | 40 | 14 | ok |
| 75 | fake BSM ghost vehicles traffic impact | arXiv | 0 | 0 | ok |
| 75 | fake BSM ghost vehicles traffic impact | DataCite-arXiv | 0 | 0 | ok |
| 75 | fake BSM ghost vehicles traffic impact | Crossref | 40 | 3 | ok |
| 76 | V2X attack impact assessment traffic simulation SUMO | arXiv | 1 | 0 | ok |
| 76 | V2X attack impact assessment traffic simulation SUMO | DataCite-arXiv | 0 | 0 | ok |
| 76 | V2X attack impact assessment traffic simulation SUMO | Crossref | 40 | 1 | ok |
| 77 | cyber attack connected automated vehicles traffic flow stability | arXiv | 0 | 0 | ok |
| 77 | cyber attack connected automated vehicles traffic flow stability | DataCite-arXiv | 0 | 0 | ok |
| 77 | cyber attack connected automated vehicles traffic flow stability | Crossref | 40 | 5 | ok |
| 78 | malicious vehicles mixed traffic safety | arXiv | 0 | 0 | ok |
| 78 | malicious vehicles mixed traffic safety | DataCite-arXiv | 0 | 0 | ok |
| 78 | malicious vehicles mixed traffic safety | Crossref | 40 | 2 | ok |
| 79 | jamming attack cooperative awareness safety application | arXiv | 0 | 0 | ok |
| 79 | jamming attack cooperative awareness safety application | DataCite-arXiv | 0 | 0 | ok |
| 79 | jamming attack cooperative awareness safety application | Crossref | 40 | 2 | ok |
| 80 | DoS attack VANET packet delivery safety | arXiv | 0 | 0 | ok |
| 80 | DoS attack VANET packet delivery safety | DataCite-arXiv | 0 | 0 | ok |
| 80 | DoS attack VANET packet delivery safety | Crossref | 40 | 18 | ok |
| 81 | misbehavior detection framework simulation F2MD | arXiv | 1 | 0 | ok |
| 81 | misbehavior detection framework simulation F2MD | DataCite-arXiv | 0 | 0 | ok |
| 81 | misbehavior detection framework simulation F2MD | Crossref | 40 | 13 | ok |
| 82 | VeReMi misbehavior dataset | arXiv | 9 | 0 | ok |
| 82 | VeReMi misbehavior dataset | DataCite-arXiv | 9 | 7 | ok |
| 82 | VeReMi misbehavior dataset | Crossref | 40 | 9 | ok |
| 83 | security overhead VANET signature verification delay | arXiv | 0 | 0 | ok |
| 83 | security overhead VANET signature verification delay | DataCite-arXiv | 0 | 0 | ok |
| 83 | security overhead VANET signature verification delay | Crossref | 40 | 9 | ok |
| 84 | ECDSA verification latency V2X channel congestion | arXiv | 0 | 0 | ok |
| 84 | ECDSA verification latency V2X channel congestion | DataCite-arXiv | 0 | 0 | ok |
| 84 | ECDSA verification latency V2X channel congestion | Crossref | 40 | 1 | ok |
| 85 | verify-on-demand V2X | arXiv | 1 | 1 | ok |
| 85 | verify-on-demand V2X | DataCite-arXiv | 0 | 0 | ok |
| 85 | verify-on-demand V2X | Crossref | 40 | 1 | ok |
| 86 | threat analysis risk assessment connected vehicle V2X | arXiv | 1 | 1 | ok |
| 86 | threat analysis risk assessment connected vehicle V2X | DataCite-arXiv | 0 | 0 | ok |
| 86 | threat analysis risk assessment connected vehicle V2X | Crossref | 40 | 5 | ok |
| 87 | automotive security risk assessment HEAVENS EVITA | arXiv | 0 | 0 | ok |
| 87 | automotive security risk assessment HEAVENS EVITA | DataCite-arXiv | 0 | 0 | ok |
| 87 | automotive security risk assessment HEAVENS EVITA | Crossref | 40 | 4 | ok |
| 88 | cybersecurity risk quantification connected vehicles | arXiv | 0 | 0 | ok |
| 88 | cybersecurity risk quantification connected vehicles | DataCite-arXiv | 0 | 0 | ok |
| 88 | cybersecurity risk quantification connected vehicles | Crossref | 40 | 14 | ok |
| 89 | pseudonym change VANET privacy | arXiv | 4 | 0 | ok |
| 89 | pseudonym change VANET privacy | DataCite-arXiv | 1 | 0 | ok |
| 89 | pseudonym change VANET privacy | Crossref | 40 | 7 | ok |
| 90 | pseudonym changing location privacy vehicles | arXiv | 2 | 0 | ok |
| 90 | pseudonym changing location privacy vehicles | DataCite-arXiv | 1 | 1 | ok |
| 90 | pseudonym changing location privacy vehicles | Crossref | 40 | 22 | ok |
| 91 | mix-zone pseudonym connected vehicles | arXiv | 0 | 0 | ok |
| 91 | mix-zone pseudonym connected vehicles | DataCite-arXiv | 0 | 0 | ok |
| 91 | mix-zone pseudonym connected vehicles | Crossref | 40 | 5 | ok |
| 92 | privacy vehicle-to-vehicle tracking adversary | arXiv | 5 | 1 | ok |
| 92 | privacy vehicle-to-vehicle tracking adversary | DataCite-arXiv | 0 | 0 | ok |
| 92 | privacy vehicle-to-vehicle tracking adversary | Crossref | 40 | 8 | ok |
| 93 | trajectory linking pseudonymous vehicles | arXiv | 0 | 0 | ok |
| 93 | trajectory linking pseudonymous vehicles | DataCite-arXiv | 0 | 0 | ok |
| 93 | trajectory linking pseudonymous vehicles | Crossref | 40 | 1 | ok |
| 94 | de-anonymization vehicular messages | arXiv | 1 | 0 | ok |
| 94 | de-anonymization vehicular messages | DataCite-arXiv | 1 | 1 | ok |
| 94 | de-anonymization vehicular messages | Crossref | 40 | 8 | ok |
| 95 | unlinkability pseudonyms C-ITS | arXiv | 1 | 0 | ok |
| 95 | unlinkability pseudonyms C-ITS | DataCite-arXiv | 1 | 1 | ok |
| 95 | unlinkability pseudonyms C-ITS | Crossref | 40 | 2 | ok |
| 96 | anonymity set entropy vehicular privacy | arXiv | 0 | 0 | ok |
| 96 | anonymity set entropy vehicular privacy | DataCite-arXiv | 0 | 0 | ok |
| 96 | anonymity set entropy vehicular privacy | Crossref | 40 | 2 | ok |
| 97 | pseudonym change synchronization | arXiv | 1 | 0 | ok |
| 97 | pseudonym change synchronization | DataCite-arXiv | 1 | 1 | ok |
| 97 | pseudonym change synchronization | Crossref | 40 | 0 | ok |
| 98 | eavesdropping attack vehicle tracking roadside | arXiv | 0 | 0 | ok |
| 98 | eavesdropping attack vehicle tracking roadside | DataCite-arXiv | 0 | 0 | ok |
| 98 | eavesdropping attack vehicle tracking roadside | Crossref | 40 | 2 | ok |
| 99 | privacy cooperative awareness messages tracking | arXiv | 1 | 0 | ok |
| 99 | privacy cooperative awareness messages tracking | DataCite-arXiv | 0 | 0 | ok |
| 99 | privacy cooperative awareness messages tracking | Crossref | 40 | 1 | ok |
| 100 | radio fingerprinting vehicular authentication | arXiv | 1 | 0 | ok |
| 100 | radio fingerprinting vehicular authentication | DataCite-arXiv | 1 | 1 | ok |
| 100 | radio fingerprinting vehicular authentication | Crossref | 40 | 1 | ok |
| 101 | physical layer identification vehicle V2X | arXiv | 0 | 0 | ok |
| 101 | physical layer identification vehicle V2X | DataCite-arXiv | 0 | 0 | ok |
| 101 | physical layer identification vehicle V2X | Crossref | 40 | 1 | ok |
| 102 | LTE-V2X security | arXiv | 8 | 0 | ok |
| 102 | LTE-V2X security | DataCite-arXiv | 8 | 6 | ok |
| 102 | LTE-V2X security | Crossref | 40 | 1 | ok |
| 103 | cellular V2X jamming | arXiv | 0 | 0 | ok |
| 103 | cellular V2X jamming | DataCite-arXiv | 0 | 0 | ok |
| 103 | cellular V2X jamming | Crossref | 40 | 9 | ok |
| 104 | sidelink resource allocation attack | arXiv | 1 | 0 | ok |
| 104 | sidelink resource allocation attack | DataCite-arXiv | 1 | 0 | ok |
| 104 | sidelink resource allocation attack | Crossref | 40 | 0 | ok |
| 105 | PC5 sidelink authentication | arXiv | 0 | 0 | ok |
| 105 | PC5 sidelink authentication | DataCite-arXiv | 1 | 1 | ok |
| 105 | PC5 sidelink authentication | Crossref | 40 | 0 | ok |
| 106 | 5G NR V2X attacks | arXiv | 2 | 0 | ok |
| 106 | 5G NR V2X attacks | DataCite-arXiv | 1 | 0 | ok |
| 106 | 5G NR V2X attacks | Crossref | 40 | 1 | ok |
| 107 | C-V2X mode 4 attack | arXiv | 1 | 0 | ok |
| 107 | C-V2X mode 4 attack | DataCite-arXiv | 1 | 0 | ok |
| 107 | C-V2X mode 4 attack | Crossref | 40 | 0 | ok |
| 108 | V2X misbehavior SPS resource reservation | arXiv | 0 | 0 | ok |
| 108 | V2X misbehavior SPS resource reservation | DataCite-arXiv | 0 | 0 | ok |
| 108 | V2X misbehavior SPS resource reservation | Crossref | 40 | 0 | ok |
| 109 | transition DSRC C-V2X | arXiv | 2 | 0 | ok |
| 109 | transition DSRC C-V2X | DataCite-arXiv | 1 | 0 | ok |
| 109 | transition DSRC C-V2X | Crossref | 40 | 0 | ok |
| 110 | platoon attack | arXiv | 53 | 11 | ok |
| 110 | platoon attack | DataCite-arXiv | 17 | 6 | ok |
| 111 | platooning security | arXiv | 48 | 7 | ok |
| 111 | platooning security | DataCite-arXiv | 20 | 11 | ok |
| 112 | CACC attack | arXiv | 20 | 0 | ok |
| 112 | CACC attack | DataCite-arXiv | 15 | 3 | ok |
| 113 | cooperative perception attack | arXiv | 14 | 5 | ok |
| 113 | cooperative perception attack | DataCite-arXiv | 4 | 2 | ok |
| 114 | collaborative perception adversarial | arXiv | 35 | 2 | ok |
| 114 | collaborative perception adversarial | DataCite-arXiv | 23 | 11 | ok |
| 115 | V2X perception spoofing | arXiv | 3 | 0 | ok |
| 115 | V2X perception spoofing | DataCite-arXiv | 2 | 1 | ok |
| 116 | pseudonym change | arXiv | 28 | 2 | ok |
| 116 | pseudonym change | DataCite-arXiv | 6 | 0 | ok |
| 117 | mix zone vehicles | arXiv | 40 | 2 | ok |
| 117 | mix zone vehicles | DataCite-arXiv | 3 | 1 | ok |
| 118 | vehicular location privacy | arXiv | 38 | 3 | ok |
| 118 | vehicular location privacy | DataCite-arXiv | 34 | 29 | ok |
| 119 | V2X tracking privacy | arXiv | 0 | 0 | ok |
| 119 | V2X tracking privacy | DataCite-arXiv | 0 | 0 | ok |
| 120 | V2X jamming | arXiv | 7 | 2 | ok |
| 120 | V2X jamming | DataCite-arXiv | 3 | 2 | ok |
| 121 | sidelink attack | arXiv | 3 | 0 | ok |
| 121 | sidelink attack | DataCite-arXiv | 1 | 0 | ok |
| 122 | C-V2X security | arXiv | 19 | 0 | ok |
| 122 | C-V2X security | DataCite-arXiv | 18 | 4 | ok |
| 123 | traffic signal attack | arXiv | 60 | 9 | ok |
| 123 | traffic signal attack | DataCite-arXiv | 46 | 11 | ok |
| 124 | V2X attack safety | arXiv | 37 | 6 | ok |
| 124 | V2X attack safety | DataCite-arXiv | 20 | 9 | ok |
| 125 | connected vehicles cyberattack traffic | arXiv | 17 | 6 | ok |
| 125 | connected vehicles cyberattack traffic | DataCite-arXiv | 3 | 0 | ok |
| 126 | V2X security latency | arXiv | 30 | 1 | ok |
| 126 | V2X security latency | DataCite-arXiv | 27 | 16 | ok |
| 127 | V2X threat analysis | arXiv | 9 | 1 | ok |
| 127 | V2X threat analysis | DataCite-arXiv | 4 | 3 | ok |

## Totals

- Distinct queries: 127 (set A 57 on all sources; set B 52 on arXiv, DataCite, Crossref; set C 18 short arXiv-style queries on arXiv and DataCite).
- Records written: 2422 (high 848, medium 1402, low 172).
- With abstract: 1574; with OA PDF link: 477; with arXiv id: 286.
