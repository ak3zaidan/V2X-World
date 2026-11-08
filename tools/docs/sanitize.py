"""Strip pointers to working notes that are not part of the published tree.

The engine's setting descriptions, model cards and metric definitions sometimes cite
internal design notes ("04-models.md §2.5", "docs/design/...") next to the primary
source. The generated reference pages keep the primary citation and the text, and drop
those pointers. Usable as a module (``sanitize(text)``) or as a script over Markdown
files (``python3 tools/docs/sanitize.py <file.md>...``), which rewrites them in place.
"""

import re
import sys

_NOTES = (
    "00-design-brief|01-inventory|02-architecture|03-interfaces|04-models|05-protocols|"
    "06-node-models|07-threats-and-detection|08-measurement-and-data|09-ui|10-roadmap|"
    "11-open-questions|12-build-decisions|13-product-direction|14-studio-shell|"
    # The SCMS realism notes, then the short forms people wrote for the long names (the
    # long forms come first so that an alternation never stops at the short prefix).
    "00-gap-analysis-and-plan|01-deployment-and-governance|02-cryptography-and-formats|"
    "03-protocols-and-operations|04-validation-report|05-scale-and-regression|"
    "06-conformance-run|06-validation-summary|07-config-testing-report|"
    "08-settings-ux-and-gaps|09-completion-plan|10-completion-specs|11-j2945-1-conformance|"
    "00-gap-analysis|07-threats|08-measurement|"
    # Audit, benchmark and oracle notes; a scenario file such as `scms-audit-mbpipe.yaml`
    # is published and is not matched (the lookarounds below).
    r"audit-(?:gov|prov|mbpipe|sim-metrics|db1-3)|final-run|final-bench|full-bench|honest-density|"
    r"oracle-final|(?:rerun|oracle|platform|radio|traffic)-\d{4}-\d{2}-\d{2}(?:-[a-z])?|"
    r"rule-gap-specs-\d{4}-\d{2}-\d{2}|(?:ui-review|world-review|msg-sec|physical-layer|record-metrics)-register|vehicle-and-roadside|"
    r"radio-fidelity"
)
# A private finding id ("GOV.L.onecert", "AU.DIST.rsu", "PV.aca-cannot-group").
_FINDING = (
    r"\b(?:AU|CV|GOV|PV|OF|FO|QD|FB|NET|SC|EX|MI|MWP|LCV|LN|LT|LV|OA|RR|CW|BQ|FS)\d*"
    r"\.[A-Za-z0-9][A-Za-z0-9._*-]*[A-Za-z0-9*]"
)
_REF = (
    r"(?:docs/design/[\w./-]*[\w/]|(?:docs/design/)?(?<![\w-])(?:" + _NOTES + r")(?![\w-])"
    r"(?:\.md)?)"
    r"(?:\s*§\s*[\dA-Z](?:[\d.]*\d)?(?:\s+item\s+\d+)?|\s+\d+(?:\.\d+)+)?"
    r"(?:[:,]?\s+" + _FINDING + r")?"
    # A note's own item labels ("Q3, Q14", "O-1", "S3").
    r"(?:\s+[A-Z]{1,2}-?\d+(?:,\s*[A-Z]{1,2}-?\d+)*\b)?"
)
_PATTERNS = [
    # A sentence that names a work package is a plan, not a description.
    (re.compile(r"(?<=[.;])\s*[^.;|]*\bWP-\d+[a-z]?\b[^.;|]*[.;]\s*([a-z])?"),
     lambda m: " " + (m.group(1).upper() if m.group(1) else "")),
    # Internal research-note indices ("R11 §D1", "[R3 §G.7]", "(R1 §D.3-D.4)"); when one
    # prefixes the real source ("[R5 §B.5: CAMP EE Requirements ...]"), keep the source.
    (re.compile(r"\bR\d{1,2} §[A-Z]?(?:[\dA-Z.\u2013-]*[\dA-Z])?:\s*"), ""),
    (re.compile(r"\s*[\[(]R\d{1,2} §[A-Z]?(?:[\dA-Z.\u2013-]*[\dA-Z])?[\])]"), ""),
    (re.compile(r",?\s*\bR\d{1,2} §[A-Z]?(?:[\dA-Z.\u2013-]*[\dA-Z])?"), ""),
    # A work-package tag left inside a clause.
    (re.compile(r",?\s*\bWP-\d+[a-z]?\b\s*"), " "),
    # The pointer itself, with the connective that introduced it.
    (re.compile(r"(?:,\s*|;\s*|\s+via\s+|\s+in\s+|\s+of\s+|\s+see\s+|\s*\bbenchmark\s+)?" + _REF), ""),
    # A finding id cited alone, with the connective that introduced it.
    (re.compile(r"(?:,\s*|;\s*)?" + _FINDING), ""),
    # Tidy what the removal left behind.
    (re.compile(r"\(\s*[;,:]?\s*\)"), ""),
    (re.compile(r"\(\s*[;,]\s*"), "("),
    (re.compile(r"\s*[;,]\s*\)"), ")"),
    (re.compile(r"[ \t]+([.,;])(?=\s|$)"), r"\1"),
    (re.compile(r",([.;:])"), r"\1"),
    (re.compile(r"[ \t]{2,}"), " "),
]


def sanitize(text: str) -> str:
    """Return ``text`` without pointers to unpublished working notes."""
    for pattern, repl in _PATTERNS:
        text = pattern.sub(repl, text)
    return text


if __name__ == "__main__":
    for path in sys.argv[1:]:
        with open(path, encoding="utf-8") as f:
            before = f.read()
        after = sanitize(before)
        if after != before:
            with open(path, "w", encoding="utf-8") as f:
                f.write(after)
            print(f"sanitized {path}")
