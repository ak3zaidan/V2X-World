#!/usr/bin/env python3
"""Replays the 1 s BSM certificate cadence on a run's records (live-scms PT30.full-cert).

    python3 -I tools/realworld/cert_cadence_replay.py <run-dir> [<run-dir> ...]

A certificate is attached when none was attached for 1 s, and on the first frame after a
pseudonym change (`sec.pseudonym`). The replay runs on each vehicle's recorded BSM
instants (`node.tx` `t_generated`) with and without the changes, prints how many recorded
signer choices it reproduces, and how many certificates the changes add. It reads the
run's recording only and shares no code with the engine.
"""
import collections
import json
import sys

from mcap.reader import make_reader

SECOND = 1_000_000_000
SLACK = 1_000_000  # 1 ms: the cadence is evaluated on 100 ms ticks


def replay(frames, changes, use_changes):
    last = None
    k = 0
    out = []
    for t, _ in frames:
        changed = False
        while use_changes and k < len(changes) and changes[k][0] <= t:
            changed = True
            k += 1
        cert = changed or last is None or t - last >= SECOND - SLACK
        if cert:
            last = t
        out.append(cert)
    return out


def main(runs):
    for run in runs:
        tx = collections.defaultdict(list)
        ch = collections.defaultdict(list)
        with open(run + '/recording.mcap', 'rb') as f:
            topics = ['record/node.tx', 'record/sec.pseudonym']
            for _, c, m in make_reader(f).iter_messages(topics=topics):
                d = json.loads(m.data)
                if c.topic == 'record/node.tx':
                    if d.get('msg_type') == 'bsm':
                        tx[d['node']].append((d['t_generated'], d['signer'] == 'certificate'))
                else:
                    ch[d['node']].append((d['t'], d['reason']))
        match = mism = with_ch = no_ch = 0
        for n, fr in tx.items():
            fr.sort()
            cs = sorted(ch.get(n, []))
            a = replay(fr, cs, True)
            b = replay(fr, cs, False)
            for (_, rec), x in zip(fr, a):
                if rec == x:
                    match += 1
                else:
                    mism += 1
            with_ch += sum(a)
            no_ch += sum(b)
        print(run, 'replay matches', match, 'mismatches', mism, '| certificates with changes',
              with_ch, 'without', no_ch, 'added by the changes', with_ch - no_ch)


if __name__ == '__main__':
    main(sys.argv[1:])
