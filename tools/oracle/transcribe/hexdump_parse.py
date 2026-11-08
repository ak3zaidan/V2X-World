"""Parse the 'LABEL = 0xHEX' layout shared by IEEE 1609.2a Annex D.6 and the CAMP vector files.

Both sources print each value as a label line ending in '=' (or 'LABEL = 0x...' on one line),
followed by a hex string that may wrap over several lines, followed by a C-array repeat of the
same bytes ('name[n] = { 0x.., ... }').  Page furniture (running headers, copyright lines, page
numbers) can interrupt a wrapped hex string in the PDF text; the parser skips it.  The C-array
repeat is used as a cross-check: every hex value whose array is present must equal it, so a
wrapping or extraction error cannot pass silently.

Returns records: {'line': first line number (1-based), 'label': str, 'hex': str, 'array_ok': bool|None}.
"""

import re

_HEX_START = re.compile(r'^(?P<label>.*?)=\s*0x(?P<hex>[0-9A-Fa-f]+)\s*$')
_HEX_ONLY = re.compile(r'^0x(?P<hex>[0-9A-Fa-f]+)\s*$')
_HEX_CONT = re.compile(r'^(?P<hex>[0-9A-Fa-f][0-9A-Fa-f ]*)$')
_ARRAY_HEAD = re.compile(r'^[A-Za-z_][A-Za-z0-9_]*\[(?P<n>\d+)\]\s*=\s*(?P<rest>\{.*)?$')
_BYTE = re.compile(r'0x([0-9A-Fa-f]{2})')

FURNITURE = (
    'IEEE Std 1609.2a-2017',
    'IEEE Standard for Wireless Access',
    'Messages—Amendment 1',
    'Copyright ©',
)


def _is_furniture(s):
    return (not s) or any(s.startswith(f) for f in FURNITURE) or re.fullmatch(r'\d{1,3}', s) is not None


def parse(lines, start=0, end=None):
    end = len(lines) if end is None else min(end, len(lines))
    out = []
    prev_label = ''
    i = start
    while i < end:
        s = lines[i].strip()
        m = _HEX_START.match(s)
        m2 = _HEX_ONLY.match(s)
        if m or m2:
            label = (m.group('label').strip() if m else prev_label).rstrip('= ').strip()
            hx = (m or m2).group('hex')
            first = i + 1
            j = i + 1
            # continuation lines (pure hex), possibly separated by page furniture
            while j < end:
                t = lines[j].strip()
                if _HEX_CONT.match(t) and not re.fullmatch(r'\d{1,3}', t):
                    hx += t.replace(' ', '')
                    j += 1
                    continue
                if _is_furniture(t):
                    k = j
                    while k < end and _is_furniture(lines[k].strip()):
                        k += 1
                    if k < end and _HEX_CONT.match(lines[k].strip()):
                        j = k
                        continue
                break
            # optional C-array repeat
            arr_ok = None
            k = j
            while k < end and _is_furniture(lines[k].strip()):
                k += 1
            if k < end and _ARRAY_HEAD.match(lines[k].strip()):
                n = int(_ARRAY_HEAD.match(lines[k].strip()).group('n'))
                bs = []
                while k < end:
                    t = re.sub(r'\s+', '', lines[k])
                    t = re.sub(r'^[A-Za-z_][A-Za-z0-9_]*\[\d+\]=', '', t)
                    if not _is_furniture(t):
                        bs += _BYTE.findall(t)
                    k += 1
                    if '}' in t:
                        break
                arr_ok = (''.join(bs).upper() == hx.upper()) and len(bs) == n
                j = k
            out.append({'line': first, 'label': label, 'hex': hx.upper(), 'array_ok': arr_ok})
            i = j
            continue
        if s and not _is_furniture(s) and not s.startswith('{') and not _ARRAY_HEAD.match(s):
            prev_label = s
        i += 1
    return out
