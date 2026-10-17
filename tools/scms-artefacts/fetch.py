#!/usr/bin/env python3
"""Fetch the published SCMS Manager CTL, certificate-chain and elector files into
third_party/scms-artefacts/files/ and check each against SCMS Manager's own SHA-384
fingerprint (https://www.scmsmanager.org/publications/, read 2026-10-16) and against the
sha256 recorded in third_party/scms-artefacts/PROVENANCE.md.

The files are not committed: the publications page carries "(c) SCMS Manager LLC All
Rights Reserved" and no licence (owner decision pending, see PROVENANCE.md). Run this once,
then the ignored tests in crates/v2xw-sec/tests/real_artefacts.rs:

    python3 -I tools/scms-artefacts/fetch.py
    cargo test -p v2xw-sec --test real_artefacts -- --ignored

Prints one verdict line. Standard library plus the curl command. Re-running is a no-op when every file is
present and matches.
"""

import hashlib
import io
import os
import subprocess
import sys
import zipfile

BASE = 'https://www.scmsmanager.org/wp-content/uploads/'
UA = ('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 '
      '(KHTML, like Gecko) Version/17.0 Safari/605.1.15')
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'third_party',
                    'scms-artefacts', 'files')

# (zip path under BASE, member basename, local path, published SHA-384 or None, unused)
# The published SHA-384 values are the fingerprints printed next to each download on the
# publications page; for a CTL zip the page's value is the SHA-384 of the .oer inside it.
FILES = [
    ('2025/08/20250813-production_ctl.zip', 'production_ctl.oer', 'production-2025-08-13/production_ctl.oer',
     'ca871c47fa486aa95c5114e01c5905878cba026c062caf325336a2cb575a82992dc556e8327f5515a36f3d3ada3f5921',
     None),
    ('2025/08/20250813-production_ctl.zip', 'cert-chain_0.oer', 'production-2025-08-13/cert-chain_0.oer', None, None),
    ('2025/08/production_ctl.zip', 'production_ctl.oer', 'production-2025-08/production_ctl.oer',
     'c7401113c537f33dae5d851a40de715e9238b218afdb2f35eab283df738caebed83042ee5eb2a1c22d03558e897445a9',
     None),
    ('2025/07/SCMS_Manager-Test_CTL-0348000000000000-679737605.zip',
     'SCMS_Manager-Test_CTL-0348000000000000-679737605.oer', 'test-2025-07-17/test_ctl.oer',
     '1327546c169c763cf67182244310613c440af4ecde15dffc284aec228be0c01028d5fc8f903134e7001f5c112514dd18',
     None),
    ('2025/07/SCMS_Manager-Test_CTL-0348000000000000-679737605.zip', 'SCMS_Manager-Test_Cert_Chain.oer',
     'test-2025-07-17/test_cert_chain.oer', None, None),
    ('2025/03/SCMS_Manager-Test_CTL-0348000000000000-669060983.zip',
     'SCMS_Manager-Test_CTL-0348000000000000-669060983.oer', 'test-2025-03-18/test_ctl.oer',
     '0518312d089d851db50df1cc01b50a9adf2da8918299ee5241ab4ba2d8905fbee9ee6a99225b5a8b7863ee5b72519fe2',
     None),
    ('2024/05/SCMS_Manager-Test_CTL-0348000000000000-640624233.zip',
     'SCMS_Manager-Test_CTL-0348000000000000-640624233.oer', 'test-2024-04-19/test_ctl.oer',
     '57529db996d99c6b63f361abbc7da7c84b7ee5cb6f779ca0e97930ca8176576241fb551b489d00d77876c7428300ec6f',
     None),
    ('2023/04/test_ctl.zip', 'test_ctl.oer', 'test-2023/test_ctl.oer',
     'bf4dfa460bf4f3001547bfeb5c85abfa7aca23398e74733ce06a7ee29c5972b77b9f52ec68c5648ee0d320f4ba9f9b93',
     None),
]
PROD_ELECTORS = [
    '2621351ce48cd2dde9e7a2c21a0c6831af5ff8e348887fc64b98cc7066d1dd8ade4ed9f290b343ca5bc9586e217980f4',
    'b29cea24e45d1bf80c6771a1ba2c9d602d8520fc49226a9e861093922562a07298f659218ab9f3a58fe0d1cfe1498fb6',
    'ae574cc2ea73afd2ad1461114a92ac9faa98b533251deb993f3f717b437ff8f520211c63bd4cb3af991fffc80a0c0236',
    '649f1f960d54a4af55249faf055fc3645144f8081e50490855cb52e022af7b24c9e49c5cbd64fa47d7efc29e2c4fd699',
    '8af5944670d1faad01ef67b9a49f0a528acecec6bad377401b5c24d253c563ad57e348d7af20de33a64775d3c8e723e5',
]
TEST_ELECTORS = [
    'f0a2b93f6b27ed5b6b59655f82698f10a02e373b01488539ce02eb60857b492547f282f1e830e437a77b28daf47aa53b',
    '42d4cc9dc47f8ec105cea034cf4078969f7044ccbaa078d4d3481dc639238db7d5687e2d970873ffd1327899a869e489',
    '893e3f96aa821c6319c3bd9004722996f6f03c2ce3565401b7175f8f57be11b6a81372e724211db8253375d819624b1a',
    '5cbe8d326c9ce52b83ecfca2b98ae65e09c947c2c1c8d88dc02f277cf6468f7eb491382b1c1144310aa4f60198d935bb',
    '76d5b5e5105d0d9878fbd2b674fa93d25221060a9ce20e0de1df9df0b8baef6b655e9f77a7ccf4195965d66d0b46049c',
]
for i, h in enumerate(PROD_ELECTORS, 1):
    FILES.append((f'2020/01/v2xelector{i}.certificate.oer_.zip', f'v2xelector{i}.certificate.oer',
                  f'electors/v2xelector{i}.certificate.oer', h, None))
for i, h in enumerate(TEST_ELECTORS, 1):
    FILES.append((f'2020/08/v2xelector{i}.test_.certificate.zip', f'v2xelector{i}.test.certificate.oer',
                  f'electors/v2xelector{i}.test.certificate.oer', h, None))
for n in ['AutocryptTestCTLRoot.oer', 'cht-scms.rca.cert', 'iss_preprod_root.oer', 'microsecRCA.oer',
          'saesol_rca_09F453B62DE0813A.oer']:
    FILES.append(('2025/07/SCMS_Manager-Test_CTL-0348000000000000-679737605.zip', n,
                  f'test-2025-07-17/roots/{n}', None, None))


# sha256 of every file as first fetched on 2026-10-16 (also listed in PROVENANCE.md).
SHA256 = {
    'electors/v2xelector1.certificate.oer': '69672c34cc6150e6eb7e5ee33a5e417764c7367915b7499ffafd9cf5f58efc80',
    'electors/v2xelector1.test.certificate.oer': 'b7effd75567d6e1a168ae6149bf6deaf5510649b998cc715ebafda807be40eb9',
    'electors/v2xelector2.certificate.oer': '8572edc4a0fa38590cb4ea42c56f23afdc57361b97c907c07823ab1dae165bf8',
    'electors/v2xelector2.test.certificate.oer': 'a30575cdaa169207c9ba938c4fcc18bda9cac843e6db0e9e6e837a4a914c1dd5',
    'electors/v2xelector3.certificate.oer': '00eceda7a1c93d8708e435854f6264137c8b0fcf89def1353ffe2a75cbc16f71',
    'electors/v2xelector3.test.certificate.oer': '1e170909d4841e4ca9d53e9c5bb54083033f90c2ae52290ea4bf8dc210674542',
    'electors/v2xelector4.certificate.oer': '4a0c4aba3df05992a162af35196e0e4d5230d3c23cdd699708495def2024db77',
    'electors/v2xelector4.test.certificate.oer': 'b623e92028634943c5e95455934ecdd758c4a148935066ffdee78546d2607d6c',
    'electors/v2xelector5.certificate.oer': '2eb53d2a565bf1464de7822d025493edbd84d7d8864ed0786f92d83bbca56afc',
    'electors/v2xelector5.test.certificate.oer': '5cdd0883e013abe963386125656eba04c4afa3161fdca06eeac03b12c0d800a2',
    'production-2025-08-13/cert-chain_0.oer': '738b3400c4133f7087812b460000998535795f296ce74f3edebd3952aeb6808b',
    'production-2025-08-13/production_ctl.oer': '4c0b7d61b37ce75c1bf6665574ffce0cbdc0081c5e53af7faf94343dc4b33d65',
    'production-2025-08/production_ctl.oer': 'c554f1c32ff4538dc87bc539d4e76c55c646456f2c4754278f1b0d116077ccd5',
    'test-2023/test_ctl.oer': 'a43948eac84bd6fccc2427e58d50b21d5e460e017c1428c040500ed093bce514',
    'test-2024-04-19/test_ctl.oer': 'fbf3f0f2c3078626671bf3b97a7a6ec307709f23384b34c15b44bf834c4424ec',
    'test-2025-03-18/test_ctl.oer': '529d1c2f1f499ce110d2e0c12446c366530a8eb74aa4a85e8845aeb95bdd5daa',
    'test-2025-07-17/roots/AutocryptTestCTLRoot.oer': 'fb792716aa0f1bd736741830bfa46581767d8f21d0e6e2cb160eb4841a2761c2',
    'test-2025-07-17/roots/cht-scms.rca.cert': '284e3b44b1631ba0ce6311a0fa88ae562397282e27c2b27c3dd54909f1153c31',
    'test-2025-07-17/roots/iss_preprod_root.oer': '1f4c904756288dbe29594136c00bfc7a61b0ee868d45cb7a93232614ee5e6f5b',
    'test-2025-07-17/roots/microsecRCA.oer': '497db8c6223911af42d048e8aff03ffd1a7027fb90991695de37b64ec032dcde',
    'test-2025-07-17/roots/saesol_rca_09F453B62DE0813A.oer': '9605dcb79d890cfb3636469551d09538843509708edb930409f453b62de0813a',
    'test-2025-07-17/test_cert_chain.oer': '9dbc552e16d3dae96377c5fab094ffeeb3a289444cbece2ed218331aa598414c',
    'test-2025-07-17/test_ctl.oer': 'b7f2a46c88175b07a2103ce7b8aee84ab2b932565a5d26f8adb2e7cb9715ae71',
}


def fetch(path, cache={}):
    # curl, not urllib: some Python builds ship without a CA bundle, and the site's CDN
    # refuses clients without a browser User-Agent.
    if path not in cache:
        body = subprocess.run(['curl', '-sSfL', '--max-time', '60', '-A', UA, BASE + path],
                              check=True, capture_output=True).stdout
        cache[path] = zipfile.ZipFile(io.BytesIO(body))
    return cache[path]


def main():
    bad, fetched = [], 0
    for zpath, member, local, sha384, _ in FILES:
        dst = os.path.join(ROOT, local)
        if os.path.exists(dst):
            data = open(dst, 'rb').read()
        else:
            zf = fetch(zpath)
            names = [n for n in zf.namelist() if not n.startswith('__MACOSX') and os.path.basename(n) == member]
            if len(names) != 1:
                bad.append(f'{local}: {member} not found once in {zpath}')
                continue
            data = zf.read(names[0])
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            open(dst, 'wb').write(data)
            fetched += 1
        if sha384 and hashlib.sha384(data).hexdigest() != sha384:
            bad.append(f'{local}: SHA-384 differs from the published fingerprint')
        if hashlib.sha256(data).hexdigest() != SHA256.get(local):
            bad.append(f'{local}: sha256 differs from the recorded provenance')
    verdict = 'PASS' if not bad else 'FAIL'
    print(f'scms-artefacts: {verdict} ({len(FILES)} files, {fetched} fetched, '
          f'{sum(1 for f in FILES if f[3])} checked against published SHA-384)')
    for b in bad:
        print('  ' + b)
    return 0 if not bad else 1


if __name__ == '__main__':
    sys.exit(main())
