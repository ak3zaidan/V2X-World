# Catalog statistics

Built from the six cluster searches in `search/` (13666 input records), merged by DOI, then arXiv id, then normalised title. Preprint and published versions are one entry (arXiv DOIs 10.48550 are treated as arXiv ids; other DOIs kept in `other_dois`). Each entry keeps its highest relevance and every cluster that found it.

- Entries: **11315** (high 3430, medium 5169, low 2716)
- Found by more than one cluster: 1947
- With an abstract: 8695; with a DOI: 9844; with an arXiv id: 1788
- Open-access PDF link known: 5135; PDFs downloaded to `papers/`: **1956** (high 714, medium 1242, low 0)
- Download outcomes: ok 1956, not_pdf 698, http_403 453, err_URLError 102, http_404 68, too_big 11, http_400 10, err_InvalidURL 7, http_405 6, http_410 5, http_429 4, http_500 3, err_ConnectionResetError 2, http_503 2, http_522 2, err_TimeoutError 2, http_401 2, http_502 1, http_302 1, http_511 1 (`not_pdf` = link led to a landing page or paywall, skipped)
- Summary batches: 30 files in `batches/`, 378 ids each (last may be shorter), high relevance first

## By cluster

A paper counts once for each cluster that found it.

| Cluster | All | High |
|---|---:|---:|
| mbd | 3589 | 1171 |
| safety-privacy | 2419 | 1189 |
| crypto | 2186 | 731 |
| pki-scms | 2013 | 870 |
| ai | 1910 | 279 |
| sim | 1527 | 664 |

## By relevance

| By relevance | Papers |
|---|---:|
| high | 3430 |
| medium | 5169 |
| low | 2716 |

## By venue type

| By venue type | Papers |
|---|---:|
| journal | 6348 |
| conference | 2830 |
| preprint | 1369 |
| book | 421 |
| thesis | 138 |
| workshop | 117 |
| report | 44 |
| (unknown) | 23 |
| standard | 22 |
| peer-review | 1 |
| other | 1 |
| component | 1 |

## By year

| By year | Papers |
|---|---:|
| 2027 | 12 |
| 2026 | 1511 |
| 2025 | 1904 |
| 2024 | 1409 |
| 2023 | 1097 |
| 2022 | 965 |
| 2021 | 730 |
| 2020 | 621 |
| 2019 | 574 |
| 2018 | 483 |
| 2017 | 317 |
| 2016 | 270 |
| 2015 | 217 |
| 2014 | 202 |
| 2013 | 151 |
| 2012 | 163 |
| 2011 | 135 |
| 2010 | 114 |
| 2009 | 78 |
| 2008 | 64 |
| 2007 | 42 |
| 2006 | 34 |
| 2005 | 21 |
| 2004 | 28 |
| 2003 | 5 |
| 2002 | 8 |
| 2001 | 8 |
| 2000 | 3 |
| 1999 | 5 |
| 1998 | 7 |
| 1997 | 1 |
| 1996 | 1 |
| 1994 | 1 |
| 1992 | 1 |
| 1991 | 2 |
| 1986 | 3 |
| 1985 | 1 |
| 1971 | 1 |
| 1968 | 1 |
| 1961 | 1 |
| 1926 | 1 |
| unknown | 123 |


## Download note

PDF downloading stopped early when free disk space fell below the 8 GB guard (other work on the machine was also using disk). All 1334 high-relevance entries with a PDF link were tried; 714 of them downloaded. 1799 entries with a PDF link (medium 461, low 1338) were not tried. To resume, run the downloader again; it skips entries already tried.
