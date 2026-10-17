"""Named faults the oracle can inject into itself, to prove that each corpus check can go red.

``python tools/oracle/oracle.py mutate`` runs the whole corpus once per fault and requires every
fault to turn at least one of its target checks red.  A fault that changes nothing means the check
it targets cannot fail, which is a defect in the check (see the project's "checks that cannot
fail" rule).
"""

ACTIVE = set()

# fault -> (what it breaks, corpus checks that must go red)
CATALOGUE = {
    'ecies-printed-p1': ('ECIES uses the P1 printed in D.6.2 / ecies.txt (= k) instead of the real P1', ['ieee-d6.2', 'camp-ecies']),
    'ecies-dhaes': ('ECIES hashes V into the KDF input (DHAES mode) instead of non-DHAES', ['ieee-d6.2', 'camp-ecies']),
    'kdf2-counter-0': ('KDF2 counter starts at 0 (KDF1-style) instead of 1', ['ieee-d6.4', 'camp-kdf', 'nist-x963', 'ieee-d6.2']),
    'mac1-no-truncate': ('MAC1 keeps the full 32-byte HMAC instead of the leftmost 16 bytes', ['ieee-d6.3', 'camp-mac1', 'ieee-d6.2']),
    'hmac-wrong-pad': ('HMAC uses ipad 0x35 instead of 0x36', ['rfc4231', 'ieee-d6.3']),
    'ccm-flags-tag8': ('CCM B0 flags encode an 8-byte tag instead of 16', ['ieee-d6.1', 'camp-aesccm', 'nist-ccm']),
    'ccm-counter-1': ('CCM keystream counter starts at 2 instead of 1', ['ieee-d6.1', 'camp-aesccm', 'nist-ccm']),
    'hashedid-high-bytes': ('HashedIdN takes the first N bytes of the hash instead of the last', ['ieee-hashedid']),
    'sha256-is-sha224': ('SHA-256 replaced by SHA-224 padded', ['nist-sha', 'ieee-hashedid']),
    'ecdsa-accept-any': ('ECDSA verify returns True without checking', ['nist-ecdsa']),
    'ecdsa-skip-range': ('ECDSA verify skips the 1 <= r, s < n range check', ['nist-ecdsa-range']),
    'bf-swap-prefix': ('butterfly expansion swaps the signing (00000000) and encryption (FFFFFFFF) prefixes', ['camp-bfkeyexp']),
    'bf-no-reduce': ('butterfly f is not reduced mod n', ['camp-bfkeyexp']),
    'lv-no-pad': ('linkage seed update drops the 0^112 padding', ['camp-lv', 'camp-glv']),
    'lv-laid-32': ('linkage uses a 32-bit la_id field', ['camp-lv', 'camp-glv']),
    'lv-no-xor': ('PLV omits the Davies-Meyer XOR with the input', ['camp-lv', 'camp-glv']),
    'ecqv-no-sec4-truncate': ('SEC 4 mode uses the full 256-bit e instead of the leftmost 255 bits', ['camp-implicit']),
    'ecqv-1609-sec4-truncate': ('1609 mode truncates e to 255 bits (SEC 4 rule)', ['oracle-ecqv-1609']),
    'ecqv-1609-whole-cert': ('1609 mode hashes the whole certificate instead of H(tbs) || H(issuer)', ['oracle-ecqv-1609']),
    'time32-no-leap': ('Time32 ignores leap seconds (UTC seconds since 2004)', ['time32']),
    'chain-skip-signature': ('chain validation does not verify certificate signatures', ['oracle-chain']),
    'chain-skip-validity': ('chain validation skips validity-period containment', ['oracle-chain']),
    'crl-no-advance': ('linkage CRL matching does not advance the seeds from iRev to iCert', ['oracle-crl']),
    'ec-double-bug': ('point doubling uses a = 0 instead of a = -3', ['ec-cross', 'camp-bfkeyexp', 'ieee-d6.2']),
}


def on(name):
    return name in ACTIVE
