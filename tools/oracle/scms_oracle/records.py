"""Recording-level oracle (oracle-final): standard rules checked over a run's own PKI records.

``oracle.py records RUN_DIR...`` reads ``RUN_DIR/recording.mcap`` (the engine's ``pki.flow``,
``pki.state``, ``node.credentials`` and ``ma.decision`` records, written with the PKI view on and
``--record-level events`` or more) and ``RUN_DIR/scenario.resolved.yaml`` (``time.t0``), decodes
every captured SPDU with the WP-A0 codec and checks the rules below.  Nothing here is taken from
the engine's Rust: each rule cites the standard text it comes from (the ASN.1 module comments
under ``tools/oracle/asn1`` are the 1609.2-2022 / 1609.2.1-2022 text; the research notes 02/03
cite the profiles).  ``--mutate`` injects one named fault into the loaded records at a time and
requires its target rule to go red, so that no rule here is one that cannot fail.

Rule ids (and sources):
  DECODE      every captured PDU decodes as its type and re-encodes byte for byte (canonical COER,
              1609.2 6.2; 02 sec. 3)
  TIME       pki.state time32 = Time32(t0 + t), TAI with leap seconds (1609.2 6.3.11; 02 sec. 4)
  CERT-ID     every recorded certificate key is the HashedId8 of that certificate (1609.2 6.4.3)
  CRL-ENV     SecuredCrl = Signed CrlContents, psid 256, headerInfo psid only, SignerSingleCert
              or digest, version 1, signature verifies (1609.2 7.4; 02 sec. 11.1; 03 sec. 3.1)
  CRL-SSP     the signer's CrlSsp lists crlSeries; crlCraca = HashedId8 of the signer (isCraca)
              or of its issuer (issuerIsCraca) (Ieee1609Dot2CrlSsp CrlSsp, CracaType comments)
  CRL-NEXT    nextCrl strictly after issueDate (CrlBaseTypes CrlContents.nextCrl comment)
  CRL-TIME    issueDate not after the instant the CRL is first sent (CrlContents.issueDate)
  CRL-SEQ     per (crlCraca, crlSeries), in issue order: issueDate strictly increases; hash CRLs:
              crlSerial +1 per CRL; linkage CRLs: iRev never decreases, indexWithinI is 0 for the
              first CRL of an iRev and +1 per further CRL with that iRev (CrlBaseTypes comments)
  CRL-IREV    iRev <= the i-period at issueDate (US weekly calendar, 03 sec. 6.1), so that the
              seeds at iRev can be advanced to every current certificate (1609.2 5.1.3.4)
  CRL-KEEP    a later full CRL keeps every earlier entry that has not expired: linkage entries
              while iMax >= the later CRL's i, hash entries while expiry > its issueDate
              ([SM-PROV] 3.4.9 "revoked certificates stay until expiry")
  CRL-SLA     each CRL is followed by the next one of its (crlCraca, crlSeries) no later than its
              nextCrl ([SM-PROV] 3.4.10), when the run lasts past that nextCrl
  CRL-PUB     a revoked device is on a CRL within one business day of the MA's decision, and on
              none before it ([SM-PROV] 3.4.10)
  CRL-STATE   the engine's CRL rows (pki.state crls) agree with the decoded bytes
  CRL-MATCH   every revoked device is matched by its provider's linkage CRL through cracaId and
              crlSeries agreement and the seed algorithm; no device the MA did not decide on is
              matched (1609.2 5.1.3.2 / 5.1.3.4; Ieee1609Dot2 ToBeSignedCertificate.cracaId)
  CRACA-CERT  every certificate's cracaId is the HashedId3 of a CRACA whose CRL signer lists the
              certificate's crlSeries; series per type: pseudonym 1, application/identification 3,
              enrolment 4 ([SMP] Table 1; Ieee1609Dot2 cracaId/crlSeries comments)
  ENR-MAP     EcaEeCertResponse.requestHash = HashedId8 of the device's EeEcaCertRequestSpdu as
              sent; the ECA's signature verifies ([ASN-2.1] EcaEeInterface)
  DL-ACK      RaEeCertAck: Signed psid 35 by an RA certificate, requestHash = HashedId8 of the
              device's EeRaCertRequestSpdu as sent, firstI present (butterfly), nextDlTime >=
              generationTime ([ASN-2.1] EeRaInterface RaEeCertAck comments)
  DL-ENC      EeRaDownloadRequestSpdu = Encrypted{Signed{..}}: one certRecipInfo to an RA
              certificate; inner Signed psid 35, headerInfo psid only, SignerSingleCert
              ([ASN-2.1] Protocol SignedEncrypted; 03 sec. 3.1-3.2)
  DL-SIGN     the Download-Req is signed by the sending device's own enrolment certificate and the
              signature verifies ([CAMP122] 6.3; [SM-MBR] 4; [SMP] 4.4 eeAuth = enrollment)
  DL-TIME     generationTime within 5 s of the send time ([CAMP122] 5.1.7 clock tolerance)
  DL-NAME     filename `<requestHash 16 HEX>_<i HEX>.zip` with i >= firstI, `<requestHash>.info`,
              or `misbehavior-report` on a report upload ([CAMP122] 6.3.6, Table 79; 03 sec. 4.2)
  DL-AFTER    a download is not requested before the RaEeCertAck's nextDlTime, "the time after
              which the EE should connect to the RA to download" (EeRaInterface comments)
  ENR-LIFE    an enrolment certificate is valid 6 years ([CAMP122] Table 3, SCMS-1307)
  RA-BLOCK    no certificate batch reaches a device more than a day after the MA's revoke
              decision on it: the RA blacklists its enrolment certificate, refuses its login
              (SCMS-1204) and stops pre-generation (SCMS-509) ([CAMP122]; 03 sec. 8.2)
  DL-WAIT     the same file is not requested again within download-minWait (300 s), and at most
              download-maxReqs (100) times ([SMP] Table 3; 03 sec. 6.2)
  MBR-ENC     a report is Encrypted to one certRecipInfo naming an MA certificate whose MaSsp
              relevantPsids holds 0x20 ([SM-MBR] 3; [ASN-2.1] MaSsp)
  MBR-SIG     inner Signed: psid 0x26, headerInfo {psid, generationTime} only, signer a full
              pseudonym certificate whose appPermissions hold 0x26, generationTime inside its
              validity and not after the upload, signature verifies ([SM-MBR] 1, 3; 1609.2 5.2)
  MBR-UP      each EE upload carries the report and a `misbehavior-report` Download-Req
              ([SM-MBR] 4); the RA forwards reports to the MA byte for byte, each at most once
  CTL-SUM     CTL history: sequence +1 per CTL, each signed by at least quorum electors,
              effectiveDate increasing; a CTL is in force only from its effectiveDate
              ([ASN-2.1] CertManagement FullIeeeTbsCtl; 03 sec. 10)
  CTL-DEV     a device's installed CTL sequence never decreases and never runs ahead of the
              published one
  OF-3        every device holds each CRACA's current pseudonym CRL (series 1) from the moment
              it is on the road: before its first node.security row a CRL Store sends it, per
              CRACA that has published, a SecuredCrl that is one of that CRACA's issued lists;
              every node.security row lists, per such CRACA, a list it issued no later than the
              row, at least as new as the fetched one, and no older than the newest list issued a
              week before the row ("as often as possible but no later than once a week",
              [CAMP122] sec. 3 item 10; a receiver checks revocation against the current list,
              1609.2 5.1.3.4; [SMP] Table 1 series 1)
  OF-3-JOIN   the list a device fetches when it joins is its CRACA's newest at that instant
              (R-JOINCRL: "a joiner holds the newest list each CRACA published before it
              joined"); the same sources
"""

import collections
import copy
import datetime as dt
import functools
import json
import os
import re

from . import certs, chain, coer, prims, time1609

NS = 1_000_000_000
WEEK = 604800
# US i-period epoch: Tuesday 2015-01-06 04:00 EST = 09:00 UTC ([SMP] 2.1.17 Table 2)
I_EPOCH = dt.datetime(2015, 1, 6, 9, 0, tzinfo=dt.timezone.utc)
MIN_WAIT_S = 300
MAX_REQS = 100
CLOCK_TOL_S = 5
SECMGMT_PSID, CRL_PSID, MBR_PSID, BSM_PSID = 35, 256, 0x26, 0x20
FN_BATCH = re.compile(r'([0-9A-F]{16})_([0-9A-F]{1,8})\.zip')
FN_INFO = re.compile(r'([0-9A-F]{16})\.info')


# --------------------------------------------------------------------------------------------
# loading

class Rec:
    pass


def _t0(run_dir):
    for line in open(os.path.join(run_dir, 'scenario.resolved.yaml')):
        m = re.match(r"\s*t0:\s*['\"]?([0-9T:\-]+)Z?['\"]?", line)
        if m:
            return dt.datetime.fromisoformat(m.group(1)).replace(tzinfo=dt.timezone.utc)
    raise ValueError('no time.t0 in scenario.resolved.yaml')


def load(run_dir):
    from mcap.reader import make_reader
    R = Rec()
    R.name = os.path.basename(run_dir.rstrip('/'))
    R.t0 = _t0(run_dir)
    R.pdus = []            # every PDU occurrence, with its carrying message
    R.cert_hex = {}        # recorded key -> hex (pki.state and node.credentials stores)
    R.crl_rows = {}        # (provider, id8) -> row (first seen, with hex when present)
    R.ctl = []             # (t, time32, summary) on change
    R.times = []           # (t, time32)
    R.creds = collections.defaultdict(list)    # device -> [(t, ctl_sequence)]
    R.decisions = []
    R.reports = []         # EE report messages: (id, from, transport, carries PDUs, instance, t)
    R.revstages = {}       # proto.revocation: subject -> {stage: first t}
    R.security = collections.defaultdict(list)  # node.security: node -> [(t, pseudonym, crls)]
    R.end_t = 0
    with open(os.path.join(run_dir, 'recording.mcap'), 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            topic = ch.topic
            if topic == 'record/pki.flow':
                q = json.loads(m.data)
                R.end_t = max(R.end_t, q['t'])
                if q['kind'] == 'msg' and q['name'] == 'report' and q.get('from', '').startswith('ee/'):
                    R.reports.append((q['id'], q['from'], q.get('transport'), bool(q.get('pdus')),
                                      q.get('instance'), q['t'], q.get('bytes')))
                for d in q.get('pdus') or []:
                    R.pdus.append({'id': d['id'], 'type': d['type'], 'from': d['from'], 'to': d['to'], 't': d['t'],
                                   'hex': d['hex'], 'id8': d.get('id8'),
                                   'opened': (d.get('opened') or {}).get('hex'), 'sealed_to': d.get('sealed_to'),
                                   'msg': q['name'], 'flow': q.get('flow'), 'kind': q['kind'],
                                   'transport': q.get('transport'), 'msg_from': q.get('from'), 'msg_t': q['t'],
                                   'instance': q.get('instance'), 'provider': q.get('provider')})
            elif topic == 'record/pki.state':
                q = json.loads(m.data)
                R.end_t = max(R.end_t, q['t'])
                R.times.append((q['t'], q.get('time32')))
                for k, h in (q.get('certificates') or {}).items():
                    R.cert_hex.setdefault(k, h)
                for row in q.get('crls') or []:
                    key = (row.get('provider'), row.get('id8'))
                    if key not in R.crl_rows or ('hex' in row and 'hex' not in R.crl_rows[key]):
                        R.crl_rows[key] = dict(row, seen_t=q['t'])
                c = q.get('ctl')
                if c is not None:
                    s = {k: c.get(k) for k in ('sequence', 'quorum', 'electors', 'effective', 'signers', 'history',
                                               'pending', 'roots')}
                    if not R.ctl or R.ctl[-1][2] != s:
                        R.ctl.append((q['t'], q.get('time32'), s))
            elif topic == 'record/node.credentials':
                q = json.loads(m.data)
                for k, h in (q.get('certificates') or {}).items():
                    R.cert_hex.setdefault(k, h)
                R.creds[q['device']].append((q['t'], q.get('ctl_sequence'), q.get('provider'), q.get('revoked'),
                                             sorted((q.get('certificates') or {}).keys())))
            elif topic == 'record/node.security':
                q = json.loads(m.data)
                R.security[q['node']].append((q['t'], q.get('pseudonym'), [
                    (c.get('craca'), c.get('series'), c.get('issue_date'), c.get('next_crl'))
                    for c in q.get('crls') or []]))
            elif topic == 'record/ma.decision':
                R.decisions.append(json.loads(m.data))
            elif topic == 'record/proto.revocation':
                q = json.loads(m.data)
                R.revstages.setdefault(q.get('id'), {}).setdefault(q.get('stage'), q['t'])
    return R


# --------------------------------------------------------------------------------------------
# helpers

def t32(R, t_ns):
    return time1609.time32(R.t0 + dt.timedelta(microseconds=t_ns // 1000))


def t64(R, t_ns):
    return time1609.time64(R.t0 + dt.timedelta(microseconds=t_ns // 1000))


def i_at_time32(v):
    u = time1609.from_time32(v) or time1609.from_time32(v - 1)
    return int((u - I_EPOCH).total_seconds() // WEEK)


@functools.lru_cache(maxsize=200000)
def dec(type_name, b):
    """Cached decode: the rules only read the values (the fault mutators use coer.decode)."""
    return coer.decode(type_name, b)


def signed(d):
    """Ieee1609Dot2Data value -> signedData body, or None."""
    return d['content'][1] if d['content'][0] == 'signedData' else None


def payload_bytes(sd):
    inner = sd['tbsData']['payload']['data']
    return inner['content'][1] if inner['content'][0] == 'unsecuredData' else None


def ssp_of(cert, psid):
    for p in cert['toBeSigned'].get('appPermissions') or []:
        if p['psid'] == psid:
            s = p.get('ssp')
            if s is None:
                return None
            return s[1] if s[0] == 'opaque' else s
    return False


def mgmt_role(cert):
    s = ssp_of(cert, SECMGMT_PSID)
    if not isinstance(s, bytes):
        return None
    try:
        return dec('SecurityMgmtSsp', s)
    except Exception:
        return None


class Store:
    """Certificates by the oracle's own HashedId8."""

    def __init__(self):
        self.by = {}

    def add(self, cert):
        h = h8(cert)
        self.by.setdefault(h, cert)
        return h

    def add_hex(self, hx):
        return self.add(dec('Certificate', bytes.fromhex(hx)))

    def get(self, h8):
        return self.by.get(h8)

    def issuer(self, cert):
        iss = cert['issuer']
        return None if iss[0] == 'self' else self.by.get(iss[1])

    def key(self, cert):
        h = h8(cert)
        return _key(self, h, cert)


_KEYS = {}


def _key(store, h, cert):
    if h not in _KEYS:
        if cert['toBeSigned']['verifyKeyIndicator'][0] == 'verificationKey':
            _KEYS[h] = certs.verification_key(cert)
        else:
            iss = store.issuer(cert)
            _KEYS[h] = None if iss is None else certs.verification_key(cert, iss)
    return _KEYS[h]


@functools.lru_cache(maxsize=None)
def _verify(curve_name, qx, qy, digest, r, s):
    from . import ec
    C = ec.CURVES[curve_name]
    return prims.ecdsa_verify(C, (qx, qy), digest, r, s)


_BY_ID = {}


def _memo(tag, obj, fn):
    """Memoise fn(obj) by object identity; the entry keeps obj alive so its id is never reused.
    Decoded values come from the cached `dec`, so equal bytes give the same object."""
    k = (tag, id(obj))
    hit = _BY_ID.get(k)
    if hit is not None and hit[0] is obj:
        return hit[1]
    v = fn(obj)
    _BY_ID[k] = (obj, v)
    return v


def h8(cert):
    return _memo('h8', cert, certs.hashed_id8)


def verify_sd(store, sd, signer_cert):
    k = store.key(signer_cert)
    if k is None:
        return False, 'signer key unresolved (issuer not in the store)'
    C, Q = k
    hname = certs.HASH_OF_CURVE[C.name]
    tbs = _memo('tbs', sd['tbsData'], lambda t: coer.encode('ToBeSignedData', t))
    digest = prims.digest_1609(hname, tbs, _memo('canon', signer_cert, certs.encode_canonical))
    r, s = certs.sig_rs(sd['signature'])
    ok = _verify(C.name, Q[0], Q[1], digest, r, s)
    return ok, '' if ok else 'signature does not verify'


def signer_cert(store, sd):
    k, v = sd['signer']
    if k == 'certificate':
        return v[0] if len(v) == 1 else None, len(v)
    if k == 'digest':
        return store.get(v), 0
    return None, -1


class Rules:
    def __init__(self, R):
        self.R = R
        self.ok = collections.Counter()
        self.bad = collections.defaultdict(list)
        self.na = set()
        self.info = {}
        self.first_crl = {}

    def check(self, rule, cond, where=''):
        if cond:
            self.ok[rule] += 1
        else:
            self.bad[rule].append(where)
        return cond

    def rows(self):
        out = []
        for r in sorted(set(self.ok) | set(self.bad)):
            out.append((r, self.ok[r], self.bad.get(r, [])))
        return out


# --------------------------------------------------------------------------------------------
# rules

def run_rules(R):
    X = Rules(R)
    store = Store()
    # CERT-ID
    for k, hx in R.cert_hex.items():
        h = store.add_hex(hx)
        X.check('CERT-ID', h.hex() == k, f'recorded {k} != HashedId8 {h.hex()}')
    for t, v in R.times:
        if v is not None:
            X.check('TIME', v == t32(R, t), f't={t}: recorded {v} != oracle {t32(R, t)}')
    decoded = []
    for p in R.pdus:
        typ = 'EncryptedIndividualPLV' if p['type'] == 'EncryptedIndividualPLV' else 'Ieee1609Dot2Data'
        try:
            raw = bytes.fromhex(p['hex'])
            outer = dec(typ, raw)
            X.check('DECODE', coer.encode(typ, outer) == raw, f'{p["id"]} {p["type"]}: not canonical COER')
            if typ != 'Ieee1609Dot2Data':
                continue
            opened = dec('Ieee1609Dot2Data', bytes.fromhex(p['opened'])) if p.get('opened') else None
        except Exception as e:
            X.check('DECODE', False, f'{p["id"]} {p["type"]}: {e}')
            continue
        decoded.append((p, outer, opened))
        # certificates carried in SPDUs join the store
        for d in (outer, opened):
            sd = signed(d) if d else None
            if sd and sd['signer'][0] == 'certificate':
                for c in sd['signer'][1]:
                    store.add(c)
    by_type = collections.defaultdict(list)
    for x in decoded:
        by_type[x[0]['type']].append(x)
    enrol = rule_enrolment(X, store, by_type)
    acks = rule_acks(X, store, by_type)
    rule_downloads(X, R, store, by_type, enrol, acks)
    rule_reports(X, R, store, by_type)
    crls = rule_crl(X, R, store, by_type)
    rule_craca(X, R, store, crls)
    rule_ctl(X, R)
    rule_block(X, R)
    rule_held_crls(X, R, crls)
    return X


PSEUDONYM_SERIES = 1


def _crl_lists(R, crls):
    """crlCraca hex -> [(issued ns, issueDate, nextCrl, hex)] of the pseudonym series, in issue order."""
    issued_at = {id8: row.get('issued_at') for (_, id8), row in R.crl_rows.items()}
    lists = collections.defaultdict(list)
    for c in crls:
        cc = c['cc']
        if cc['crlSeries'] != PSEUDONYM_SERIES:
            continue
        at = issued_at.get(prims.hashed_id(bytes.fromhex(c['hex']), 8).hex())
        lists[cc['crlCraca'].hex()].append((c['p']['t'] if at is None else at, cc['issueDate'], cc['nextCrl'], c['hex']))
    for v in lists.values():
        v.sort()
    return lists


def _device_of_node(R):
    """node -> device, by the pseudonym a node.security row names and the certificates the
    device's node.credentials rows list."""
    owner = {}
    for dev, rows in R.creds.items():
        for row in rows:
            for k in row[4]:
                owner.setdefault(k, dev)
    return {node: next((owner[r[1]] for r in rows if r[1] in owner), None) for node, rows in R.security.items()}


def rule_held_crls(X, R, crls):
    """OF-3 and OF-3-JOIN (see the module docstring)."""
    if not R.security:
        return
    lists = _crl_lists(R, crls)

    def newest(craca, t_ns):
        best = None
        for x in lists[craca]:
            if x[0] <= t_ns and (best is None or x[1] > best[1]):
                best = x
        return best
    devices = _device_of_node(R)
    fetched = collections.defaultdict(list)  # device -> [(t, from, hex)]
    for p in R.pdus:
        if p['type'] == 'SecuredCrl' and str(p['to']).startswith('ee/'):
            fetched[p['to']].append((p['t'], p['from'], p['hex']))
    by_hex = {x[3]: (craca, x) for craca, v in lists.items() for x in v}
    behind = 0
    max_lag = 0
    for node, rows in sorted(R.security.items()):
        rows = sorted(rows, key=lambda r: r[0])
        dev = devices.get(node)
        w = f'node {node}'
        if not X.check('OF-3', dev is not None, w + ': no node.credentials device holds its pseudonyms'):
            continue
        w = f'{dev} (node {node})'
        join = rows[0][0]
        got = {}
        for t, frm, hx in sorted(fetched[dev]):
            if t <= join and hx in by_hex and 'crl-store' in str(frm):
                craca, x = by_hex[hx]
                got[craca] = (t, x)
        for craca in sorted(lists):
            if newest(craca, join) is None:
                continue
            if not X.check('OF-3', craca in got, w + f': no SecuredCrl of CRACA {craca} from a CRL Store by its '
                                                     f'first row at {join / NS:.3f} s'):
                continue
            t, x = got[craca]
            now = newest(craca, t)
            X.check('OF-3-JOIN', x[1] >= now[1], w + f': fetched the {craca} list of issueDate {x[1]} at '
                                                      f'{t / NS:.3f} s; the newest then was {now[1]}')
        for t, _, held in rows:
            have = {c: (i, n) for c, s_, i, n in held if s_ == PSEUDONYM_SERIES}
            for craca in sorted(lists):
                if newest(craca, t) is None:
                    continue
                wr = w + f' at {t / NS:.3f} s, CRACA {craca}'
                if not X.check('OF-3', craca in have, wr + ': no list held'):
                    continue
                i, n = have[craca]
                real = [x for x in lists[craca] if (x[1], x[2]) == (i, n)]
                if not X.check('OF-3', bool(real) and real[0][0] <= t,
                               wr + f': holds issueDate {i} nextCrl {n}, not a list issued by then'):
                    continue
                if craca in got:
                    X.check('OF-3', i >= got[craca][1][1], wr + f': holds {i}, older than the {got[craca][1][1]} it fetched')
                floor = newest(craca, t - WEEK * NS)
                X.check('OF-3', floor is None or i >= floor[1],
                        wr + f': holds issueDate {i}; {floor and floor[1]} was issued more than a week earlier')
                cur = newest(craca, t)
                if i < cur[1]:
                    behind += 1
                    max_lag = max(max_lag, (t - cur[0]) // NS)
    X.info['of-3'] = {'devices': len(R.security), 'rows_behind_newest': behind, 'max_lag_s': max_lag,
                      'cracas': len(lists)}


BLOCK_GRACE_NS = 24 * 3600 * NS
PUB_GRACE_NS = 600 * NS


def one_business_day(R, t_ns, strict=False):
    """Deadline of "within 1 business day" ([SM-PROV] 3.4.10) on the provider's Eastern
    calendar; no US federal holiday falls in the runs checked here (2026-10-20 .. 11-04).
    The text does not say how a change outside business hours counts (UNCONFIRMED): the rule
    uses the end of the next business day after the change's day; `strict` gives the same wall
    time one business day later (a Saturday change: Monday at that time)."""
    from zoneinfo import ZoneInfo
    et = ZoneInfo('America/New_York')
    u = (R.t0 + dt.timedelta(microseconds=t_ns // 1000)).astimezone(et)
    d = u + dt.timedelta(days=1)
    while d.weekday() >= 5:
        d += dt.timedelta(days=1)
    if not strict:
        d = d.replace(hour=23, minute=59, second=59, microsecond=0)
    return int((d.astimezone(dt.timezone.utc) - R.t0).total_seconds() * NS)


def rule_publication(X, R, subj_dev):
    """CRL-PUB: a revoked device is on its provider's CRL within one business day of the MA's
    decision ([SM-PROV] 3.4.10), and on no CRL before that decision.  The decision time is the
    engine's proto.revocation `decision` stage; which CRL holds the device is the oracle's own
    match (CRL-MATCH)."""
    judged = late_strict = 0
    for subj, st in R.revstages.items():
        if 'decision' not in st:
            continue
        dev = subj_dev.get(subj)
        if dev is None:
            continue
        td = st['decision']
        first = X.first_crl.get(dev)
        if first is not None:
            X.check('CRL-PUB', first >= td, f'{dev}: on a CRL at {first / NS:.0f} s, before the decision at {td / NS:.0f} s')
        deadline = one_business_day(R, td)
        strict = one_business_day(R, td, strict=True)
        if first is not None and first > strict + PUB_GRACE_NS:
            late_strict += 1
        if deadline + PUB_GRACE_NS <= R.end_t:
            judged += 1
            X.check('CRL-PUB', first is not None and first <= deadline + PUB_GRACE_NS,
                    f'{dev}: decided {td / NS:.0f} s, first on a CRL {first and first / NS}, deadline {deadline / NS:.0f} s')
    X.info['crl-pub'] = {'judged': judged, 'late_under_the_strict_reading': late_strict}


def rule_block(X, R):
    """RA-BLOCK: a device the MA decided to revoke gets no new certificate batch from its RA once
    the RA has blacklisted its enrolment certificate: login refused with HTTP 500 (SCMS-1204) and
    pre-generation stopped (SCMS-509) ([CAMP122]; 03 sec. 8.2, 6.4).  The RA's blacklisting time
    is not on the records, so a day after the decision is allowed for it."""
    subj = {}
    for dev, recs in R.creds.items():
        for r in recs:
            for k8 in r[4]:
                subj[k8] = dev
    dec_t = {}
    unmapped = 0
    for d in R.decisions:
        if d.get('decision') != 'revoke':
            continue
        dev = subj.get(d['subject'])
        if dev is None:
            unmapped += 1
            continue
        # the RA's blacklisting instant when the records give it (proto.revocation
        # `blocklisted`), else the MA's decision record plus a day
        st = R.revstages.get(d['subject'], {})
        tb = st['blocklisted'] - BLOCK_GRACE_NS + 60 * NS if 'blocklisted' in st else d['t']
        dec_t[dev] = min(dec_t.get(dev, tb), tb)
    late = collections.defaultdict(list)
    for p in R.pdus:
        if p['msg'] == 'batch-download' and p['to'] in dec_t and p['t'] > dec_t[p['to']] + BLOCK_GRACE_NS:
            late[p['to']].append(p['t'])
    for dev, t in dec_t.items():
        v = late.get(dev, [])
        X.check('RA-BLOCK', not v, f'{dev}: {len(v)} certificates delivered from {min(v) / NS if v else 0:.0f} s, '
                                   f'decision at {t / NS:.0f} s')
    tried = sum(1 for p in R.pdus if p['type'] == 'EeRaDownloadRequestSpdu' and p['msg'] != 'report'
                and p['from'] in dec_t and p['t'] > dec_t[p['from']] + BLOCK_GRACE_NS)
    X.info['ra-block'] = {'revoked_devices': len(dec_t), 'unmapped_decisions': unmapped,
                          'download_requests_after_block': tried}


def rule_enrolment(X, store, by_type):
    """device -> enrolment certificate (from the ECA response to that device's request)."""
    req_dev = {}
    for p, _, _ in by_type['EeEcaCertRequestSpdu']:
        req_dev[prims.hashed_id(bytes.fromhex(p['hex']), 8)] = p['from']
    out = {}
    for p, outer, _ in by_type['EcaEeCertResponseSpdu']:
        sd = signed(outer)
        resp = dec('ScmsPdu', payload_bytes(sd))['content'][1][1]
        for c in resp.get('ecaCertChain') or []:
            store.add(c)
        dev = req_dev.get(resp['requestHash'])
        X.check('ENR-MAP', dev is not None, f'{p["id"]}: requestHash {resp["requestHash"].hex()} names no captured request')
        sc, _ = signer_cert(store, sd)
        ok, why = verify_sd(store, sd, sc) if sc else (False, 'no signer')
        X.check('ENR-MAP', ok, f'{p["id"]}: ECA {why}')
        cert = resp['certificate']
        store.add(cert)
        if dev:
            out.setdefault(dev, []).append(h8(cert))
    return out


def rule_acks(X, store, by_type):
    """device -> list of (requestHash, firstI, nextDlTime)."""
    req = {}
    butterfly = {}
    for p, _, opened in by_type['EeRaCertRequestSpdu']:
        h = prims.hashed_id(bytes.fromhex(p['hex']), 8)
        req.setdefault(p['from'], set()).add(h)
        try:
            scr = dec('SignedCertificateRequest', opened['content'][1])
            body = scr['tbsRequest']['content'][1][1]
            butterfly[h] = 'additionalParams' in body and body['additionalParams'][0] == 'original'
        except Exception as e:
            X.check('DL-ACK', False, f'{p["id"]}: EeRaCertRequest does not open: {e}')
    out = collections.defaultdict(list)
    for p, outer, _ in by_type['RaEeCertAckSpdu']:
        sd = signed(outer)
        w = f'{p["id"]} to {p["to"]}'
        if not X.check('DL-ACK', sd is not None, w + ': not signedData'):
            continue
        hi = sd['tbsData']['headerInfo']
        X.check('DL-ACK', hi.get('psid') == SECMGMT_PSID and set(hi) == {'psid'}, w + f': headerInfo {sorted(hi)}')
        sc, n = signer_cert(store, sd)
        X.check('DL-ACK', sc is not None and n == 1, w + ': signer is not SignerSingleCert')
        role = mgmt_role(sc) if sc else None
        X.check('DL-ACK', role is not None and role[0] == 'ra', w + f': signer role {role and role[0]}')
        if sc:
            ok, why = verify_sd(store, sd, sc)
            X.check('DL-ACK', ok, w + ': ' + why)
        a = dec('ScmsPdu', payload_bytes(sd))['content'][1][1]
        X.check('DL-ACK', a['requestHash'] in req.get(p['to'], set()),
                w + f': requestHash {a["requestHash"].hex()} is not the HashedId8 of a request {p["to"]} sent')
        bf = butterfly.get(a['requestHash'])
        if bf:
            # "shall be present ... if the corresponding EeRaCertRequest uses the butterfly key
            # mechanism" (EeRaInterface RaEeCertAck.firstI); nothing is said for other requests
            X.check('DL-ACK', 'firstI' in a, w + ': firstI absent for a butterfly request')
        X.check('DL-ACK', a['nextDlTime'] >= a['generationTime'], w + ': nextDlTime before generationTime')
        out[p['to']].append((a['requestHash'], a.get('firstI'), a['nextDlTime']))
    return out


def _open_download(X, store, p, outer, opened, rule='DL-ENC'):
    w = f'{p["id"]} from {p["from"]}'
    ok = X.check(rule, outer['content'][0] == 'encryptedData', w + ': outer not encryptedData')
    if ok:
        rec = outer['content'][1]['recipients']
        X.check(rule, len(rec) == 1 and rec[0][0] == 'certRecipInfo', w + ': not one certRecipInfo')
        rc = store.get(rec[0][1]['recipientId']) if rec[0][0] == 'certRecipInfo' else None
        role = mgmt_role(rc) if rc else None
        X.check(rule, role is not None and role[0] == 'ra', w + ': recipient is not an RA certificate in the store')
    sd = signed(opened) if opened else None
    if not X.check(rule, sd is not None, w + ': opened PDU is not signedData'):
        return None, None
    hi = sd['tbsData']['headerInfo']
    X.check(rule, hi.get('psid') == SECMGMT_PSID and set(hi) == {'psid'}, w + f': headerInfo {sorted(hi)}')
    sc, n = signer_cert(store, sd)
    X.check(rule, sc is not None and n == 1, w + ': signer is not SignerSingleCert')
    body = dec('ScmsPdu', payload_bytes(sd))['content'][1]
    X.check(rule, body[0] == 'eeRaDownloadRequest', w + f': inner {body[0]}')
    return sd, (sc, body[1])


def rule_downloads(X, R, store, by_type, enrol, acks):
    polls = collections.defaultdict(list)
    for p, outer, opened in by_type['EeRaDownloadRequestSpdu']:
        sd, got = _open_download(X, store, p, outer, opened)
        if got is None:
            continue
        sc, req = got
        w = f'{p["id"]} from {p["from"]} ({p["msg"]})'
        dev = p['from']
        mine = enrol.get(dev, [])
        X.check('DL-SIGN', sc is not None and h8(sc) in mine,
                w + f': signer {sc and h8(sc).hex()} is not {dev}\'s enrolment certificate {[m.hex() for m in mine]}')
        if sc:
            ok, why = verify_sd(store, sd, sc)
            X.check('DL-SIGN', ok, w + ': ' + why)
        if p['flow'] != 'preload':
            now = t32(R, p['t'])
            X.check('DL-TIME', abs(req['generationTime'] - now) <= CLOCK_TOL_S,
                    w + f': generationTime {req["generationTime"]} vs send {now}')
        else:
            X.check('DL-TIME', req['generationTime'] <= t32(R, 0) + CLOCK_TOL_S, w + ': preload request after t0')
        fn = req['filename']
        hashes = {a[0].hex().upper(): a for a in acks.get(dev, [])}
        if p['msg'] == 'report':
            X.check('DL-NAME', fn == 'misbehavior-report', w + f': report upload filename {fn!r}')
            continue
        m, mi = FN_BATCH.fullmatch(fn), FN_INFO.fullmatch(fn)
        if not X.check('DL-NAME', m or mi, w + f': filename {fn!r} matches neither form'):
            continue
        h = (m or mi).group(1)
        a = hashes.get(h)
        X.check('DL-NAME', a is not None, w + f': {fn} names no requestHash acknowledged to {dev}')
        if m and a is not None and a[1] is not None:
            X.check('DL-NAME', int(m.group(2), 16) >= a[1], w + f': i {int(m.group(2), 16)} < firstI {a[1]}')
        if a is not None and p['flow'] != 'preload':
            # nextDlTime: "the time after which the EE should connect to the RA to download"
            X.check('DL-AFTER', req['generationTime'] >= a[2] - CLOCK_TOL_S,
                    w + f': requested at {req["generationTime"]}, before nextDlTime {a[2]}')
        polls[(dev, fn)].append(p['t'])
    for (dev, fn), ts in polls.items():
        ts = sorted(set(ts))
        X.check('DL-WAIT', len(ts) <= MAX_REQS, f'{dev} {fn}: {len(ts)} requests > {MAX_REQS}')
        for a, b in zip(ts, ts[1:]):
            X.check('DL-WAIT', (b - a) >= MIN_WAIT_S * NS, f'{dev} {fn}: requested again after {(b - a) / NS:.1f} s')


def rule_reports(X, R, store, by_type):
    uploads = {}
    forwards = collections.Counter()
    up_msgs = collections.defaultdict(set)
    for p, outer, opened in by_type['SecuredMisbehaviorReport']:
        if p['msg'] == 'report' and p['from'].startswith('ee/'):
            up_msgs[p['instance']].add('report')
            uploads[p['hex']] = p
        elif p['msg'] in ('report-forward',):
            forwards[p['hex']] += 1
            continue
        else:
            continue
        w = f'{p["id"]} from {p["from"]}'
        ok = X.check('MBR-ENC', outer['content'][0] == 'encryptedData', w + ': not encryptedData')
        if ok:
            rec = outer['content'][1]['recipients']
            rc = store.get(rec[0][1]['recipientId']) if len(rec) == 1 and rec[0][0] == 'certRecipInfo' else None
            role = mgmt_role(rc) if rc else None
            X.check('MBR-ENC', role is not None and role[0] == 'ma' and BSM_PSID in role[1].get('relevantPsids', []),
                    w + f': recipient is not an MA certificate for 0x20 ({role and role[0]})')
        sd = signed(opened) if opened else None
        if not X.check('MBR-SIG', sd is not None, w + ': opened report is not signedData'):
            continue
        hi = sd['tbsData']['headerInfo']
        X.check('MBR-SIG', hi.get('psid') == MBR_PSID and set(hi) == {'psid', 'generationTime'},
                w + f': headerInfo psid {hi.get("psid")} fields {sorted(hi)}')
        sc, n = signer_cert(store, sd)
        if not X.check('MBR-SIG', sc is not None and sd['signer'][0] == 'certificate' and n == 1,
                       w + ': signer is not one full certificate'):
            continue
        tb = sc['toBeSigned']
        X.check('MBR-SIG', tb['id'][0] == 'linkageData', w + f': signer id {tb["id"][0]} is not a pseudonym')
        X.check('MBR-SIG', ssp_of(sc, MBR_PSID) is not False, w + ': signer appPermissions lack psid 0x26')
        start, end = chain.validity(tb)
        g = hi.get('generationTime', 0) // 1_000_000
        X.check('MBR-SIG', start <= g < end, w + f': generationTime {g} outside [{start}, {end})')
        X.check('MBR-SIG', g <= t32(R, p['t']) + CLOCK_TOL_S, w + ': generationTime after the upload')
        ok, why = verify_sd(store, sd, sc)
        X.check('MBR-SIG', ok, w + ': ' + why)
    # uploads carry a Download-Req; forwards are byte-identical and at most once
    for p, _, _ in by_type['EeRaDownloadRequestSpdu']:
        if p['msg'] == 'report':
            up_msgs[p['instance']].add('dl')
    for inst, s in up_msgs.items():
        X.check('MBR-UP', s == {'report', 'dl'}, f'{inst}: upload carries {sorted(s)}')
    # every cellular upload by a device carries its bytes (RSU-filed backhaul reports are a
    # separate, labelled path)
    for mid, frm, tr, has, inst, t, nbytes in R.reports:
        if tr == 'cellular-uu':
            X.check('MBR-UP', has, f'{mid} {frm} {inst}: cellular report upload ({nbytes} B) without its SPDUs')
    for hx, n in forwards.items():
        X.check('MBR-UP', hx in uploads, 'a forwarded report is no EE upload byte for byte')
        X.check('MBR-UP', n == 1, f'a report forwarded {n} times')


def rule_crl(X, R, store, by_type):
    first = {}
    for p, outer, _ in by_type['SecuredCrl']:
        if p['hex'] not in first or p['t'] < first[p['hex']][0]['t']:
            first[p['hex']] = (p, outer)
    # CRLs the engine records as issued (pki.state crls[].hex) but never sends on a pki.flow
    # message: they still count in the (crlCraca, crlSeries) sequence (indexWithinI counts
    # every full or delta CRL issued)
    unsent = 0
    for (prov, id8), row in R.crl_rows.items():
        hx = row.get('hex')
        if hx and hx not in first:
            unsent += 1
            p = {'id': f'{prov}/state-crl/{id8}', 'provider': prov, 't': row.get('issued_at', row['seen_t']),
                 'type': 'SecuredCrl', 'msg': None, 'unsent': True}
            first[hx] = (p, dec('Ieee1609Dot2Data', bytes.fromhex(hx)))
    X.info['crls'] = {'issued': len(first), 'never_sent': unsent}
    crls = []
    for hx, (p, outer) in first.items():
        w = f'{p["id"]} ({p["provider"]})'
        sd = signed(outer)
        if not X.check('CRL-ENV', sd is not None, w + ': not signedData'):
            continue
        hi = sd['tbsData']['headerInfo']
        X.check('CRL-ENV', hi.get('psid') == CRL_PSID and set(hi) == {'psid'}, w + f': headerInfo {hi}')
        cc = dec('CrlContents', payload_bytes(sd))
        X.check('CRL-ENV', cc['version'] == 1, w + ': version')
        sc, _ = signer_cert(store, sd)
        if not X.check('CRL-ENV', sc is not None, w + ': signer unresolved'):
            continue
        ok, why = verify_sd(store, sd, sc)
        X.check('CRL-ENV', ok, w + ': ' + why)
        crls.append({'p': p, 'sd': sd, 'cc': cc, 'signer': sc, 'hex': hx})
    by_key = collections.defaultdict(list)
    for c in crls:
        cc, sc = c['cc'], c['signer']
        w = f'{c["p"]["id"]}'
        s = ssp_of(sc, CRL_PSID)
        ssp = dec('CrlSsp', s) if isinstance(s, bytes) else None
        if X.check('CRL-SSP', ssp is not None, w + ': signer has no CrlSsp for psid 256'):
            X.check('CRL-SSP', cc['crlSeries'] in ssp['crls'], w + f': series {cc["crlSeries"]} not in {ssp["crls"]}')
            craca = sc if ssp['associatedCraca'] == 'isCraca' else store.issuer(sc)
            X.check('CRL-SSP', craca is not None and cc['crlCraca'] == h8(craca),
                    w + f': crlCraca {cc["crlCraca"].hex()} vs {ssp["associatedCraca"]} '
                        f'{craca and h8(craca).hex()}')
            c['craca'] = craca
        X.check('CRL-NEXT', cc['nextCrl'] > cc['issueDate'], w + ': nextCrl not after issueDate')
        X.check('CRL-TIME', cc['issueDate'] <= t32(R, c['p']['t']) + 1, w + ': issueDate after the CRL was sent')
        kind, body = cc['typeSpecific']
        if 'Linked' in kind:
            X.check('CRL-IREV', body['iRev'] <= i_at_time32(cc['issueDate']),
                    w + f': iRev {body["iRev"]} > i {i_at_time32(cc["issueDate"])} at issueDate')
        by_key[(cc['crlCraca'], cc['crlSeries'])].append(c)
    for key, lst in by_key.items():
        def order(c):
            k, b = c['cc']['typeSpecific']
            return (c['cc']['issueDate'], b.get('iRev', 0), b.get('indexWithinI', b.get('crlSerial', 0)), c['p']['t'])
        lst.sort(key=order)
        prev_full = None
        for a, b in zip(lst, lst[1:]):
            ca, cb = a['cc'], b['cc']
            w = f'{key[0].hex()}/{key[1]}: {a["p"]["id"]} -> {b["p"]["id"]}'
            # "the time when the CRL was issued": a full and a delta CRL issued together share it
            X.check('CRL-SEQ', cb['issueDate'] >= ca['issueDate'], w + ': issueDate decreases')
            ka, ba = ca['typeSpecific']
            kb, bb = cb['typeSpecific']
            if 'Hash' in ka and 'Hash' in kb:
                X.check('CRL-SEQ', bb['crlSerial'] == ba['crlSerial'] + 1, w + f': crlSerial {ba["crlSerial"]} -> {bb["crlSerial"]}')
            if 'Linked' in ka and 'Linked' in kb:
                X.check('CRL-SEQ', bb['iRev'] >= ba['iRev'], w + ': iRev decreases')
                want = ba['indexWithinI'] + 1 if bb['iRev'] == ba['iRev'] else 0
                X.check('CRL-SEQ', bb['indexWithinI'] == want, w + f': indexWithinI {bb["indexWithinI"]}, want {want}')
            if ka.startswith('full'):
                prev_full = a
            if kb.startswith('full') and prev_full is not None:
                X.check('CRL-KEEP', _kept(prev_full['cc'], cb),
                        f'{key[0].hex()}/{key[1]}: {prev_full["p"]["id"]} -> {b["p"]["id"]}: an unexpired entry is missing')
            if cb['issueDate'] > ca['issueDate']:
                X.check('CRL-SLA', cb['issueDate'] <= ca['nextCrl'], w + f': issued {cb["issueDate"] - ca["nextCrl"]} s after nextCrl')
        last = lst[-1]['cc']
        if t32(R, R.end_t) > last['nextCrl']:
            X.check('CRL-SLA', False, f'{key[0].hex()}/{key[1]}: no CRL after nextCrl {last["nextCrl"]} (run ends {t32(R, R.end_t)})')
    # the engine's CRL rows vs the bytes
    by_id8 = {prims.hashed_id(bytes.fromhex(c['hex']), 8).hex(): c for c in crls}
    for (prov, id8), row in R.crl_rows.items():
        c = by_id8.get(id8)
        if not X.check('CRL-STATE', c is not None, f'{prov} {id8}: no captured SecuredCrl with that HashedId8'):
            continue
        cc = c['cc']
        k, _ = cc['typeSpecific']
        X.check('CRL-STATE', row.get('issue_date') == cc['issueDate'] and row.get('next_crl') == cc['nextCrl']
                and row.get('series') == cc['crlSeries'] and row.get('delta') == k.startswith('delta')
                and row.get('entries') == _entries(cc),
                f'{prov} {id8}: row {({k2: row.get(k2) for k2 in ("issue_date", "next_crl", "series", "delta", "entries")})} '
                f'vs bytes {cc["issueDate"], cc["nextCrl"], cc["crlSeries"], k, _entries(cc)}')
        if 'hex' in row:
            X.check('CRL-STATE', row['hex'] == c['hex'], f'{prov} {id8}: row hex differs from the captured bytes')
    return crls


def _entries(cc):
    k, b = cc['typeSpecific']
    if 'Hash' in k:
        return len(b['entries'])
    n = 0
    for jg in b.get('individual', []):
        for lg in jg['contents']:
            for ig in lg['contents']:
                n += len(ig['contents'])
    return n


def _lin_set(b):
    out = set()
    for jg in b.get('individual', []):
        for lg in jg['contents']:
            for ig in lg['contents']:
                for r in ig['contents']:
                    out.add((lg['la1Id'], lg['la2Id'], ig['iMax'], r['linkageSeed1'], r['linkageSeed2']))
    return out


def _kept(ca, cb):
    ka, ba = ca['typeSpecific']
    kb, bb = cb['typeSpecific']
    if 'Hash' in ka:
        want = {e['id'] for e in ba['entries'] if e['expiry'] > cb['issueDate']}
        return want <= {e['id'] for e in bb['entries']}
    i_b = bb['iRev']
    need = [e for e in _lin_set(ba) if e[2] >= i_b]
    if not need:
        return True
    from . import linkage
    have = _lin_set(bb)
    steps = bb['iRev'] - ba['iRev']
    # seeds at the later iRev: advance the earlier entry's seeds (1609.2 5.1.3.4)
    have_adv = {(e[0], e[1], e[3], e[4]) for e in have}
    for la1, la2, imax, s1, s2 in need:
        n1 = linkage.seed_at(int.from_bytes(la1, 'big'), s1, steps)
        n2 = linkage.seed_at(int.from_bytes(la2, 'big'), s2, steps)
        if (la1, la2, n1, n2) not in have_adv:
            return False
    return True


def rule_craca(X, R, store, crls):
    # CRACAs: for every CRL signer certificate in the store, (HashedId3 of its CRACA, series)
    covered = set()
    for c in list(store.by.values()):
        s = ssp_of(c, CRL_PSID)
        if not isinstance(s, bytes):
            continue
        try:
            ssp = dec('CrlSsp', s)
        except Exception:
            continue
        craca = c if ssp['associatedCraca'] == 'isCraca' else store.issuer(c)
        if craca is None:
            continue
        for series in ssp['crls']:
            covered.add((h8(craca)[-3:], series))
    for h, c in store.by.items():
        tb = c['toBeSigned']
        if c['issuer'][0] == 'self':
            continue
        if tb['id'][0] == 'linkageData':
            want, kind = 1, 'pseudonym'
        elif mgmt_role(c) is not None:
            continue          # SCMS components: published artefacts (series checked by `real`)
        elif tb.get('certRequestPermissions') and not tb.get('appPermissions'):
            want, kind = 4, 'enrolment'
        elif tb.get('appPermissions'):
            want, kind = 3, 'application/identification'
        else:
            continue
        w = f'{kind} {h.hex()} ({tb["id"][1] if tb["id"][0] == "name" else tb["id"][0]})'
        X.check('CRACA-CERT', tb['crlSeries'] == want, w + f': crlSeries {tb["crlSeries"]}, want {want}')
        if kind == 'enrolment':
            # 6-year enrolment certificate ([CAMP122] Table 3, SCMS-1307; owner decision 10)
            X.check('ENR-LIFE', tb['validityPeriod']['duration'] in (('hours', 52560), ('years', 6)),
                    w + f': validity {tb["validityPeriod"]["duration"]}, want 6 years')
        X.check('CRACA-CERT', (tb['cracaId'], tb['crlSeries']) in covered,
                w + f': no CRL signer for cracaId {tb["cracaId"].hex()} series {tb["crlSeries"]}')
    # CRL-MATCH: each linkage entry of the newest full linkage CRL of each (crlCraca, series)
    # against every pseudonym the devices recorded; matching needs cracaId/crlSeries agreement
    last = {}
    for c in crls:
        k = (c['cc']['crlCraca'], c['cc']['crlSeries'])
        if c['cc']['typeSpecific'][0] == 'fullLinkedCrl':
            if k not in last or c['cc']['issueDate'] > last[k]['cc']['issueDate']:
                last[k] = c
    if not last:
        return
    dev_certs = {}
    for dev, recs in R.creds.items():
        ids = set()
        for r in recs:
            ids |= set(r[4])
        cs = []
        for k8 in ids:
            hx = R.cert_hex.get(k8)
            if hx is None:
                continue
            cert = dec('Certificate', bytes.fromhex(hx))
            if cert['toBeSigned']['id'][0] == 'linkageData':
                cs.append((k8, cert))
        dev_certs[dev] = cs
    subj_dev = {k8: dev for dev, cs in dev_certs.items() for k8, _ in cs}
    decided = {subj_dev.get(d['subject'], d['subject']) for d in R.decisions if d.get('decision') == 'revoke'}
    # An entry is followed through every full linkage CRL of its (crlCraca, series): its seeds,
    # advanced to the newest CRL's iRev, identify it.  A device can only be matched by a CRL
    # whose iRev is not after the device's recorded i-periods (seeds only advance), so an entry
    # passes when some appearance revokes exactly one recorded device; it fails when an
    # appearance revokes two or more, or when no appearance can be tied to any device.
    from . import linkage
    matched = collections.defaultdict(set)
    for key, newest in last.items():
        i_ref = newest['cc']['typeSpecific'][1]['iRev']
        hits = collections.defaultdict(list)
        for c in crls:
            cc = c['cc']
            if (cc['crlCraca'], cc['crlSeries']) != key or cc['typeSpecific'][0] != 'fullLinkedCrl':
                continue
            body = cc['typeSpecific'][1]
            for jg in body.get('individual', []):
                for lg in jg['contents']:
                    la1, la2 = int.from_bytes(lg['la1Id'], 'big'), int.from_bytes(lg['la2Id'], 'big')
                    for ig in lg['contents']:
                        for rev in ig['contents']:
                            steps = i_ref - body['iRev']
                            ident = (lg['la1Id'], lg['la2Id'], linkage.seed_at(la1, rev['linkageSeed1'], steps),
                                     linkage.seed_at(la2, rev['linkageSeed2'], steps))
                            one = dict(cc, typeSpecific=('fullLinkedCrl', dict(body, individual=[
                                {'jmax': jg['jmax'], 'contents': [dict(lg, contents=[dict(ig, contents=[rev])])]}])))
                            hit = {dev for dev, cs in dev_certs.items()
                                   if any(chain.linkage_crl_revokes(one, x) for _, x in cs)}
                            hits[ident].append((c['p']['id'], hit, c['p']['t']))
        for ident, apps in hits.items():
            tag = f'{key[0].hex()}/{key[1]} entry {ident[2].hex()[:8]} ({len(apps)} CRLs)'
            many = [(cid, h) for cid, h, _ in apps if len(h) > 1]
            X.check('CRL-MATCH', not many, f'{tag}: {many[:1]} revokes several devices')
            devs = set().union(*(h for _, h, _ in apps))
            X.check('CRL-MATCH', len(devs) == 1, f'{tag}: tied to {len(devs)} recorded devices {sorted(devs)[:4]}')
            first_t = min(t for _, _, t in apps)
            for dev in devs:
                matched[dev].add(tag)
                X.first_crl[dev] = min(X.first_crl.get(dev, first_t), first_t)
    rule_publication(X, R, subj_dev)
    for dev in matched:
        X.check('CRL-MATCH', dev in decided, f'{dev}: revoked by a CRL but the MA made no revoke decision on it')
    X.info['crl-match'] = {'decided': len(decided), 'matched': len(matched),
                           'entries': sum(_entries(c['cc']) for c in last.values()),
                           'flagged_revoked': sum(any(r[3] for r in recs) for recs in R.creds.values())}


def rule_ctl(X, R):
    if not R.ctl:
        return
    hist = R.ctl[-1][2]['history'] or []
    seqs = [h['sequence'] for h in hist]
    X.check('CTL-SUM', seqs == list(range(seqs[0], seqs[0] + len(seqs))), f'history sequences {seqs}')
    q = R.ctl[-1][2]['quorum']
    for h in hist:
        X.check('CTL-SUM', len(h['signers']) >= q, f'CTL {h["sequence"]} signed by {len(h["signers"])} < quorum {q}')
    for a, b in zip(hist, hist[1:]):
        X.check('CTL-SUM', b['effective'] > a['effective'], f'CTL {b["sequence"]} effective not after {a["sequence"]}')
    prev = None
    for t, v, s in R.ctl:
        if prev is not None and s['sequence'] != prev:
            X.check('CTL-SUM', (v or t32(R, t)) >= s['effective'],
                    f'CTL {s["sequence"]} in force at {v} before its effectiveDate {s["effective"]}')
        prev = s['sequence']
    pub = [(t, s['sequence']) for t, _, s in R.ctl]

    def published_at(t):
        cur = None
        for tt, sq in pub:
            if tt <= t:
                cur = sq
        return cur

    for dev, recs in R.creds.items():
        seq = [r[1] for r in recs if r[1] is not None]
        X.check('CTL-DEV', seq == sorted(seq), f'{dev}: CTL sequence decreases {seq}')
        for r in recs:
            p = published_at(r[0])
            if r[1] is not None and p is not None:
                X.check('CTL-DEV', r[1] <= p, f'{dev}: holds CTL {r[1]} at t={r[0]} before it is published ({p})')


# --------------------------------------------------------------------------------------------
# faults injected into the loaded records: (description, rule that must go red, mutator)

def _first(R, typ, pred=lambda p: True):
    return next((p for p in R.pdus if p['type'] == typ and pred(p)), None)


def _reencode_crl(p, fn):
    d = coer.decode('Ieee1609Dot2Data', bytes.fromhex(p['hex']))
    sd = d['content'][1]
    cc = coer.decode('CrlContents', payload_bytes(sd))
    fn(cc)
    sd['tbsData']['payload']['data']['content'] = ('unsecuredData', coer.encode('CrlContents', cc))
    new = coer.encode('Ieee1609Dot2Data', d).hex()
    old = p['hex']
    return old, new


def _mut_crl(R, fn, nth=1, prev_entries=False):
    """Rewrite the nth distinct CRL of the first (craca, series) that has nth+1 CRLs.
    CRLs the engine issued but never sent (pki.state rows only, such as the t0 CRLs every
    CRACA publishes) are in the sequence too, as rule_crl counts them. With prev_entries the
    earlier CRL must hold an entry (a dropped entry needs one to drop)."""
    seen = collections.OrderedDict()
    for p in R.pdus:
        if p['type'] == 'SecuredCrl':
            seen.setdefault(p['hex'], p)
    for row in R.crl_rows.values():
        hx = row.get('hex')
        if hx and hx not in seen:
            seen[hx] = {'hex': hx, 't': row.get('issued_at', row['seen_t'])}
    groups = collections.defaultdict(list)
    for hx, p in seen.items():
        cc = coer.decode('CrlContents', payload_bytes(coer.decode('Ieee1609Dot2Data', bytes.fromhex(hx))['content'][1]))
        groups[(cc['crlCraca'], cc['crlSeries'])].append((cc['issueDate'], p['t'], hx, cc))
    for g in groups.values():
        g.sort(key=lambda x: (x[0], x[1]))
        # with prev_entries, the first later CRL whose predecessor holds an entry
        at = next((k for k in range(nth, len(g)) if not prev_entries or _entries(g[k - 1][3])), None)
        if at is not None:
            prev = g[at - 1][3] if at else None
            old, new = _reencode_crl(seen[g[at][2]], lambda cc: fn(cc, prev))
            for p in R.pdus:
                if p['hex'] == old:
                    p['hex'] = new
            for row in R.crl_rows.values():
                if row.get('hex') == old:
                    row['hex'] = new
            return True
    return False


def _mut_pdu_inner(R, typ, fn, pred=lambda p: True, field='opened'):
    p = _first(R, typ, lambda p: p.get(field) and pred(p))
    if p is None:
        return False
    d = coer.decode('Ieee1609Dot2Data', bytes.fromhex(p[field]))
    fn(d, p)
    p[field] = coer.encode('Ieee1609Dot2Data', d).hex()
    return True


def _set_dl(field, value_fn, pred=lambda p: p['msg'] != 'report' and p['flow'] != 'preload'):
    def m(R):
        def fn(d, p):
            sd = d['content'][1]
            pdu = coer.decode('ScmsPdu', payload_bytes(sd))
            body = pdu['content'][1][1]
            body[field] = value_fn(body[field], p)
            sd['tbsData']['payload']['data']['content'] = ('unsecuredData', coer.encode('ScmsPdu', pdu))
        return _mut_pdu_inner(R, 'EeRaDownloadRequestSpdu', fn, pred)
    return m


def _mut_dl_poll_fast(R):
    p = _first(R, 'EeRaDownloadRequestSpdu', lambda p: p['msg'] != 'report' and p['flow'] != 'preload')
    if p is None:
        return False
    q = dict(p, id=p['id'] + '-dup', t=p['t'] + 60 * NS)
    d = coer.decode('Ieee1609Dot2Data', bytes.fromhex(q['opened']))
    sd = d['content'][1]
    pdu = coer.decode('ScmsPdu', payload_bytes(sd))
    pdu['content'][1][1]['generationTime'] += 60
    sd['tbsData']['payload']['data']['content'] = ('unsecuredData', coer.encode('ScmsPdu', pdu))
    q['opened'] = coer.encode('Ieee1609Dot2Data', d).hex()
    R.pdus.append(q)
    return True


def _mut_sig(typ, field, pred=lambda p: True):
    def m(R):
        def fn(d, p):
            sig = d['content'][1]['signature']
            s = sig[1]
            s['sSig'] = bytes([s['sSig'][0] ^ 1]) + s['sSig'][1:]
        return _mut_pdu_inner(R, typ, fn, pred, field)
    return m


def _mut_mbr(fn):
    def m(R):
        def f(d, p):
            fn(d['content'][1])
        return _mut_pdu_inner(R, 'SecuredMisbehaviorReport', f, lambda p: p['msg'] == 'report')
    return m


def _mut_mbr_recipient(R):
    def f(d, p):
        rec = d['content'][1]['recipients'][0][1]
        rec['recipientId'] = bytes(8)
    return _mut_pdu_inner(R, 'SecuredMisbehaviorReport', f, lambda p: p['msg'] == 'report', 'hex')


def _mut_forward(R):
    p = _first(R, 'SecuredMisbehaviorReport', lambda p: p['msg'] == 'report-forward')
    if p is None:
        return False
    p['hex'] = p['hex'][:-2] + ('00' if p['hex'][-2:] != '00' else '01')
    return True


def _mut_cert(fn):
    def m(R):
        for k, hx in R.cert_hex.items():
            c = coer.decode('Certificate', bytes.fromhex(hx))
            if c['toBeSigned']['id'][0] == 'linkageData':
                fn(c['toBeSigned'])
                R.cert_hex[k] = coer.encode('Certificate', c).hex()
                return True
        return False
    return m


def _mut_ctl(fn):
    def m(R):
        if not R.ctl:
            return False
        R.ctl = copy.deepcopy(R.ctl)
        return fn(R.ctl) is not False
    return m


def _ctl_gap(ctl):
    h = ctl[-1][2]['history']
    if len(h) < 2:
        return False
    h[-1]['sequence'] += 1


def _ctl_quorum(ctl):
    ctl[-1][2]['history'][-1]['signers'] = [0]


def _ctl_early(ctl):
    for n in range(1, len(ctl)):
        if ctl[n][2]['sequence'] != ctl[n - 1][2]['sequence']:
            ctl[n] = (ctl[n][0], ctl[n][2]['effective'] - 100, ctl[n][2])
            return
    return False


def _mut_dev_ahead(R):
    for dev, recs in R.creds.items():
        if recs:
            t, s, prov, rv, ids = recs[0]
            recs[0] = (t, (s or 0) + 5, prov, rv, ids)
            return True
    return False


def _mut_time(R):
    if not R.times:
        return False
    t, v = R.times[0]
    R.times[0] = (t, v - 5)
    return True


def _mut_crl_row(R):
    for k, row in R.crl_rows.items():
        row['issue_date'] = row.get('issue_date', 0) + 1
        return True
    return False


def _mut_cert_id(R):
    for k in list(R.cert_hex):
        R.cert_hex['00' * 8] = R.cert_hex.pop(k)
        return True
    return False


def _mut_ack_hash(R):
    def fn(d, p):
        sd = d['content'][1]
        pdu = coer.decode('ScmsPdu', payload_bytes(sd))
        a = pdu['content'][1][1]
        a['requestHash'] = bytes(8)
        sd['tbsData']['payload']['data']['content'] = ('unsecuredData', coer.encode('ScmsPdu', pdu))
    return _mut_pdu_inner(R, 'RaEeCertAckSpdu', fn, field='hex')


def _mut_enrol_swap(R):
    """The Download-Req of one device signed with another device's enrolment certificate."""
    reqs = [p for p in R.pdus if p['type'] == 'EeRaDownloadRequestSpdu' and p.get('opened')]
    devs = {}
    for p in reqs:
        devs.setdefault(p['from'], p)
    if len(devs) < 2:
        return False
    a, b = list(devs.values())[:2]
    a['opened'] = b['opened']
    return True


def _mbr_psid(sd):
    sd['tbsData']['headerInfo']['psid'] = BSM_PSID


def _mbr_late(sd):
    sd['tbsData']['headerInfo']['generationTime'] += 400 * 24 * 3600 * 1_000_000


def _crl_index(cc, prev):
    k, b = cc['typeSpecific']
    if 'Linked' not in k:
        b['crlSerial'] += 5
    else:
        b['indexWithinI'] += 3


def _crl_back(cc, prev):
    cc['issueDate'] = prev['issueDate'] - 1 if prev else cc['issueDate']


def _crl_next(cc, prev):
    cc['nextCrl'] = cc['issueDate']


def _crl_irev(cc, prev):
    k, b = cc['typeSpecific']
    if 'Linked' in k:
        b['iRev'] = i_at_time32(cc['issueDate']) + 1


def _crl_drop(cc, prev):
    k, b = cc['typeSpecific']
    if 'Linked' in k:
        for jg in b.get('individual', []):
            for lg in jg['contents']:
                for ig in lg['contents']:
                    if ig['contents']:
                        ig['contents'] = ig['contents'][1:]
                        return
    else:
        b['entries'] = b['entries'][1:]


def _crl_craca(cc, prev):
    cc['crlCraca'] = bytes(8)


def _crl_series(cc, prev):
    cc['crlSeries'] = 77


def _mut_crl_sig(R):
    p = _first(R, 'SecuredCrl')
    if p is None:
        return False
    d = coer.decode('Ieee1609Dot2Data', bytes.fromhex(p['hex']))
    s = d['content'][1]['signature'][1]
    s['sSig'] = bytes([s['sSig'][0] ^ 1]) + s['sSig'][1:]
    old = p['hex']
    new = coer.encode('Ieee1609Dot2Data', d).hex()
    for q in R.pdus:
        if q['hex'] == old:
            q['hex'] = new
    return True


def _mut_crl_late(R):
    """Pretend the run lasted past the last CRL's nextCrl with no new CRL."""
    R.end_t += 60 * 24 * 3600 * NS
    return True


def _mut_block(R):
    subj = {k8: dev for dev, recs in R.creds.items() for r in recs for k8 in r[4]}
    d = next((d for d in R.decisions if d.get('decision') == 'revoke' and d['subject'] in subj), None)
    p = _first(R, 'AcaEeCertResponsePrivateSpdu', lambda p: p['msg'] == 'batch-download')
    if d is None or p is None:
        return False
    R.pdus.append(dict(p, id=p['id'] + '-late', to=subj[d['subject']], t=d['t'] + 2 * BLOCK_GRACE_NS))
    return True


def _mut_ack_next(R):
    def fn(d, p):
        sd = d['content'][1]
        pdu = coer.decode('ScmsPdu', payload_bytes(sd))
        pdu['content'][1][1]['nextDlTime'] += 30 * 24 * 3600
        sd['tbsData']['payload']['data']['content'] = ('unsecuredData', coer.encode('ScmsPdu', pdu))
    n = 0
    for p in R.pdus:          # every acknowledgement: the downloads now all come too early
        if p['type'] == 'RaEeCertAckSpdu':
            d = coer.decode('Ieee1609Dot2Data', bytes.fromhex(p['hex']))
            fn(d, p)
            p['hex'] = coer.encode('Ieee1609Dot2Data', d).hex()
            n += 1
    return n > 0


def _mut_enrol_life(R):
    for k, hx in R.cert_hex.items():
        c = coer.decode('Certificate', bytes.fromhex(hx))
        tb = c['toBeSigned']
        if tb.get('certRequestPermissions') and not tb.get('appPermissions') and c['issuer'][0] != 'self':
            if tb['validityPeriod']['duration'] in (('hours', 52560), ('years', 6)):
                tb['validityPeriod']['duration'] = ('years', 7)
                R.cert_hex[k] = coer.encode('Certificate', c).hex()
                return True
    for p in R.pdus:
        if p['type'] == 'EcaEeCertResponseSpdu':
            d = coer.decode('Ieee1609Dot2Data', bytes.fromhex(p['hex']))
            sd = d['content'][1]
            pdu = coer.decode('ScmsPdu', payload_bytes(sd))
            pdu['content'][1][1]['certificate']['toBeSigned']['validityPeriod']['duration'] = ('years', 7)
            sd['tbsData']['payload']['data']['content'] = ('unsecuredData', coer.encode('ScmsPdu', pdu))
            p['hex'] = coer.encode('Ieee1609Dot2Data', d).hex()
            return True
    return False


def _mut_noncanonical(R):
    p = _first(R, 'RaAcaCertRequestSpdu') or _first(R, 'SecuredCrl')
    if p is None:
        return False
    p['hex'] = p['hex'] + '00'
    return True


def _mut_prerun_skipped(R):
    """The engine's pre-run CRL fetch is skipped (ScmsRun::note_prerun_crl): no CRL Store sends a
    device its lists before it is on the road."""
    n = len(R.pdus)
    R.pdus = [p for p in R.pdus if not (p['type'] == 'SecuredCrl' and p.get('flow') == 'preload')]
    return len(R.pdus) < n


def _mut_held_dropped(R):
    """One device's node.security rows stop listing one CRACA's CRL."""
    R.security = {k: list(v) for k, v in R.security.items()}
    for node, rows in sorted(R.security.items()):
        if rows and rows[0][2]:
            gone = rows[0][2][0][0]
            R.security[node] = [(t, ps, [c for c in crls if c[0] != gone]) for t, ps, crls in rows]
            return True
    return False


def _mut_held_future(R):
    """A device's first node.security row lists a CRL a week newer than the one it holds: a list
    no CRACA had issued."""
    R.security = {k: list(v) for k, v in R.security.items()}
    for node, rows in sorted(R.security.items()):
        if rows and rows[0][2]:
            t, ps, crls = rows[0]
            c = crls[0]
            R.security[node] = [(t, ps, [(c[0], c[1], c[2] + WEEK, c[3] + WEEK)] + crls[1:])] + rows[1:]
            return True
    return False


def _mut_join_old(R):
    """A device's pre-run fetch moved to after its CRACA's next list was issued, keeping the older
    bytes, and its rows with it: a joiner that holds a superseded list (R-JOINCRL)."""
    issued = sorted((row.get('issued_at') or 0, row.get('issue_date') or 0, id8)
                    for (_, id8), row in R.crl_rows.items() if row.get('series') == PSEUDONYM_SERIES)
    if len(issued) < 2:
        return False
    late = issued[-1][0]
    devices = _device_of_node(R)
    R.pdus = [dict(p) for p in R.pdus]
    for p in R.pdus:
        if p['type'] != 'SecuredCrl' or p.get('flow') != 'preload':
            continue
        if prims.hashed_id(bytes.fromhex(p['hex']), 8).hex() == issued[-1][2]:
            continue
        node = next((n for n, d in devices.items() if d == p['to']), None)
        if node is None:
            continue
        for q in R.pdus:
            if q['type'] == 'SecuredCrl' and q.get('flow') == 'preload' and q['to'] == p['to']:
                q['t'] = late + 1
        R.security = {k: list(v) for k, v in R.security.items()}
        R.security[node] = [(max(t, late + 2), ps, crls) for t, ps, crls in R.security[node]]
        return True
    return False


def _mut_pub_late(R):
    """The MA's decision moved five days earlier: the CRL now comes after the SLA."""
    R.revstages = {k: dict(v) for k, v in R.revstages.items()}
    for k, st in R.revstages.items():
        if 'decision' in st:
            st['decision'] -= 5 * 86400 * NS
            return True
    return False


FAULTS = {
    'crl-after-sla': ('a decision published more than a business day later', 'CRL-PUB', _mut_pub_late),
    'pdu-trailing-byte': ('a PDU with a trailing byte (not canonical COER)', 'DECODE', _mut_noncanonical),
    'ack-nextdl-late': ('RaEeCertAck nextDlTime a month later than the downloads', 'DL-AFTER', _mut_ack_next),
    'enrol-7y': ('an enrolment certificate valid 7 years', 'ENR-LIFE', _mut_enrol_life),
    'ra-serves-blocked': ('the RA delivers a batch to a revoked device two days after the decision', 'RA-BLOCK', _mut_block),
    'time32-off': ('pki.state time32 5 s behind (leap seconds dropped)', 'TIME', _mut_time),
    'cert-id8': ('a recorded certificate filed under a wrong HashedId8', 'CERT-ID', _mut_cert_id),
    'crl-sig': ('a CRL signature bit flipped', 'CRL-ENV', _mut_crl_sig),
    'crl-craca': ('crlCraca zeroed', 'CRL-SSP', lambda R: _mut_crl(R, _crl_craca, 0)),
    'crl-series': ('crlSeries 77, not in the signer\'s CrlSsp', 'CRL-SSP', lambda R: _mut_crl(R, _crl_series, 0)),
    'crl-next': ('nextCrl = issueDate', 'CRL-NEXT', lambda R: _mut_crl(R, _crl_next, 0)),
    'crl-irev-future': ('iRev one past the i-period at issueDate', 'CRL-IREV', lambda R: _mut_crl(R, _crl_irev, 0)),
    'crl-index': ('indexWithinI / crlSerial jumps', 'CRL-SEQ', lambda R: _mut_crl(R, _crl_index, 1)),
    'crl-backdated': ('a later CRL dated before the earlier one', 'CRL-SEQ', lambda R: _mut_crl(R, _crl_back, 1)),
    'crl-drop-entry': ('a later full CRL drops an unexpired entry', 'CRL-KEEP', lambda R: _mut_crl(R, _crl_drop, 1, prev_entries=True)),
    'crl-overdue': ('the run outlives the last nextCrl with no new CRL', 'CRL-SLA', _mut_crl_late),
    'crl-row': ('the engine\'s CRL row issue_date off by one', 'CRL-STATE', _mut_crl_row),
    'cert-craca': ('a pseudonym\'s cracaId changed', 'CRACA-CERT', _mut_cert(lambda t: t.update(cracaId=b'\x00\x00\x01'))),
    'cert-series': ('a pseudonym on crlSeries 4', 'CRACA-CERT', _mut_cert(lambda t: t.update(crlSeries=4))),
    'ack-hash': ('RaEeCertAck requestHash zeroed', 'DL-ACK', _mut_ack_hash),
    'dl-sig': ('a Download-Req signature bit flipped', 'DL-SIGN', _mut_sig('EeRaDownloadRequestSpdu', 'opened')),
    'dl-other-enrolment': ('a device\'s Download-Req signed by another device', 'DL-SIGN', _mut_enrol_swap),
    'dl-time': ('Download-Req generationTime 10 s ahead', 'DL-TIME', _set_dl('generationTime', lambda v, p: v + 10)),
    'dl-lowercase': ('filename in lower-case hex', 'DL-NAME', _set_dl('filename', lambda v, p: v.lower())),
    'dl-wrong-hash': ('filename names another request', 'DL-NAME', _set_dl('filename', lambda v, p: '0123456789ABCDEF' + v[16:],
                                                                          lambda p: p['msg'] != 'report' and p['flow'] not in ('preload', 'rsu-application-cert'))),
    'dl-poll-fast': ('the same file requested again after 60 s', 'DL-WAIT', _mut_dl_poll_fast),
    'mbr-recipient': ('report encrypted to an unknown recipient', 'MBR-ENC', _mut_mbr_recipient),
    'mbr-psid': ('report signed under psid 0x20', 'MBR-SIG', _mut_mbr(_mbr_psid)),
    'mbr-late': ('report generationTime after its certificate expires', 'MBR-SIG', _mut_mbr(_mbr_late)),
    'mbr-sig': ('report signature bit flipped', 'MBR-SIG', _mut_sig('SecuredMisbehaviorReport', 'opened', lambda p: p['msg'] == 'report')),
    'mbr-forward-altered': ('RA forwards a report with one byte changed', 'MBR-UP', _mut_forward),
    'ctl-gap': ('CTL history skips a sequence number', 'CTL-SUM', _mut_ctl(_ctl_gap)),
    'ctl-quorum': ('the newest CTL has one elector signature', 'CTL-SUM', _mut_ctl(_ctl_quorum)),
    'ctl-early': ('a CTL in force before its effectiveDate', 'CTL-SUM', _mut_ctl(_ctl_early)),
    'ctl-device-ahead': ('a device holds a CTL not yet published', 'CTL-DEV', _mut_dev_ahead),
    'crl-prerun-skipped': ('no pre-run CRL fetch (skip note_prerun_crl)', 'OF-3', _mut_prerun_skipped),
    'crl-held-dropped': ('a device stops listing one CRACA\'s CRL', 'OF-3', _mut_held_dropped),
    'crl-held-future': ('a device lists a CRL no CRACA has issued', 'OF-3', _mut_held_future),
    'crl-join-old': ('a joiner fetches a superseded list', 'OF-3-JOIN', _mut_join_old),
}


def mutate(R, names=None):
    """-> list of (fault, rule, status) with status 'red', 'SILENT' or 'n/a' (no data to mutate).
    Red means the target rule has more failures than on the unmutated records."""
    out = []
    base = {r: len(f) for r, f in run_rules(R).bad.items()}
    for name, (desc, rule, fn) in FAULTS.items():
        if names and name not in names:
            continue
        M = copy.copy(R)
        M.pdus = [dict(p) for p in R.pdus]
        M.cert_hex = dict(R.cert_hex)
        M.crl_rows = {k: dict(v) for k, v in R.crl_rows.items()}
        M.times = list(R.times)
        M.creds = collections.defaultdict(list, {k: list(v) for k, v in R.creds.items()})
        M.ctl = list(R.ctl)
        M.security = R.security
        if not fn(M):
            out.append((name, rule, 'n/a'))
            continue
        X = run_rules(M)
        out.append((name, rule, 'red' if len(X.bad.get(rule, [])) > base.get(rule, 0) else 'SILENT'))
    return out
