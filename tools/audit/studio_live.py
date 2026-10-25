#!/usr/bin/env python3
"""Live Studio checker (docs/validation/live-studio-2026-10-25.md).

Judges the rows of crates/v2xw-engine/tests/expectations/live-studio.yaml against the evidence
`ui/apps/studio/e2e-engine/live-studio.spec.ts` writes (evidence.json, A.mcap, B.mcap per case
directory). It shares no code with the engine or the Studio: the display formatter, the run
pooling and every statistic are re-derived here from the recordings' own `metric.sample`
records, so a number the page shows is held against the run, not against the code that drew it.

  studio_live.py check <yaml> <cases-dir>       print one verdict line per row
  studio_live.py selftest <yaml> <cases-dir>    apply each row's fault; every row must go red
"""
import copy
import json
import math
import os
import re
import sys

NS = 10**9


# ------------------------------------------------------------------------------ the records

def mcap_samples(path):
    """[(metric, t_ns, dims, value_dict, unit)] for every metric.sample record."""
    from mcap.reader import make_reader
    out = []
    with open(path, 'rb') as f:
        for _s, ch, msg in make_reader(f).iter_messages(log_time_order=False):
            topic = ch.topic.split('/', 1)[1] if ch.topic.startswith('record/') else ch.topic
            if topic != 'metric.sample':
                continue
            d = json.loads(msg.data)
            out.append(d)
    return out


def headline(sample):
    dims = sample.get('dims') or {}
    if isinstance(dims, list):
        return all(k == 't' for k, _ in dims)
    return all(k == 't' for k in dims)


def kind_of(v):
    if not isinstance(v, dict):
        return 'number', v
    if len(v) == 1:
        (k, inner), = v.items()
        return k.lower(), inner
    return 'object', v


def point(v):
    """The point estimate of one recorded sample value; None when it has none."""
    if v is None:
        return None
    if isinstance(v, (int, float)):
        return float(v)
    k, inner = kind_of(v)
    if k == 'count':
        return float(inner['count'] if isinstance(inner, dict) else inner)
    if 'count' in v and len(v) == 1:
        return float(v['count'])
    if k == 'insufficient' or v.get('insufficient'):
        return None
    if k == 'ratio' or k == 'proportion' or k == 'ratioofsums':
        return point(inner)
    if 'successes' in v and 'trials' in v:
        return v['successes'] / v['trials'] if v['trials'] else None
    if 'numerator' in v and 'denominator' in v:
        return v['numerator'] / v['denominator'] if v['denominator'] else None
    if 'mean' in v:
        return v['mean']
    if 'value' in v:
        return point(v['value'])
    if k in ('scalar', 'distribution', 'summary', 'exact', 'estimate'):
        return point(inner)
    return None


# ------------------------------------------------------------------------------ display

def fmt_number(v):
    """The Studio's formatNumber (ui/apps/studio/src/metrics/model.ts), re-derived."""
    if v is None or not math.isfinite(v):
        return '—'
    a = abs(v)
    if a == 0:
        return '0'
    if float(v).is_integer() and a < 1e15:
        return f'{int(v):,}'
    if a >= 1000:
        return f'{round(v):,}' if abs(round(v)) >= 1 else f'{v:,.0f}'
    if a >= 100:
        return f'{v:.1f}'
    if a >= 10:
        return f'{v:.2f}'
    if a >= 0.001:
        s = f'{v:.3g}'
        if 'e' in s:
            s = f'{v:.{max(0, 2 - int(math.floor(math.log10(a))))}f}'
        if '.' in s:
            s = s.rstrip('0').rstrip('.')
        return s
    m, e = f'{v:.2e}'.split('e')
    return f'{m}e{int(e):+d}'


def parse_shown(text):
    """A shown number back to a float (commas, a unit after it); None for a dash."""
    t = text.strip().split(' ')[0].replace(',', '')
    try:
        return float(t)
    except ValueError:
        return None


# ------------------------------------------------------------------------------ evidence

class Case:
    def __init__(self, d):
        self.dir = d
        self.name = os.path.basename(d.rstrip('/'))
        self.ev = json.load(open(os.path.join(d, 'evidence.json')))
        self._rec = {}

    def run(self, side):
        return self.ev['runs'].get(side)

    def order(self):
        return self.ev.get('order') or ['A'] + list(self.ev.get('variants', {}))

    def prev(self, side):
        o = self.order()
        return o[o.index(side) - 1] if side in o and o.index(side) > 0 else None

    def cmp(self, side):
        """(ui, api) of the comparison of `side` with the run before it."""
        c = (self.run(side) or {}).get('compare') or {}
        return c.get('ui') or {}, c.get('api') or {}

    def edits(self, side):
        return [(p, v) for p, v, _b in self.ev.get('variants', {}).get(side, [])]

    def records(self, side):
        if side not in self._rec:
            p = os.path.join(self.dir, f'{side}.mcap')
            self._rec[side] = mcap_samples(p) if os.path.exists(p) else None
        return self._rec[side]

    def api_series(self, side):
        """{metric: [(t_s, v)]} from the run's metrics.query answer at the metric period."""
        ans = self.run(side)['api_series']
        cols = [c['name'] for c in ans.get('columns', [])]
        out = {c: [] for c in cols[1:]}
        for row in ans.get('rows', []):
            t = row[0] / NS if cols and cols[0] in ('t', 't_ns') and row[0] is not None else row[0]
            for c, v in zip(cols[1:], row[1:]):
                if v is not None:
                    out[c].append((t, float(v)))
        return out

    def rec_series(self, side):
        """{metric: [(t_s, point)]} from the recording's headline samples."""
        rec = self.records(side)
        if rec is None:
            return None
        out = {}
        for s in rec:
            if not headline(s):
                continue
            p = point(s.get('value'))
            if p is None:
                continue
            out.setdefault(s['metric'], []).append((s['t'] / NS, p))
        for v in out.values():
            v.sort()
        return out

    def agg_of(self, side, metric):
        for c in self.run(side)['catalogue']:
            if c['name'] == metric:
                return c.get('agg', '')
        return ''

    def pooled(self, side, metric):
        """The run's value as theory pools it: proportions and ratios by their counts,
        distributions weighted by their n, a per-window count summed, a gauge (agg max or
        last) its largest or newest value."""
        rec = self.records(side)
        succ = trials = num = den = wsum = wn = 0.0
        counts = []
        for s in rec or []:
            if s['metric'] != metric or not headline(s):
                continue
            v = s.get('value')
            flat = json.dumps(v)
            if 'insufficient' in flat.lower():
                continue
            sv = find(v, 'successes'), find(v, 'trials')
            if sv[1]:
                succ += sv[0]
                trials += sv[1]
                continue
            nd = find(v, 'numerator'), find(v, 'denominator')
            if nd[1]:
                num += nd[0]
                den += nd[1]
                continue
            c = find(v, 'count')
            if c is not None and find(v, 'mean') is None:
                counts.append((s['t'], c))
                continue
            p = point(v)
            if p is not None:
                n = find(v, 'n') or 1
                wsum += p * n
                wn += n
        if trials:
            return succ / trials
        if den:
            return num / den
        if wn:
            return wsum / wn
        if counts:
            agg = self.agg_of(side, metric).lower()
            if agg.startswith('max'):
                return float(max(c for _, c in counts))
            if agg.startswith('last') or agg.startswith('gauge'):
                return float(sorted(counts)[-1][1])
            return float(sum(c for _, c in counts))
        return None


def find(v, key):
    if isinstance(v, dict):
        if key in v and isinstance(v[key], (int, float)):
            return v[key]
        for x in v.values():
            r = find(x, key)
            if r is not None:
                return r
    return None


def stats_of(series):
    vals = sorted(v for _, v in series)
    if not vals:
        return None
    n = len(vals)

    def q7(p):
        h = (n - 1) * p
        lo = math.floor(h)
        hi = min(lo + 1, n - 1)
        return vals[lo] + (h - lo) * (vals[hi] - vals[lo])
    return {'n': n, 'min': vals[0], 'mean': sum(vals) / n, 'p50': q7(0.5), 'p95': q7(0.95), 'p99': q7(0.99), 'max': vals[-1]}


# ------------------------------------------------------------------------------ checks
# Each check returns (ok, detail). `fault` mutates the evidence before the check runs.

def close(a, b, rel=1e-9, abs_=1e-12):
    return a is not None and b is not None and abs(a - b) <= max(abs_, rel * max(abs(a), abs(b)))


def c_cards_api(case, side, row):
    """Every card shows the newest window of its headline as the API answers it."""
    run = case.run(side)
    api = case.api_series(side)
    bad, n = [], 0
    for base, shown in run['cards'].items():
        if shown.startswith('EMPTY') or base not in api:
            continue
        n += 1
        want = fmt_number(api[base][-1][1]) if api[base] else '—'
        if shown != want:
            bad.append(f'{base}: shown {shown}, api {want}')
    return n > 0 and not bad, f'{n} cards; ' + ('; '.join(bad[:6]) or 'all equal')


def c_api_records(case, side, row):
    """metrics.query at the metric period holds the recording's headline samples, window for window."""
    api = case.api_series(side)
    rec = case.rec_series(side)
    if rec is None:
        return False, 'no recording'
    bad, n = [], 0
    for m, series in api.items():
        r = dict((round(t, 6), v) for t, v in rec.get(m, []))
        for t, v in series:
            n += 1
            want = r.get(round(t, 6))
            if want is None or not close(v, want, rel=1e-6, abs_=1e-9):
                bad.append(f'{m}@{t}: api {v} rec {want}')
    return n > 0 and not bad, f'{n} windows over {len(api)} metrics; ' + ('; '.join(bad[:5]) or 'all equal')


def c_expanded(case, side, row):
    """The single-metric view's statistics are those of the recorded series over the whole run."""
    run = case.run(side)
    ex = run.get('expanded')
    if not ex or not ex['rows']:
        return False, 'no single-metric view read'
    rec = case.rec_series(side) or {}
    bad = []
    for name, cells in ex['rows'].items():
        st = stats_of(rec.get(name, []))
        if st is None:
            bad.append(f'{name}: no records')
            continue
        for k, shown in cells.items():
            want = f'{st[k]:,}' if k == 'n' else fmt_number(st[k])
            if shown != want:
                bad.append(f'{name}.{k}: shown {shown}, records {want}')
    return not bad, f'{len(ex["rows"])} series; ' + ('; '.join(bad[:6]) or 'all equal')


def c_compare_ui_api(case, side, row):
    """Every compare row shows runs.compare's values for the two runs."""
    ui_doc, api_doc = case.cmp(side)
    ui = ui_doc.get('rows', {})
    api = {m['name']: m for m in api_doc.get('metrics', [])}
    bad, n = [], 0
    for name, r in ui.items():
        m = api.get(name)
        if m is None:
            bad.append(f'{name}: not in runs.compare')
            continue
        n += 1
        for s in ('a', 'b'):
            want = fmt_number(m.get(s)) if m.get(s) is not None else '—'
            if r[s].split(' ')[0] != want:
                bad.append(f'{name}.{s}: shown {r[s]}, api {want}')
    return n > 0 and not bad, f'{n} rows; ' + ('; '.join(bad[:6]) or 'all equal')


def c_compare_records(case, side, row):
    """runs.compare's pooled value of every metric is the run's own value from its records."""
    api = case.cmp(side)[1].get('metrics', [])
    bad, n = [], 0
    only = row.get('metrics')
    for m in api:
        name = m['name']
        if '.' in name or (only and name not in only):
            continue
        for s, sd in (('a', case.prev(side)), ('b', side)):
            got = m.get(s)
            want = case.pooled(sd, name)
            if got is None and want is None:
                continue
            n += 1
            if not close(got, want, rel=1e-6, abs_=1e-9):
                bad.append(f'{name}.{s}: compare {got}, records {want} (agg {case.agg_of(sd, name)})')
    return n > 0 and not bad, f'{n} pooled values; ' + ('; '.join(bad[:6]) or 'all equal')


def scenario_doc(case, side):
    doc = case.run(side)['scenario']
    return doc.get('scenario', doc) if isinstance(doc, dict) else doc


def c_same_seed(case, side, row):
    """The dialog and runs.compare say whether the two runs share a seed, as their scenarios do."""
    ui, api = case.cmp(side)
    seeds = [scenario_doc(case, s).get('seed') for s in (case.prev(side), side)]
    want = str(seeds[0] == seeds[1]).lower()
    ok = ui.get('same_seed') == want and str(api.get('same_seed')).lower() == want
    return ok, f'seeds {seeds}; ui {ui.get("same_seed")}, api {api.get("same_seed")}'


def c_settings_named(case, side, row):
    """The comparison names every edited setting with its new value."""
    ui, api = case.cmp(side)
    text = ui.get('settings', '')
    changes = api.get('settings', [])
    bad = []
    for pointer, value in case.edits(side):
        if value not in text:
            bad.append(f'{pointer}={value} not in the dialog')
        key = pointer.strip('/').replace('/', '.')
        if not any(key in json.dumps(c) for c in changes):
            bad.append(f'{key} not in runs.compare settings')
    return not bad and bool(case.edits(side)), '; '.join(bad) or f'{len(case.edits(side))} edits named'


def same_value(cur, typed):
    """Whether a scenario value is what was typed: an emptied field is unset, a JSON field is
    its parsed document, a number is equal as a number, a boolean as 'true' / 'false'."""
    if typed == '':
        return cur is None
    if isinstance(cur, bool):
        return str(cur).lower() == typed
    try:
        want = json.loads(typed)
    except ValueError:
        want = typed
    norm = lambda x: json.dumps(x, sort_keys=True, default=str).replace('.0,', ',').replace('.0}', '}').replace('.0]', ']')
    if isinstance(cur, (dict, list)):
        return norm(cur) == norm(want)
    if isinstance(cur, (int, float)) and isinstance(want, (int, float)):
        return close(float(cur), float(want))
    return str(cur) == str(want)


def c_edit_landed(case, side, row):
    """The scenario the variant ran holds each edit (scenario.get after the run)."""
    doc = scenario_doc(case, side)
    bad = []
    for pointer, value in case.edits(side):
        cur = doc
        for part in pointer.strip('/').split('/'):
            if isinstance(cur, list):
                cur = cur[int(part)] if part.isdigit() and int(part) < len(cur) else None
            elif isinstance(cur, dict):
                cur = cur.get(part)
            else:
                cur = None
        same = same_value(cur, value)
        if not same:
            bad.append(f'{pointer}: ran {cur}, typed {value}')
    return not bad and bool(case.edits(side)), '; '.join(bad) or 'every edit ran'


def flat(doc, prefix=''):
    out = {}
    if isinstance(doc, dict):
        for k, v in doc.items():
            out.update(flat(v, f'{prefix}/{k}'))
    elif isinstance(doc, list):
        for i, v in enumerate(doc):
            out.update(flat(v, f'{prefix}/{i}'))
    else:
        out[prefix] = doc
    return out


def c_only_edits(case, side, row):
    """The variant ran the base scenario with its own edits and nothing else: the previous
    variant's edits were typed back, and the settings window changed nothing it was not asked to."""
    a = flat(scenario_doc(case, 'A'))
    b = flat(scenario_doc(case, side))
    edited = {p for p, _v in case.edits(side)}
    diff = sorted(k for k in set(a) | set(b) if a.get(k) != b.get(k))
    stray = [k for k in diff if not any(k == p or k.startswith(p + '/') for p in edited)]
    return not stray and bool(diff), f'{len(diff)} paths differ; ' + ('stray: ' + ', '.join(f"{k} {a.get(k)!r}->{b.get(k)!r}" for k in stray[:5]) if stray else 'only the edits')


def pki_api(case, side):
    run = case.run(side)
    veh = run.get('pki_vehicles') or {}
    vs = veh.get('vehicles') or (veh.get('answer') or {}).get('vehicles') or []
    trust = run.get('pki_trust') or {}
    state = trust.get('state') or {}
    devices = sum(p.get('devices', {}).get('devices', 0) for p in state.get('providers', []))
    revoked = sum(1 for v in vs if v.get('revoked') is True or ((v.get('truth') or {}).get('revoked_certs') or 0) > 0)
    return {'certs': sum(v.get('usable', 0) for v in vs), 'vehicles': len(vs), 'devices': devices, 'revoked': revoked}


def c_pki_strip(case, side, row):
    """The health strip's certificates and revoked devices are the API's vehicles and providers."""
    run = case.run(side)
    strip = run.get('pki_strip')
    if not strip:
        return False, 'no health strip read'
    a = pki_api(case, side)
    want = {'certs': f'{a["certs"]:,}', 'revoked': f'{a["revoked"]} of {a["devices"]}'}
    bad = [f'{k}: shown {strip[k]}, api {v}' for k, v in want.items() if strip.get(k) != v]
    return not bad, '; '.join(bad) or f'certs {strip["certs"]}, revoked {strip["revoked"]}'


def c_deterministic(case, side, row, cases=None):
    """Run A is the same base scenario in every case: one output digest across sessions."""
    digests = {c.name: c.run('A')['status']['engine']['output_digest'] for c in cases if c.run('A')}
    return len(set(digests.values())) == 1 and len(digests) >= 2, f'{len(digests)} sessions, {len(set(digests.values()))} digests'


def value_of(case, side, metric):
    """A run's headline number for a prediction row, times the fault's scale when one is set."""
    v = raw_value(case, side, metric)
    return None if v is None else v * getattr(case, 'scale', {}).get(side, 1.0)


def raw_value(case, side, metric):
    """`cost.*` and `stats.*` from the session, `windows.<m>` a count of windows, else pooled."""
    run = case.run(side)
    if metric == 'cost.wall_s':
        return run['wall_ms'] / 1000
    if metric == 'cost.footprint_mb':
        return run['footprint_kb'] / 1024
    if metric.startswith('stats.'):
        st = run['status']['engine']['stats']
        k = metric.split('.', 1)[1]
        if k.startswith('tx.'):
            return float(sum(st['tx_by_type'].values())) if k == 'tx.all' else float(st['tx_by_type'].get(k[3:], 0))
        return float(st[k]) if st.get(k) is not None else None
    if metric == 'pki.certs':
        return parse_shown((run.get('pki_strip') or {}).get('certs', ''))
    if metric.startswith('windows.'):
        return float(len((case.rec_series(side) or {}).get(metric.split('.', 1)[1], [])))
    return case.pooled(side, metric)


def c_predict(case, side, row):
    """`metric` moves `expect` (up/down/same) from A to B by at least `min_change` (relative),
    or within `ratio` [lo, hi] of B/A when given."""
    a = value_of(case, row.get('against', 'A'), row['metric'])
    b = value_of(case, side, row['metric'])
    if a is None or b is None:
        return False, f'A {a}, B {b}: not observed'
    rel = (b - a) / abs(a) if a else (math.inf if b > 0 else (-math.inf if b < 0 else 0.0))
    mc = row.get('min_change', 0.01)
    exp = row['expect']
    if 'delta' in row:
        lo, hi = row['delta']
        ok = lo <= b - a <= hi
    elif 'ratio' in row:
        lo, hi = row['ratio']
        ok = a != 0 and lo <= b / a <= hi
    elif exp == 'up':
        ok = rel >= mc
    elif exp == 'down':
        ok = rel <= -mc
    elif exp == 'same':
        ok = abs(rel) <= row.get('tolerance', 0.0)
    else:
        ok = rel != 0
    return ok, f'A {a:.6g} -> B {b:.6g} ({rel:+.1%})'


CHECKS = {
    'cards_api': c_cards_api, 'api_records': c_api_records, 'expanded': c_expanded,
    'compare_ui_api': c_compare_ui_api, 'compare_records': c_compare_records,
    'same_seed': c_same_seed, 'settings_named': c_settings_named, 'edit_landed': c_edit_landed,
    'pki_strip': c_pki_strip, 'only_edits': c_only_edits, 'deterministic': c_deterministic, 'predict': c_predict,
}


# ------------------------------------------------------------------------------ faults

def apply_fault(case, row):
    """Breaks the evidence the way the row's `fault` says, so its check must go red."""
    f = row.get('fault', '')
    side = row.get('side', 'A')
    run = case.ev['runs'][side]
    if f == 'card':
        k = next(k for k, v in run['cards'].items() if not v.startswith('EMPTY'))
        run['cards'][k] = '123,456'
    elif f == 'api':
        rows = run['api_series']['rows']
        r = next(r for r in rows if any(x is not None for x in r[1:]))
        i = next(i for i, x in enumerate(r) if i > 0 and x is not None)
        r[i] = r[i] * 1.5 + 1
    elif f == 'stat':
        ex = run['expanded']['rows']
        k = next(iter(ex))
        ex[k]['mean'] = '9,999'
    elif f == 'compare_ui':
        r = next(iter(run['compare']['ui']['rows'].values()))
        r['a'] = '777,777'
    elif f == 'compare_api':
        for m in run['compare']['api']['metrics']:
            if m.get('a') is not None and '.' not in m['name'] and (not row.get('metrics') or m['name'] in row['metrics']):
                m['a'] = m['a'] * 3 + 1
                break
    elif f == 'seed':
        ui = run['compare']['ui']
        ui['same_seed'] = 'false' if ui['same_seed'] == 'true' else 'true'
    elif f == 'settings':
        run['compare']['ui']['settings'] = ''
    elif f == 'stray':
        doc = scenario_doc(case, side)
        doc['seed'] = str(doc.get('seed')) + '1'
    elif f == 'edit':
        case.ev['variants'][side] = [[p, v + '9', b] for p, v, b in case.ev['variants'][side]]
    elif f == 'strip':
        run['pki_strip']['certs'] = '1'
    elif f == 'digest':
        case.ev['runs']['A']['status']['engine']['output_digest'] = 'f' * 64
    elif f == 'scale':
        # The variant's value moved by half again: a 'same' row must see it, an 'up' or
        # 'down' row is swapped below instead.
        case.scale = {side: 1.5}
    elif f == 'swap':
        against = row.get('against', 'A')
        case.ev['runs'][against], case.ev['runs'][side] = run, case.ev['runs'][against]
        case._rec = {against: case.records(side), side: case.records(against)}
    else:
        raise SystemExit(f'row {row["id"]}: unknown fault {f!r}')


# ------------------------------------------------------------------------------ driver

def load_yaml(path):
    import yaml
    return yaml.safe_load(open(path))


def judge(doc, cdir, fault=False):
    cases = {}
    for name in sorted(os.listdir(cdir)):
        p = os.path.join(cdir, name)
        if os.path.exists(os.path.join(p, 'evidence.json')):
            cases[name] = Case(p)
    out = []
    for row in doc['rows']:
        names = row.get('cases') or [row['case']]
        for cname in names:
            c0 = cases.get(cname)
            if c0 is None:
                out.append((row['id'], cname, None, 'not run'))
                continue
            sides = []
            for s in row.get('sides') or [row.get('side', 'A')]:
                if s == 'all':
                    sides += [x for x in c0.order() if x != 'A']
                elif s == 'variants':
                    sides += c0.order()[1:]
                else:
                    sides.append(s)
            for side in sides:
                label = f'{cname}:{side}'
                if c0.run(side) is None:
                    out.append((row['id'], label, None, 'run not reached'))
                    continue
                r = dict(row, side=side)
                c = c0
                if fault:
                    c = copy.copy(c0)
                    c.ev = copy.deepcopy(c0.ev)
                    c._rec = dict(c0._rec)
                    try:
                        apply_fault(c, r)
                    except (StopIteration, KeyError, TypeError) as e:
                        out.append((row['id'], label, None, f'fault not applicable: {e!r}'))
                        continue
                fn = CHECKS[row['check']]
                try:
                    if row['check'] == 'deterministic':
                        pool = [x for x in cases.values() if x is not c0] + [c]
                        ok, detail = fn(c, side, r, cases=pool)
                    else:
                        ok, detail = fn(c, side, r)
                except Exception as e:  # noqa: BLE001 - a check that cannot run is red
                    ok, detail = False, f'error {e!r}'
                out.append((row['id'], label, ok, detail))
    return out


def main(argv):
    if len(argv) != 4 or argv[1] not in ('check', 'selftest'):
        print(__doc__)
        return 2
    doc = load_yaml(argv[2])
    if argv[1] == 'check':
        res = judge(doc, argv[3])
        for rid, cname, ok, detail in res:
            v = 'PASS' if ok else ('----' if ok is None else 'FAIL')
            print(f'{v} {rid} [{cname}] {detail[:300]}')
        print(f'{sum(1 for r in res if r[2])} pass, {sum(1 for r in res if r[2] is False)} fail, {sum(1 for r in res if r[2] is None)} not run')
        return 0
    base = {(r[0], r[1]): r[2] for r in judge(doc, argv[3])}
    res = judge(doc, argv[3], fault=True)
    red = 0
    for rid, cname, ok, detail in res:
        if base.get((rid, cname)) is None:
            continue
        went_red = ok is False
        red += went_red
        print(f'{"RED " if went_red else "STAYED-GREEN"} {rid} [{cname}] {detail[:160]}')
    print(f'{red} of {sum(1 for k, v in base.items() if v is not None)} rows went red under their fault')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
