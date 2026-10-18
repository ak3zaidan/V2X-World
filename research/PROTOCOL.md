# Literature review protocol

Purpose: find every published work (arXiv, journals, conferences, workshops, theses, standards
reports) that overlaps with this V2X engine and its candidate research directions, summarise each
one, and use the result to decide which research questions are genuinely open. Nothing may be
claimed as novel unless this review shows it is.

## What the engine does (the reference for "overlap")

A single-process, deterministic V2X simulator: city-scale microscopic traffic (IDM/MOBIL, signals,
pedestrians, cyclists; cross-validated against SUMO; 50+ US city maps); DSRC 802.11p, LTE-V2X and
NR-V2X radio stacks (Veins/OpenCV2X-matched PHY/MAC); ETSI GeoNetworking; IEEE 1609.2 security; a
complete US SCMS backend (RA, PCA, LA1/LA2, MA, ECA, DCM, CRL; butterfly keys; linkage-based
revocation; pseudonym provisioning and top-ups over cellular TCP/TLS; RSU relay and CRL
broadcast), validated against official test vectors and the real SCMS Manager trust lists;
post-quantum and hybrid signatures; threshold authorities (FROST, CGGMP21, Umbrella);
misbehaviour detection and attacks executed inside the simulation (closed loop); per-message joint
ground truth across traffic, radio and PKI; exact counterfactual reruns (same seed, one change);
about 300 configurable settings; an LLM agent that configures and analyses runs.

Candidate directions under evaluation (see `synthesis/`):
1. Closed-loop misbehaviour → revocation → safety ("time to neutralise", MA decision design).
2. The safety cost of security (PQ migration of the SCMS, verification policies, detection thresholds).
3. Counterfactual V2X: a benchmark with per-unit counterfactual ground truth and a learned world model / surrogate.
Runners-up: cross-layer linkability; multi-provider SCMS interoperability; NR sidelink attacks with
verification load; LLM agents for cross-layer root-cause analysis; quantified ISO/SAE 21434 ratings.

## Sources

Query each systematically and log every query with its hit count:
- arXiv API `http://export.arxiv.org/api/query`
- OpenAlex `https://api.openalex.org/works?search=...` (journals, conferences, theses; gives open-access links)
- Semantic Scholar `https://api.semanticscholar.org/graph/v1/paper/search` and its references/citations endpoints (snowballing)
- DBLP `https://dblp.org/search/publ/api`
- Crossref `https://api.crossref.org/works`
- IACR ePrint (cryptography) via OpenAlex/web search
- Web search for standards bodies' reports (5GAA, ETSI, SAE, CAMP, USDOT) and theses

Snowballing: for every high-relevance paper, pull its references and the papers citing it (Semantic
Scholar), and add the relevant ones.

**Privacy rule:** never put the repository owner's name or email address in any request (no
`mailto=` parameter, no email in User-Agent). Use `User-Agent: v2x-lit-review/1.0`. Respect rate
limits (sleep and retry on HTTP 429).

## Inclusion

Include anything about: V2X / VANET / C-ITS security, PKI, SCMS, CCMS, pseudonyms, privacy,
revocation, misbehaviour detection and reporting; V2X attacks and their traffic or safety impact;
post-quantum, hybrid or threshold cryptography for vehicles; V2X simulators, co-simulation,
digital twins, datasets and benchmarks; learned surrogates, world models or foundation models for
vehicular or wireless networks; counterfactual/causal evaluation with network simulators; LLM
agents for network simulation or security operations. All years, all venues, all languages with an
English abstract. Include adjacent general work (non-vehicular VPKI, network world models) at
lower relevance.

## Record format (`search/<cluster>.jsonl`, one JSON object per line)

```json
{"title": "", "authors": ["First Last"], "year": 2024, "venue": "", "venue_type": "journal|conference|workshop|preprint|thesis|report|standard|book",
 "doi": "", "arxiv_id": "", "url": "", "oa_pdf_url": "", "abstract": "", "citations": 0,
 "cluster": "", "relevance": "high|medium|low", "relevance_reason": "one line", "found_via": "query string or 'snowball:<seed title>'"}
```

Relevance: **high** = overlaps the engine or a candidate direction directly; **medium** = same
area, different focus; **low** = adjacent background.

Every cluster also writes `search/<cluster>-queries.md`: each query, source, date, hit count, and
how many were kept.

## Catalog (`catalog.jsonl`, `catalog.csv`)

Merged and deduplicated by DOI, then arXiv id, then normalised title (lowercase, punctuation
removed). Preprint and published versions of one paper are one entry (keep both links). Each entry
gets an `id`: first author's surname + year + first significant title word, lowercase
(`brecht2018security`); add a letter on collision. Open-access PDFs go to `papers/<id>.pdf` (not
committed).

## Summaries (`summaries/<id>.md`)

```markdown
# <Title>
<Authors> · <Venue> <Year> · <DOI or arXiv link> · relevance: high|medium|low · basis: full text|abstract

**Problem:** one or two sentences.
**Approach:** what they built or proposed.
**Tools and scale:** simulator(s), radio tech, traffic model, PKI/security model, scale (vehicles, area, duration), real data if any.
**Results:** the key quantitative findings.
**Limitations and future work (as stated by the authors):** bullet list. Quote short phrases where useful.
**Overlap with our engine:** none | partial | strong — what exactly.
**Novelty implications:** what we can no longer claim because of this paper; what it leaves open.
```

Use the full text for high-relevance papers with an open-access PDF; otherwise the abstract (say so
in `basis`). Keep each summary under about 350 words.

## Synthesis (`synthesis/`)

- `landscape.md`: taxonomy, counts by topic and year, key groups and venues.
- `open-questions.md`: every stated limitation and future-work item, clustered, with sources.
- `novelty-check.md`: for each candidate direction, the closest papers, what is already done, what remains, and a verdict.
- `claims-we-cannot-make.md`: specific claims ruled out by prior work, each with the paper that rules it out.
