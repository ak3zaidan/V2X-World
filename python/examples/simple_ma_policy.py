"""A minimal custom Misbehaviour Authority (MA) policy.

Rule: revoke a pseudonym certificate once 3 different vehicles have reported it
within the last 30 seconds.

Run (after `cd python && maturin develop --release`), from anywhere:
    python python/examples/simple_ma_policy.py

It writes its recording to runs/simple-ma/run.mcap under the repository root and needs
only pyarrow besides v2xw. It runs the scenario's full 300 s, because the first reports
reach the MA after about two simulated minutes; on a laptop that takes tens of minutes.
"""

from pathlib import Path

import v2xw

REPO = Path(__file__).resolve().parents[2]

K_REPORTERS = 3          # distinct reporters needed to revoke
WINDOW_NS = 30 * 10**9   # only count reports from the last 30 s


class WindowedK:
    # Every plug-in carries a model card: what it is and what it assumes.
    card = {
        "id": "threat/ma/simple-windowed-k",
        "family": "ma-pipeline",
        "version": "0.1.0",
        "api_version": "1.0.0",
        "tier": ["medium"],
        "purpose": "Revoke a pseudonym once K distinct reporters accused it within a time window.",
        "equations": [],
        "parameters": [],
        "determinism": {"uses_rng": False},
    }

    def __init__(self):
        # (ma, subject) -> {reporter: last time it reported this subject}
        self.reports = {}
        self.revoked = set()

    def decide(self, ctx, batch):
        """Called each step with the reports the MA received. Returns the decisions."""
        decisions = []
        for i, row in enumerate(batch.rows()):
            key = (ctx.ma, row["subject_cert"])
            if key in self.revoked:
                continue

            # Remember who reported this pseudonym and when.
            seen = self.reports.setdefault(key, {})
            seen[row["reporter_cert"]] = row["observed_ns"]

            # Forget reports older than the window.
            for reporter, t in list(seen.items()):
                if ctx.t_ns - t > WINDOW_NS:
                    del seen[reporter]

            if len(seen) >= K_REPORTERS:
                self.revoked.add(key)
                decisions.append((row["subject_cert"], True, i))  # (subject, revoke, evidence row)
        return decisions


def main():
    scenario = v2xw.Scenario.load(str(REPO / "scenarios" / "credential-lifecycle.yaml"))

    # A Python policy replaces the scenario's own MA rule, so clear that rule first.
    doc = scenario.as_dict()
    doc["detection"]["ma"] = None
    doc["experiment"] = None
    doc["time"]["duration_s"] = 300.0  # the scenario's own: reports start after the first minutes
    scenario = v2xw.Scenario.from_dict(doc)

    policy = WindowedK()
    out = REPO / "runs" / "simple-ma"
    out.mkdir(parents=True, exist_ok=True)  # the recorder does not create directories
    run = v2xw.run(scenario, recording=str(out / "run.mcap"), ma_policy=policy)

    print(f"revoked {len(policy.revoked)} pseudonyms")
    recording = v2xw.Recording.open(run.recording)
    if "record/ma.decision" not in [c["topic"] for c in recording.channels]:
        print("the MA recorded no decision")
        return
    decisions = recording.arrow("ma.decision", ground_truth=False)
    print(f"{decisions.num_rows} MA decisions; the first five:")
    for row in decisions.slice(0, 5).to_pylist():
        print("  ", row)


if __name__ == "__main__":
    main()
