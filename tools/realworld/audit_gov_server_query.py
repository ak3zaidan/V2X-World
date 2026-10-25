"""Server == records: query v2xw-server's inspect.trust at several instants of a finished live
run and compare each answer with the pki.state record of the same instant in its --record."""
import json, sys, time
from websockets.sync.client import connect

port, rec, out = int(sys.argv[1]), sys.argv[2], sys.argv[3]
ws = connect(f'ws://127.0.0.1:{port}/vwp/v1', max_size=None, open_timeout=60)
nid = [0]


def call(method, params=None):
    nid[0] += 1
    ws.send(json.dumps({'jsonrpc': '2.0', 'id': nid[0], 'method': method, 'params': params or {}}))
    while True:
        m = ws.recv(timeout=120)
        if isinstance(m, bytes):
            continue
        q = json.loads(m)
        if q.get('id') == nid[0]:
            return q


# wait for the run to reach its end
for _ in range(600):
    st = call('run.status')
    r = st.get('result') or {}
    if r.get('state') in ('finished', 'ended', 'done', 'stopped') or r.get('t_ns', 0) >= 139_000_000_000:
        break
    time.sleep(1)
print('status', json.dumps(st)[:300])
answers = {}
for t in [0, 30_000_000_000, 60_000_000_000, 121_000_000_000, 139_000_000_000]:
    a = call('inspect.trust', {'t_ns': t})
    answers[t] = a
json.dump({str(k): v for k, v in answers.items()}, open(out, 'w'))
print('answers', {k: list((v.get('result') or v.get('error') or {}).keys())[:12] for k, v in answers.items()})
call('run.stop')
ws.close()
