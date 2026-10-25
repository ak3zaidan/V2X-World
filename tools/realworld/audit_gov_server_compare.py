"""GOV.SRV.trust: v2xw-server inspect.trust answers against the server's own pki.state records.
The byte stores (`certificates`, each CRL's `hex`) are left out: the server serves them through
inspect.cert / inspect.crl instead (crates/v2xw-server/src/pki.rs)."""
import copy, json, sys
from mcap.reader import make_reader
rec, ans = sys.argv[1], sys.argv[2]
states = []
with open(rec, 'rb') as f:
    for _, ch, m in make_reader(f).iter_messages():
        if ch.topic == 'record/pki.state':
            states.append(json.loads(m.data))
A = json.load(open(ans))


def norm(s):
    s = dict(s)
    s.pop('certificates', None)
    s['crls'] = [{k: v for k, v in c.items() if k != 'hex'} for c in s.get('crls') or []]
    return s


def judge(A):
    bad = []
    for t, a in A.items():
        st = a['result']['state']
        want = max((s for s in states if s['t'] <= int(t)), key=lambda s: s['t'])
        if norm(st) != norm(want):
            bad.append(t)
    return bad


print('clean: instants differing', judge(A), 'of', len(A), '; pki.state records', len(states))
M = copy.deepcopy(A)
c = M['121000000000']['result']['state']['providers'][1]['components'][1]['counters']
c['certs_issued'] = (c.get('certs_issued') or 0) + 1
print('fault (provider-b root certs_issued + 1 in the 121 s answer): differing', judge(M))
print('fields compared', sorted(norm(A['0']['result']['state']).keys()))
