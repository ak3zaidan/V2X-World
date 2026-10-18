# Literature review for the V2X engine

This folder is a systematic literature review of everything published that overlaps with this V2X engine and its candidate research directions. Its purpose is to decide which research questions are still open, so that nothing is claimed as novel when it already exists.

The review was built on 2026-10-17 by following [`PROTOCOL.md`](PROTOCOL.md). That file sets the sources, the privacy rule, the inclusion criteria, and the record and summary formats.

## Totals

- **11,315 unique works** after merging 13,666 search records by DOI, then arXiv id, then normalised title. Preprint and published versions count as one entry.
  - By relevance: **3,430 high**, 5,169 medium, 2,716 low.
  - By venue type: 6,348 journal, 2,830 conference, 1,369 preprint, 421 book, 138 thesis, 117 workshop, 44 report, 22 standard.
  - By year: 1926 to 2027. 6,898 are from 2022 or later; 1,523 from 2026 and later.
- **11,315 summaries**, one per work. 693 were written from the full text, 8,271 from the abstract and 2,351 from the title and metadata only. Each summary states its basis.
  - 192 summaries are rated *strong* overlap, 5,149 *partial* and 5,974 *none*.
- **1,956 open-access PDFs** downloaded to `papers/`. These are not committed; see `.gitignore`. Of these, 714 are high-relevance papers.
- **2,828 stated limitations and future-work items** from 2,051 papers, clustered into 30 open problems.

## Contents

| Path | What it is |
|---|---|
| `PROTOCOL.md` | Review protocol: what the engine does, the candidate directions, sources, inclusion criteria, formats |
| `search/<cluster>.jsonl` | Raw records per search cluster: `pki-scms`, `mbd`, `safety-privacy`, `crypto`, `sim`, `ai` |
| `search/<cluster>-queries.md` | **Query logs**: every query with source, date, hit count and number kept, plus source availability notes |
| `catalog.jsonl`, `catalog.csv` | Merged, deduplicated catalog; one entry per work with `id`, links, abstract, clusters, relevance |
| `catalog-stats.md` | Counts by cluster, relevance, venue type and year; PDF download outcomes |
| `summaries/<id>.md` | One summary per work: problem, approach, tools and scale, results, stated limitations, overlap with the engine, novelty implications |
| `batches/batch-NN.txt`, `batch-NN-notes.md` | The 30 summary batches (high relevance first) and per-batch notes: limitations list and strong-overlap papers |
| `papers/` | Downloaded open-access PDFs (local only) |
| `synthesis/landscape.md` | Taxonomy, counts by topic and year, venues, research groups, closest prior works to the engine |
| `synthesis/open-questions.md` | Every stated limitation and future-work item, clustered into open problems, with source ids and paper counts |
| `synthesis/novelty-check.md` | For each candidate direction and runner-up: closest papers, what is done, what is open, verdict; new directions; final ranking |
| `synthesis/claims-we-cannot-make.md` | Specific "first" claims ruled out, each with the papers that rule it out and the narrower claim that survives |

## How it was built

1. **Search.** Six topic clusters were each searched systematically. Each queries file records the exact queries.
   - Sources: arXiv API, Crossref, Semantic Scholar, OpenAlex, DBLP, IACR ePrint (via the other indexes) and web search for standards and agency reports.
   - Snowballing: references and citations of high-relevance papers were pulled via OpenCitations and Semantic Scholar.
   - Privacy: no request carried the repository owner's name or email.
2. **Merge.** The cluster records were merged into `catalog.jsonl` and assigned ids of the form `surname + year + first title word`.
3. **Download.** Open-access PDFs were fetched until the disk-space guard stopped the downloader. Every high-relevance paper with a PDF link was tried.
4. **Summarise.** Summaries were written in 30 batches, from the full text where a high-relevance PDF was available and from the abstract otherwise.
5. **Synthesise.** The synthesis read all 192 strong-overlap summaries in full and the strong-overlap sections of all 30 batch notes. It ran targeted catalog searches for each direction (listed in `novelty-check.md`) and read the condensed summaries of the 135 most relevant partial-overlap papers. Topic counts and limitation clusters were computed by keyword rules over titles, abstracts and summary fields.

## How to read it

- **Start with `synthesis/novelty-check.md`.** Its ranking table is the main answer to the question "what is still open". The top three are:
  1. A counterfactual V2X benchmark with exact per-unit cross-layer ground truth, used for interventional detector evaluation and for validating causal estimators.
  2. MA decision design under framing ("weaponised revocation") attacks, together with stage-decomposed time to neutralise in the US SCMS.
  3. Robustness of V2X security conclusions to modelling choices.
- **Then `synthesis/claims-we-cannot-make.md`.** Check it before writing any sentence that begins "the first ...".
- **Use `synthesis/open-questions.md`** to cite how many papers raise a problem, and which ones.
- **Look up any id** in `summaries/<id>.md` for details, then in `catalog.jsonl` for DOI and links.
- **Trust levels.** Most summaries rest on abstracts. Any paper marked "(t)" (title only) or "(a)" (abstract only) in the synthesis must be read in full before a thesis claim depends on it. `novelty-check.md` ends with the must-read list.

## Known gaps

- **OpenAlex** (free daily budget exhausted) and **DBLP** (anti-bot challenge) were unavailable for most clusters. **Semantic Scholar** was heavily rate-limited. Crossref, arXiv and OpenCitations carried most of the search, so conference papers indexed mainly in DBLP or IEEE Xplore may be under-represented. Re-running the OpenAlex and DBLP queries in the logs is the most useful extension.
- **Paywalled full texts** (most IEEE, Springer and SAE papers) were not read. 1,799 lower-relevance PDF links were never tried, because of disk space.
- **Topic and limitation counts** come from keyword rules. Treat them as estimates of attention, not exact figures.
