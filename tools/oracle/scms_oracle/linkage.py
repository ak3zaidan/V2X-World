"""Linkage values (02 section 10, CAMP lv.py / glv.py).

    ls(i)    = first 16 bytes of SHA-256(la_id || ls(i-1) || 0^112)       la_id 16 bits
    plv(i,j) = first 9 bytes of AES_ls(i)(la_id || j || 0^80) XOR (la_id || j || 0^80)   j 32 bits
    lv(i,j)  = plv1(i,j) XOR plv2(i,j)
Group linkage uses the same functions on group seeds; ei(input) = AES_ik(input) XOR input.
"""

from . import faults
from .prims import aes_block, sha256


def _laid(la_id):
    return la_id.to_bytes(4 if faults.on('lv-laid-32') else 2, 'big')


def seed_input(la_id, ls):
    pad = b'' if faults.on('lv-no-pad') else b'\x00' * 14
    return _laid(la_id) + ls + pad


def next_seed(la_id, ls):
    return sha256(seed_input(la_id, ls))[:16]


def seed_at(la_id, ls0, i):
    ls = ls0
    for _ in range(i):
        ls = next_seed(la_id, ls)
    return ls


def plv_input(la_id, j):
    b = _laid(la_id) + j.to_bytes(4, 'big')
    return b + b'\x00' * (16 - len(b))


def plv(la_id, ls_i, j):
    x = plv_input(la_id, j)
    a = aes_block(ls_i, x)
    if faults.on('lv-no-xor'):
        return a[:9]
    return bytes(p ^ q for p, q in zip(a, x))[:9]


def lv(la1, ls1_0, la2, ls2_0, i, j):
    p1 = plv(la1, seed_at(la1, ls1_0, i), j)
    p2 = plv(la2, seed_at(la2, ls2_0, i), j)
    return bytes(a ^ b for a, b in zip(p1, p2))


def encrypted_index(ik, block):
    a = aes_block(ik, block)
    return bytes(p ^ q for p, q in zip(a, block))
