"""Chain validation (02 section 12) and CRL matching (02 section 11) on asn1tools certificate values.

``validate(chain, anchors)``: chain[0] is the end-entity certificate, each next one the issuer of the
previous, the last one a self-signed root.  Returns a list of rule failures (empty = valid).
Rules implemented: issuer linkage by HashedId8, cryptographic verification (explicit ECDSA; implicit
via ECQV-1609 with the key checked on the next object), PSID coverage of appPermissions and
certIssuePermissions by the issuer's certIssuePermissions (explicit PSID or `all`), opaque/omitted
SSP rules, eeType, the chain-length inequalities, validity containment, identifiedRegion containment
(country lists), trust anchor membership (HashedId32 of the root, as listed on a CTL).

UNCONFIRMED wording (02 section 12): region containment beyond identifiedRegion country lists and
bitmap SSP ranges are not implemented; they are reported as 'unsupported' rather than passed.
"""

from . import certs, faults, linkage, prims

YEAR = 31556952
UNIT = {'microseconds': 1e-6, 'milliseconds': 1e-3, 'seconds': 1, 'minutes': 60, 'hours': 3600, 'sixtyHours': 216000, 'years': YEAR}


def validity(t):
    vp = t['validityPeriod']
    unit, n = vp['duration']
    return vp['start'], vp['start'] + int(n * UNIT[unit])


def _groups(t):
    return t.get('certIssuePermissions', [])


def _covers(issuer_t, psid, ssp, want_ee):
    explicit_hit = None
    all_hit = None
    for g in _groups(issuer_t):
        ee_bits = g.get('eeType', (b'\x80', 8))[0][0]
        allows = bool(ee_bits & (0x80 if want_ee == 'app' else 0x40))
        kind, body = g['subjectPermissions']
        if kind == 'all':
            all_hit = (g, allows, None)
        else:
            for pr in body:
                if pr['psid'] == psid:
                    explicit_hit = (g, allows, pr.get('sspRange'))
    hit = explicit_hit or all_hit
    if hit is None:
        return f'psid {psid} not covered'
    g, allows, ssp_range = hit
    if not allows:
        return f'psid {psid}: issuer eeType does not allow {want_ee}'
    if explicit_hit is not None and ssp_range is not None:
        rk, rv = ssp_range
        if rk == 'all':
            return None
        if rk == 'opaque':
            sub = ssp[1] if ssp is not None and ssp[0] == 'opaque' else (b'' if ssp is None else None)
            if sub is None:
                return f'psid {psid}: subordinate SSP kind {ssp[0]} vs opaque range'
            return None if sub in rv else f'psid {psid}: opaque SSP not in issuer range'
        return f'psid {psid}: unsupported sspRange {rk}'
    return None


def _chain_len(g):
    return g.get('minChainLength', 1), g.get('chainLengthRange', 0)


def check_pair(sub, iss, depth_below_issuer):
    """Consistency of a subordinate with its issuer (02 section 12). depth: 0 when sub is the EE."""
    errs = []
    st, it = sub['toBeSigned'], iss['toBeSigned']
    for ps in st.get('appPermissions', []):
        e = _covers(it, ps['psid'], ps.get('ssp'), 'app')
        if e:
            errs.append('appPermissions ' + e)
    for g in st.get('certIssuePermissions', []):
        kind, body = g['subjectPermissions']
        if kind == 'explicit':
            for pr in body:
                e = _covers(it, pr['psid'], None, 'app')
                if e:
                    errs.append('certIssuePermissions ' + e)
        elif not any(x['subjectPermissions'][0] == 'all' for x in _groups(it)):
            errs.append('certIssuePermissions all not covered by an issuer all entry')
    # chain length: issuer (mcr_i, clr_i) vs subordinate (mcr_s, clr_s); EE has mcr_s = clr_s = 0
    if _groups(it):
        mi, ci = max((_chain_len(g) for g in _groups(it)), key=lambda x: x[0] + (10 ** 6 if x[1] == -1 else x[1]))
        if mi == 0:
            errs.append('issuer minChainLength 0')
        if _groups(st):
            ms, cs = max((_chain_len(g) for g in _groups(st)), key=lambda x: x[0] + (10 ** 6 if x[1] == -1 else x[1]))
        else:
            ms, cs = 0, 0
        if not mi <= ms + 1:
            errs.append(f'chain length: issuer minChainLength {mi} > subordinate {ms} + 1')
        if ci != -1 and not (cs != -1 and mi + ci >= ms + cs + 1):
            errs.append(f'chain length: issuer range {mi}+{ci} < subordinate {ms}+{cs}+1')
    s0, s1 = validity(st)
    i0, i1 = validity(it)
    if (s0 < i0 or s1 > i1) and not faults.on('chain-skip-validity'):
        errs.append('validity not inside issuer validity')
    if 'region' in st:
        if 'region' not in it:
            pass
        else:
            sk, sv = st['region']
            ik, iv = it['region']
            if sk == ik == 'identifiedRegion':
                def countries(v):
                    return {x[1] if x[0] == 'countryOnly' else x[1]['countryOnly'] for x in v}
                if not countries(sv) <= countries(iv):
                    errs.append('region not inside issuer region')
            else:
                errs.append(f'unsupported region containment {sk} in {ik}')
    return errs


def validate(chain, anchor_hashes32=None):
    errs = []
    for n, c in enumerate(chain):
        if n + 1 < len(chain):
            iss = chain[n + 1]
            if c['issuer'][0] == 'self' or c['issuer'][1] != certs.hashed_id8(iss):
                errs.append(f'cert {n}: issuer HashedId8 does not match cert {n + 1}')
                continue
            if c['type'] == 'explicit' and not faults.on('chain-skip-signature'):
                if not certs.verify_explicit(c, iss):
                    errs.append(f'cert {n}: signature does not verify')
            errs += [f'cert {n}: {e}' for e in check_pair(c, iss, n)]
        else:
            if c['issuer'][0] != 'self':
                errs.append('chain does not end at a self-signed root')
            elif not certs.verify_explicit(c, None):
                errs.append('root self-signature does not verify')
            if anchor_hashes32 is not None and prims.sha256(certs.encode_canonical(c)) not in anchor_hashes32:
                errs.append('root not on the CTL (HashedId32)')
    return errs


# --- CRL matching (02 section 11.2) ----------------------------------------------------------

def hash_crl_revokes(entries, cert, now):
    """ToBeSignedHashIdCrl entries [{id: HashedId10, expiry}] against a certificate."""
    h10 = certs.hashed_id10(cert)
    return any(e['id'] == h10 for e in entries)


def linkage_crl_revokes(crl_contents, cert):
    """ToBeSignedLinkageValueCrl individual entries against a pseudonym certificate's linkage value.
    Matching requires cracaId/crlSeries agreement (crlCraca low 3 bytes = cert cracaId)."""
    t = cert['toBeSigned']
    if t['id'][0] != 'linkageData':
        return False
    if crl_contents['crlSeries'] != t['crlSeries'] or crl_contents['crlCraca'][-3:] != t['cracaId']:
        return False
    kind, body = crl_contents['typeSpecific']
    if kind not in ('fullLinkedCrl', 'deltaLinkedCrl'):
        return False
    i_cert = t['id'][1]['iCert']
    lv_cert = t['id'][1]['linkage-value']
    i_rev = body['iRev']
    if i_cert < i_rev:
        return False
    for jg in body.get('individual', []):
        for lg in jg['contents']:
            la1, la2 = int.from_bytes(lg['la1Id'], 'big'), int.from_bytes(lg['la2Id'], 'big')
            for ig in lg['contents']:
                if i_cert > ig['iMax']:
                    continue
                for rev in ig['contents']:
                    steps = 0 if faults.on('crl-no-advance') else i_cert - i_rev
                    s1 = linkage.seed_at(la1, rev['linkageSeed1'], steps)
                    s2 = linkage.seed_at(la2, rev['linkageSeed2'], steps)
                    for j in range(jg['jmax']):
                        p1, p2 = linkage.plv(la1, s1, j), linkage.plv(la2, s2, j)
                        if bytes(a ^ b for a, b in zip(p1, p2)) == lv_cert:
                            return True
    return False
