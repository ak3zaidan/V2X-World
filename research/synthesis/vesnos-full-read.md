# VESNOS read in full, from its code

Written 2026-10-23. Role: critical reviewer. This replaces the abstract-only reading of `abdo2024vehicular` that `landscape.md` §5, `claims-we-cannot-make.md` and `novelty-check.md` relied on.

**VESNOS:** "VEhicular Secure Network Open Simulator (VESNOS): A Cyber-security oriented co-simulation platform for connected and automated driving". Ahmed Abdo, Guoyuan Wu, Nael Abu-Ghazaleh. ACM SIGSIM-PADS 2024, pp. 108–118, doi:10.1145/3615979.3656054. Code: https://github.com/avmon/VESNOS.

**Basis: code.** The paper's full text could not be obtained.
- The ACM PDF, full-HTML and landing pages returned HTTP 403 on every attempt. ResearchGate returned 403 too.
- Unpaywall and OpenAlex list only the publisher copy (CC-BY, "hybrid"), with no repository copy.
- The eScholarship PDF that search offered (item `5bv507mp`) is the VENTOS-HIL paper (Ching et al., Vehicular Communications 2020), not VESNOS.

Everything below that is not marked "paper" comes from the released code, the user manual shipped in the repository, and Crossref metadata (34 references). The paper's case study is known only from a search-engine excerpt (§3.13).

All VESNOS paths are relative to the repository root at commit `997913b5` (2024-05-09). Engine paths are relative to this repository at `e4004b4c`.

---

## 0. Verdict

**VESNOS is VENTOS (2019) plus about 2,450 lines that sign and verify beacons.** It uses one hard-coded IEEE 1609.2 pseudonym certificate from a test PKI, and every vehicle shares it. No SCMS entity runs in the simulation:
- no RA, PCA/ACA, ECA, LA, MA, root or electors;
- no issuance, linkage values, misbehaviour reports or revocation of a pseudonym.

The "CA" and "CRL" modules are VENTOS's 2015-era CRL-piece broadcaster. Its CRL is a list of dummy C++ objects, and vehicles throw the pieces away.

Credit where due: the cryptography follows the CAMP reference algorithms, and the certificate set is genuine. An independent check (§3.2) confirms that it forms a consistent key pair under butterfly expansion plus ECQV reconstruction. In that narrow sense the signatures are real.

The integration, however, is broken in ways that make security research with it impossible as released:
- What is signed is the 8-byte in-memory address of the beacon object, not the beacon (`01_Beacon.cc:231-237`).
- The receiver trusts the PCA public key that the sender attaches (`Manager.cc:220-223`).
- A verified BSM is only printed to the log. It is never passed to the applications (`Manager.cc:239-276`).
- By static reading, RSUs throw a runtime error on receiving a BSM (`rsu/Manager.cc:137`). This was not run.
- The built-in falsification and replay attacks act only on the unsigned beacon type, which VESNOS vehicles no longer send (`ApplAdversary.cc:111-120`).
- Security adds no bytes to any frame and no time to any event.

**What this means for the owner.**
- **One claim stays dead.** "First simulator integrating an SCMS with traffic and network simulation" cannot be made, because VESNOS's title and abstract claim it and reviewers will take that at face value.
- **The narrowed claim is now strong and backed by code evidence:** *the first open simulator in which the SCMS backend itself (issuance, linkage, reporting, MA decision, linkage-seed revocation, trust lists) runs inside a traffic and radio loop*.
- **No research direction is threatened.** None of the three approaches in `final-three-approaches.md` and none of the directions in `research-plan.md` is touched by VESNOS. None of them can even be posed in VESNOS.
- **Most of the engine's advantage over VESNOS is engineering completeness, not research novelty** (§5). It becomes a research contribution only through a result that VESNOS-class tools cannot produce.

---

## 1. What was read

| Item | Version | How |
|---|---|---|
| VESNOS | `997913b5` (HEAD of `master`, 2024-05-09) | `git clone --depth 1`; every file under `src/nodes/SCMS`, `src/nodes/CA`, `src/nodes/adversary`, `src/nodes/vehicle/{01_Beacon,Manager}.*`, `src/nodes/rsu/{Manager,05_CRL}.*`, `src/msg/{BSM,CRL_Piece}.msg`, `src/baseAppl/04_BaseWaveApplLayer.*`, `src/makefrag`, `.oppbuildspec`, every `examples/security_*`, `examples/v2x`, `examples/omnetpp_general.ini`, `examples/config.xml`, `runme` (dependency parts) |
| VESNOS User Manual | v0.1, April 2024 (9 pp., in the repository) | Full text extracted |
| VENTOS (upstream) | `ManiAm/VENTOS_Public` `60c5d62a` (2019-03-29) | `git clone --depth 1`; `diff -r` of `src/` and `examples/` against VESNOS |
| CAMP crypto test vectors (cited by the paper as Grantcharov 2018) | `conz27/crypto-test-vectors`, branches `master` `dd5014e6`, `values_for_ghs` `a77efb56`, `python3` `f77d9ff`, `ecc_py3` `50d03ab` | Shallow clones; searched for VESNOS's hard-coded values (§3.2) |
| GitHub metadata | API, 2026-10-23 | Commits, stars, forks, issues, licence |
| Crossref, OpenAlex, Unpaywall | API, 2026-10-23 | Pages, licence, references, OA locations, citing works |
| Paper full text | n/a | Not obtained (see the basis note above) |

Nothing from VESNOS was built or run. No engine code was built or run. VESNOS is GPL-derived (§3.15), so nothing from it may be copied into this Apache-2.0 engine.

---

## 2. What VESNOS adds to VENTOS

`diff -r` against VENTOS `60c5d62a` finds VESNOS changes in only these places:

| Change | Lines |
|---|---|
| `src/nodes/SCMS/` (new): `ecc.h` 799, `bsmverify.h` 628, `bsmsign.h` 184, `bfkeyexp.h` 175, `implicit.h` 163, `radix.h` 140, `carray.h` 37, `hex_str.h` 21 | 2,147, all header-only |
| `src/msg/BSM.msg` (new) | 45 |
| `src/nodes/vehicle/01_Beacon.cc` / `.h` | +173 / +3 |
| `src/nodes/vehicle/Manager.cc` / `.h` | +78 / +5 |
| `src/baseAppl/04_BaseWaveApplLayer.h` | +1 (`TYPE_BEACON_BSM`, line 60) |
| `src/makefrag` | +1 (`-lssl -lcrypto -lboost_random -lcryptopp -lpthread`, line 3) |

Unchanged VENTOS code includes:
- the CA, CRL, adversary and RSU modules;
- the whole radio stack (`src/MIXIM_veins`);
- TraCI, the router and the traffic-light controllers;
- every example configuration (four examples gained only a `.qtenvrc`).

So every security feature beyond beacon signing that the paper might describe is either VENTOS's (Amoozadeh et al.) or absent.

---

## 3. What VESNOS actually implements

### 3.1 Cryptography

- **The library is hand-written arithmetic, not OpenSSL.**
  - The manual says "We use OpenSSL library to sign and verify different messages" (manual p5). That is not what the code does.
  - The elliptic-curve arithmetic is hand-written on `boost::multiprecision::cpp_int`. It covers affine and Jacobian points, double-and-add, and modular inversion by recursive extended Euclid. The code is `ecc.h:46-101` and `ecc.h:198-732`, and the ECDSA is at `ecc.h:735-796`.
  - OpenSSL supplies only SHA-256 (`ecc.h:31-44`, `implicit.h:43-55`). Crypto++ supplies AES-128-ECB for butterfly expansion (`bfkeyexp.h:17-20`, `41-85`).
  - The code reads as a C++ port of the CAMP Python reference (`ecc.py`, `implicit.py`, `bfkeyexp.py`, `radix.py`, `array.py`): the class names, parameter names, `sec4` flags and `cArrayDef` debug printing match. VESNOS does not port CAMP's `lv.py` (linkage values), `ecies.py`, `aesccm.py`, `kdf.py`, `mac1.py` or `glv.py`.
- **Curves.** P-256 is used. P-384 is defined (`ecc.h:187-196`) but never used.
- **The ECDSA nonce is `k = rand() % (n − 1) + 1`** (`ecc.h:770`).
  - `rand()` returns at most `RAND_MAX`, which is 2³¹ − 1 on glibc. So k has about 31 bits, and the private key can be recovered from a single signature by baby-step giant-step.
  - `rand()` is never seeded on the signing path, so the sequence is fixed. It is shared by every vehicle in the process.
  - The router example calls `srand(time(NULL))` (`src/router/Router.cc:357`), which makes the nonces depend on the wall clock there.
  - For a simulator this is not a confidentiality problem. It does mean that no key-compromise or forgery result from VESNOS would mean anything.
- **A platform-dependent hex parser.** `hexStringToBigInt` left-aligns every hex string to `sizeof(cpp_int)*8` bits (`ecc.h:120`).
  - It returns the right value only when that width equals the string's width. On x86-64 Boost that width is plausibly 256 bits, which is exactly the P-256 field and digest width.
  - A 96-digit P-384 constant makes the `size_t` subtraction wrap. The cofactor `"1"` becomes 2²⁵²; it is unused.
  - The P-256 path depends on this coincidence. This was not run, so it is flagged here rather than asserted as a crash.
- **A second copy of the arithmetic.** `bsmverify.h:19-473` duplicates `ecc.h` with every symbol renamed (`ECurve2`, `ECPoint2`, `ECDSA2`, `modinv_` …). It is evidently a workaround for the duplicate-symbol link error caused by including non-inline header definitions in two translation units (`01_Beacon.cc` and `Manager.cc`).
- **Encoding bug.** `ECPoint::output` sets `y_str = "compressed-y-1"` in both branches (`ecc.h:369-376`), so every signature's R is labelled `83` whatever its y parity. Verification uses only R.x, so nothing fails.

### 3.2 Credentials: one test-PKI certificate for every vehicle

`ApplVBeacon::SCMS_initialize` (`01_Beacon.cc:279-346`) hard-codes the following values in namespace-scope globals (`01_Beacon.cc:37-50`):
- a PCA certificate;
- a pseudonym certificate (`pseudo_cert_7A_0`) and its to-be-signed part;
- the reconstruction value and the private reconstruction value;
- the caterpillar private key (`cert_seed_prv`) and the expansion key (`cert_exp_val`).

Every vehicle runs the same initialisation on the same inputs, so **every vehicle in every run holds the same private key and the same pseudonym certificate**. There is no second certificate, no pool and no change of pseudonym. The `previous_pseudonym_certificate` field in `BSM.msg:41` and `prev_pseudo_cert` in `Manager.h` are never written or read.

Decoded fields (this review, by hand, from the COER bytes):

| Field | Value |
|---|---|
| Issuer of the pseudonym certificate | HashedId8 `da766b0e278fd23d`, which equals SHA-256 of the hard-coded PCA certificate (last 8 bytes) |
| PCA identity | name `v2xpca-test.ghsiss.com`, the ISS (Green Hills) **test** PKI. Validity starts 2017-04-06, duration 10 years |
| Pseudonym id | `linkageData`, iCert 0x7A = 122, with a linkage value and a group linkage value |
| Pseudonym validity | Time32 421,405,200 = 2017-05-09 09:00 on a leap-second-free count; 169 hours. **Expired on 2017-05-16** |
| Region | Canada (124), Mexico (484), USA (840) |
| appPermissions | PSID 0x20 (BSM) and 0x26 |

**Independent check.** The script in Appendix A is pure-Python P-256 plus the `cryptography` package's AES, and it is not part of either code base. It computes:
- the butterfly expansion f_k(x_cert) for (i, j) = (0x7A, 0);
- e = SHA-256(SHA-256(tbs) ‖ SHA-256(PCA certificate));
- d = e·b + r mod n.

The result: **d·G equals e·P_U + Q_PCA**, so the hard-coded set is a consistent butterfly pseudonym credential, and VESNOS's recipe for it is mathematically right.

The check was shown to fail. Each of three injected faults turns it False:
- one hex digit of the private reconstruction value;
- the wrong y parity on P_U;
- i = 0x7B instead of 0x7A.

The values are not in any branch of `conz27/crypto-test-vectors`. Their source is probably an ISS test batch, perhaps through the USDOT SCMS proof-of-concept EE that the paper cites; this was not confirmed.

### 3.3 Butterfly key expansion and implicit certificates

- **Butterfly expansion** (`bfkeyexp.h:102-171`) is computed **once per vehicle at initialisation**, for the single pair (i, j) = (0x7A, 0). It is followed by ECQV private-key reconstruction (`bsmsign.h:147-171`, `implicit.h:119-141`).
  - The check that the reconstructed key pair matches is skipped, because the optional PCA key argument is `nullptr` (`bsmsign.h:163`).
  - There is no caterpillar or cocoon request, no RA, no RA shuffle, no encryption key expansion (the `"enc"` branch at `bfkeyexp.h:154` is never called), no batch and no download.
- **Implicit-certificate issuance** (`implicitCertGen`, `implicit.h:83-116`) is never called.
  - It is also broken. `generate_random_cpp_int` declares a second `hexValue` inside its loop (`implicit.h:72`), so it returns the parse of an empty string. Its seed is fixed at 333 (`implicit.h:91`).
- **ECQV public-key reconstruction** runs at the receiver for every BSM (`bsmverify.h:513-552`).

### 3.4 What a vehicle signs and sends

`ApplVBeacon::sendBeacon` (`01_Beacon.cc:86-95`) now sends a `BSM` instead of the VENTOS beacon. The `send(beaconMsg, …)` line is commented out at line 93.

`generateBSMMessage` (`01_Beacon.cc:198-277`) does the following:
1. **The "encoding" is the pointer.**
   - `encodedSize = sizeof(wsm)`, where `wsm` is a `BeaconVehicle*`. Then `memcpy(encodedBuffer, &wsm, encodedSize)` copies the **8-byte address of the beacon object** (lines 231-237; the comment reads "Replace with actual size calculation").
   - The beacon object is never deleted, so it leaks once per beacon, at 10 Hz in the examples.
   - No J2735 BSM is built.
2. **The 1609.2 structure is concatenated by hand as hex text** (`bsmsign.h:66-120`), mimicking the COER layout of a CAMP/ISS sample:
   - `038100` + `400380` + length + payload (the 8 address bytes) + `40` + `0100` + generationTime + signer HashedId8 + signature;
   - then the decimal lengths of R and s (`"6464"`) are appended as a non-standard trailer, so that the receiver can parse it (lines 117-118).
3. **The header fields are wrong.**
   - `psid` encodes as 0 (`"0100"`, line 80), while the certificate permits 0x20 and 0x26. A conformant receiver would refuse the message.
   - `generationTime` is simulation milliseconds (`01_Beacon.cc:245`), not 1609.2 Time64 microseconds since 2004.
4. **The digest** is 1609.2's H(H(tbs) ‖ H(signer)) (`bsmsign.h:36-45`). The signer is identified by digest (`80` + HashedId8, line 101).
5. **The frame** is an OMNeT++ packet whose string fields carry the following (`01_Beacon.cc:270-274`):
   - the signed hex;
   - the full pseudonym certificate;
   - the full PCA certificate;
   - **the PCA public key**.

   Its bit length is set to `headerLength + beaconLengthBits`, which is 88 + 256 bits by default (`baseAppl/modules.ned:29,32`; `01_Beacon.cc:255-258`). So **the radio sees a 43-byte frame whatever the security content is**.

### 3.5 What a receiver checks

`ApplVManager::onMessageType`, BSM branch (`Manager.cc:212-288`):
- **The trust anchor comes from the message.**
  - `pca_pub_x = wsm->getPublic_key()` (line 220), and `ECPoint2 pca_pub("compressed-y-0", pca_pub_x)` (line 223).
  - The receiver reconstructs the pseudonym key from the sender-supplied PCA key and the sender-supplied certificate, then checks the ECDSA signature (lines 235-237; `bsmverify.h:555-566`).
  - So **anyone can forge**: make a PCA key pair, issue yourself an implicit certificate, and attach your PCA key.
- **It checks nothing else.**
  - There is no chain to a root and no CTL.
  - It does not check validity (the certificate expired in 2017), region, PSID against appPermissions, the CRL, generationTime, replay or duplicates.
  - The issuer HashedId8 inside the certificate is never compared with the attached PCA certificate.
- **It does nothing with the result.**
  - On success it recovers the payload by copying the 8 bytes back into a `BeaconVehicle*`. It then dereferences **the sender's heap object** and prints its fields to the log (lines 248-273).
  - This works only because every node shares one address space, and because the object leaked.
  - On failure it prints "ERROR: Failed to verify BSM" (line 281).
  - **In neither case is the message dropped, counted, reported or passed to `onBeaconVehicle`.**

### 3.6 What signing broke in the rest of VENTOS

- **The applications lose their input.** Vehicles no longer send `TYPE_BEACON_VEHICLE`, and a verified BSM is not dispatched (§3.5). So platooning, CACC, AID and incident detection, which consume `onBeaconVehicle` (`Manager.cc:312-…`), receive no V2V beacons once vehicles beacon. The radio still carries BSMs.
- **RSUs throw.**
  - `ApplRSUManager::handleLowerMsg` handles no `TYPE_BEACON_BSM` and throws "RSU … received unsupported msg" (`rsu/Manager.cc:97-137`).
  - The `v2x` example turns on vehicle beaconing (`examples/v2x/omnetpp.ini`, `sendBeacons = true`) and places six RSUs that default to `DSRCenabled = true` (`src/nodes/rsu/modules.ned:14`).
  - By static reading, the first BSM an RSU decodes ends that run. This was not executed.
- **The built-in attacks miss.**
  - `ApplAdversary` falsifies (sets acceleration to 6 m/s²) or replays (after 10 s) only `TYPE_BEACON_VEHICLE` (`ApplAdversary.cc:111-157`). With VESNOS vehicles those attacks have nothing to act on.
  - Jamming is not a radio model. The adversary sets a boolean `jamming` on every vehicle in range, and vehicles then drop all received messages (`ApplAdversary.cc:160-216`, `Manager.cc:103-108`).
  - The `security_adversary` example runs this jamming (`attackMode = 2`) against a 10-car CACC platoon.
  - **No released configuration forges, alters or replays a signed BSM.**

### 3.7 SCMS entities

| Entity | In VESNOS? | Evidence |
|---|---|---|
| Root CA, electors, CTL, ICA | No | No code |
| ECA, enrolment | No | No code |
| RA (incl. shuffle), PCA/ACA, batch download | No | No code. The only issuance function is unused (§3.3) |
| LA1/LA2, linkage values, pre-linkage values | No | CAMP `lv.py` not ported. Linkage values exist only as bytes inside the hard-coded certificate |
| MA, misbehaviour detection, misbehaviour reports | No | No code |
| CRL Generator | No | See below |
| Multiple providers | No | One PCA, one certificate |

**The "CA" is VENTOS's CRL-distribution study** (`src/nodes/CA/ApplCA.cc`, unchanged from VENTOS):
- **The CRL contents are dummies.** `createCRL` builds `CRLsize` C++ objects `Certificate{CerName="Node%d_Certificate", CAname="CA0", CAid=123456789, NodeName="V[x]", NodeID}` (`ApplCA.cc:196-218`; `Certificate.h:37-57`) and serialises them with Boost text archives (lines 220-223).
- **The erasure code is not real.** It is done in single-precision floats with Eigen (`ApplCA.cc:327-347`). The GF(256) version is commented out ("todo: uncomment this", lines 349-363).
- **Delivery to the RSUs is magic.** Pieces go to the RSUs by OMNeT++ signals, which is instantaneous and has no network (`ApplCA.cc:438-480`). Each piece is sized at 2,300 B (line 427).
- **RSUs broadcast pieces** under VENTOS's algorithms (`rsu/05_CRL.cc`: RSU-only, C2C epidemic, MPB, ICE, ICEv2).
- **Vehicles discard every piece** (`Manager.cc:207-211`: "// todo … delete msg").
- **The vehicle request path is dead.** It is the `Magic_Req` signal, to which the CA subscribes (`ApplCA.cc:98-99`) but which nothing ever emits.
- **"EnableShuffle" shuffles CRL pieces** (`ApplCA.cc:483-496`). It is not the RA's request shuffle.
- **No CRL entry relates to any certificate a vehicle uses**, and no receiver checks one.

### 3.8 Security in the network model

- **Size.** Security adds no bits: the frame length is fixed (§3.4). So channel load, PDR and latency are those of an unsigned 43-byte beacon.
- **Time.** Signing and verification take wall-clock time in the C++ process and zero simulated time. There is no `scheduleAt` or delay on the path, and no hardware cost model.
  - Per BSM, signing does two full 256-bit scalar multiplications plus one 31-bit one (`bsmsign.h:57`, `ecc.h:760`, `ecc.h:771`).
  - Each reception does three full ones (`bsmverify.h:528`, `bsmverify.h:464`). Every point construction re-checks the curve equation.
  - No timing is reported in the repository.
- **Metrics.** None for security. VENTOS's `Statistics` records beacons, PHY/MAC frames and platoon events only. Nothing counts verified, failed or dropped messages.

### 3.9 Radio

- **The stack is VENTOS's copy of Veins 4-era MiXiM** (`src/MIXIM_veins`): `PhyLayer80211p`, `Decider80211p` with the NIST error rate, `Mac1609_4` with EDCA and channel switching. Analogue models available: simple and breakpoint path loss, two-ray interference, log-normal, Jakes and Nakagami fading, simple obstacle shadowing.
- **The shipped examples configure free-space-like path loss only**: `SimplePathlossModel`, α = 2, 5.89 GHz (`examples/config.xml:5-6`).
- **There is no C-V2X**, although the manual says VESNOS supports "DSRC-enabled wireless communication and C-V2X" (manual p4). No LTE-V2X or NR code exists.

### 3.10 Traffic and scale

- **Traffic.** SUMO through VENTOS's TraCI client, with VENTOS's SUMO fork (`VENTOS_SUMO`, checked by `runme:1218-1222`) for its ACC/CACC car-following models. It supports multi-modal traffic, platoon management and adaptive signal control. All of it is VENTOS.
- **Scale in the shipped security examples:**
  - `v2x`: 8 vehicles and 6 RSUs (`examples/v2x/addNode.xml`);
  - `security_adversary`: a 10-car platoon and a jammer;
  - `security_CRL_dist`: 3 vehicles, 6 RSUs and a CA.
- **No scale or run-time figures** are in the repository. The paper's are unknown.

### 3.11 Determinism and reproducibility

- **The simulation is seeded.** OMNeT++ uses Mersenne Twister with `seed-0-mt = 0` (`examples/omnetpp_general.ini:27-29`), so OMNeT++ runs are deterministic. Signature values come from the unseeded `rand()` (§3.1) and do not affect outcomes, since verification results are only logged.
- **There is nothing to check reproducibility with:** no determinism claim, no test, no CI, no recorded outputs.
- **The build is old and takes manual steps.** It needs Ubuntu 16/18 or macOS 10.10–10.12, OMNeT++ 5.4.1, the VENTOS SUMO fork, Boost, OpenSSL, Crypto++, Shark and gtkmm (manual pp. 6-9; `runme`). The manual warns that installation "might take more than an hour".

### 3.12 Validation

- **There is no test, test vector or comparison in the repository.** The only external artefact is the hard-coded ISS test certificate set. Its internal consistency is confirmed by this review (§3.2), not by VESNOS.
- **No receiver rule is tested, because none exists** (§3.5).

### 3.13 Case studies and metrics (paper)

- **The case study is known only from an excerpt.** A search-engine excerpt of the ResearchGate copy says the case study runs "a coupled simulation of CVs among traffic stream platoon" and shows a receiver vehicle rejecting a false message through its signature and pseudonym certificate. The abstract says case studies show the platform is "an appropriate and trustworthy testbed".
- **No released configuration reproduces it.** None produces a false signed BSM (§3.6), and rejection has no effect beyond a log line (§3.5). The case study therefore used code or settings that are not in the repository, or its "rejection" is that log line.
- **The paper's own metrics and numbers are unknown** until the full text is read.

### 3.14 Performance

Nothing is measured in the repository. By construction (§3.8), the cost per BSM is several big-integer scalar multiplications in unoptimised affine arithmetic, multiplied by the number of receivers. That is a plausible reason the examples stay at about 10 vehicles. This is an inference; no number exists to support it.

### 3.15 Licence and maintenance

- **Licence.** There is no licence file, and GitHub reports none. The file headers are VENTOS's GPL-2.0-or-later. VENTOS ships GPL-3.0 (`license.txt`, which VESNOS removed). Treat VESNOS as GPL: read it, never copy it.
- **Activity.** 9 commits, from 2024-01-11 ("first commit") to 2024-05-09. Most are "Add files via upload" or README edits, and nothing has been pushed since. 2 stars, 1 fork (pushed 2026-02-10, not inspected), 0 issues.
- **Citations.** OpenAlex counts 4 citing works: a mixed-reality SDR VANET testbed, a multi-application simulation platform, a V2V attack study and a cyber-range survey. Judging by their titles, none reuses the SCMS code.
- **Odd extra.** The installer also installs an IPFS binary (`runme:712-718`) that no source file uses.

### 3.16 Limitations, as found in the code

1. A single shared, expired test certificate. No pseudonyms, privacy or accountability.
2. No SCMS backend entity or flow.
3. The receiver trusts a key supplied in the message, and checks nothing else.
4. A memory address is signed instead of a BSM. No J2735.
5. Security is invisible to the radio, to timing and to metrics.
6. Signing disconnects the beacon from the applications, and RSUs crash on BSMs.
7. The attacks cannot target the signed path.
8. The CRL is VENTOS dummy data, and vehicles discard it.
9. The radio is 802.11p only, with free-space path loss in the examples. There is no C-V2X.
10. No tests, no validation, no licence file. Abandoned since May 2024.

---

## 4. Item-by-item comparison with this engine

"Kind" says what the difference is worth in a paper:
- **R**: research-relevant. The engine can pose a question VESNOS cannot.
- **E**: engineering only. It is better, but reviewers will not count it as novelty.
- **V**: VESNOS is equal or better.

| Item | VESNOS (evidence) | This engine (evidence) | Kind |
|---|---|---|---|
| SCMS entities that run | None (§3.7) | Electors with 3-of-5 CTLs on P-384, root, ICA, ECA, RA, ACA, LA1/LA2, MA, CRL Generator and DCM per provider, two providers (`06-validation-summary.md` §3 items 2, 4, 6; README) | **R** |
| Crypto library | Hand-rolled `cpp_int` ECC; 31-bit ECDSA nonce from `rand()`; OpenSSL for SHA-256 only (§3.1) | RustCrypto `p256`, `p384`, `aes`, `ccm`, `sha2`, `hmac` (`crates/v2xw-sec/Cargo.toml`) | E |
| 1609.2 encodings | Hex strings concatenated by hand after one sample; wrong PSID and generationTime; a non-standard trailer (§3.4) | COER from the 1609.2/1609.2.1 ASN.1 (`rasn`). The oracle re-encoded all 54,446 recorded PDUs canonically (06 §3 item 1) | E (R where sizes drive the result, as in PQ) |
| Implicit certificates | ECQV reconstruction at every receiver, for one certificate (§3.3, §3.5) | ECQV and explicit certificates, issuance and reconstruction, validated against CAMP and 1609.2a vectors (06 §3 item 1) | E. VESNOS already does reconstruction |
| Butterfly keys | Expansion once per vehicle at start-up, one (i, j), no flow (§3.3) | The full provisioning flow: caterpillar, cocoon, RA shuffle, ACA encryption to the cocoon key, batch download with retries, `Range` resume and top-ups. Vectors #63–70 (06 §3 items 1, 4, 10, 11) | **R** for provisioning questions (HNLL, provisioning load); E for "has butterfly keys" |
| Pseudonyms | One certificate, shared by every vehicle, never changed (§3.2) | Per-device pools (20 valid at every census), change policies, the real weekly calendar (06 §3 items 4, 15) | **R** (Approach 3) |
| Linkage values, LA split | None | `crates/v2xw-sec/src/linkage.rs`; threshold LA in `v2xw-proto`; coalition matrix (`docs/redteam/pki-privacy.md` §2.1) | **R** |
| Misbehaviour detection | None | TS 103 759 classes 1–5, the US SM-MBR profile, legacy checks (`crates/v2xw-threat/src/ts103759.rs`; 06 §3 item 13) | **R** |
| Misbehaviour reports and MA | None | Signed SM-MBR uploads through the RA; a corroborating MA with an 8 h review (`ma_corroborate.rs`; 06 §3 items 7, 10) | **R** |
| CRL generation and distribution | VENTOS dummy objects, a float "erasure code", magic CA→RSU delivery; vehicles discard pieces (§3.7) | Linkage-seed composite CRLs from t0, `nextCrl` renewal, business-day slots, conditional polls, RSU rebroadcast, receivers enforcing the overdue rule (06 §3 items 7–9) | **R** |
| RA shuffle | None. "EnableShuffle" shuffles CRL pieces | Modelled; one knob still partial (06 §2.2, knobs) | **R**, minor |
| Multiple providers, electors, CTL | None | Yes, checked against the six published SCMS Manager CTLs (06 §2.2) | **R** for trust and agility questions; E otherwise |
| Receiver validation | Signature only, against a PCA key the sender supplies (§3.5) | Chain to an installed CTL root, validity, region, SSP/PSID, CRL, generationTime window, the validated-certificate cache (06 §3 item 12; `crates/v2xw-node/src/rxval.rs`) | **R**: attacks that pass or fail real checks |
| Signing and verification per message | Yes, real (hand-rolled) on every BSM, at every receiver | Real or modelled (`crypto_mode: modeled` in most shipped scenarios). The I-S1 test shows both give identical records (06 §3 item 4) | **V on this row only**: VESNOS computes every signature, while the engine's large runs model them. Both are defensible |
| Cost of security on the channel and in time | Zero bytes, zero simulated time (§3.8) | Security bytes on the air (46 % of BSM bytes on the realism base); sign and verify priced from OBU hardware profiles; verify-all, on-demand and prioritised policies (06 §3 item 14; bench §0) | **R** (overhead and PQ studies) |
| Radio | Veins 4-era 802.11p; examples use α = 2 path loss; no C-V2X (§3.9) | 802.11p, LTE-V2X Mode 4 and NR-V2X Mode 2 on one channel model; windowed SINR; correlated shadowing; cross-validated against Veins and OpenCV2X (README; `docs/design/frameworks/omnet-veins-artery.md` §3.1) | **R** for SPS questions (Approach 3); E for the 802.11p refinements |
| Traffic | SUMO (VENTOS fork) through TraCI | Own engine (IDM/MOBIL and others), cross-validated against SUMO; optional SUMO over TraCI (README) | **V**: SUMO is the reviewers' default and needs no defence. The engine's own traffic model has to be defended |
| Applications in the loop | CACC/platoon (VENTOS), but disconnected once vehicles sign (§3.6) | CACC/platoon fed by received messages. FCW, EEBL and IMA warnings do not move vehicles yet (`final-three-approaches.md` §3) | Mixed. Neither has a warning-response driver model |
| Attacks | Falsify and replay on unsigned beacons (inert in VESNOS); jamming as a flag (§3.6) | Insiders with valid credentials, Sybil, replay, GNSS, compromised RSUs, report poisoning, radio jamming (README; `attack.rs`) | **R** |
| Scale achieved | About 10 vehicles in the shipped examples; no figures | 1,000–1,632 nodes for 155 simulated seconds before a 2.5 GB memory cap; 1.74 wall-s per simulated second at 145 vehicles (bench §0; `docs/validation/test-run-cost.md`). **The full-fleet two-day run has never finished** | E, with an honest caveat on both sides |
| Determinism | OMNeT++ seeded; no claim, no test | Byte-identical records at 1 and 4 threads, with a check that can go red (06 §3 item 16) | **E**: OMNeT++ is deterministic too. Paired "counterfactuals" were already demoted (`final-three-approaches.md` §5) |
| Validation | None (§3.12) | 412/412 official vectors; independent Python oracle; 279,236 of 279,236 runtime checks on HEAD runs; 275 of 326 real-world rules match (84.4 %) (06 §2) | **R as credibility** (it makes results believable); E as a contribution |
| Metrics | VENTOS beacon, PHY/MAC and platoon statistics; none for security | 91 metrics, including security processing, certificate supply, revocation latency, detection quality and linkability (README) | E, except the security metrics that the R rows need |
| Case studies | One platoon "false message rejected" (paper, unverified); jamming on a platoon; CRL broadcast | Realism benchmark, 32-run matrix, multi-week runs, PQ at density, red team (06; bench) | n/a |
| Licence and maintenance | No licence file (GPL-derived); 9 commits; inactive since 2024-05 | Apache-2.0; active | E |
| Publication | Peer-reviewed (PADS 2024), open source | Unpublished | **V**: it is prior art, and the engine is not yet citable |

---

## 5. Where VESNOS is equal or better

This is the harsh part:
1. **It is published, and it owns the generic claim.** A reviewer who has not read the code will accept "SUMO + OMNeT++ + SCMS" from the abstract. The engine has no paper.
2. **It runs on the community's stack.** Veins and SUMO results are accepted without argument. The engine's own traffic model and PHY must be defended through its cross-validation reports every time.
3. **It computes every signature.** The engine's shipped scenarios model cryptography and rely on the I-S1 equivalence test. A reviewer may ask why the engine's own large runs do not sign.
4. **Its CACC platoon models come from VENTOS's SUMO fork** and from a line of published attack studies (amoozadeh2015security). Closed-loop attack effects on platoons are prior art, independent of the SCMS.

What VESNOS does **not** have equal or better: anything SCMS-specific beyond a single signature check.

---

## 6. Our advantages, split

**Research-relevant** (each one is needed to pose a question VESNOS cannot pose):
1. **The SCMS backend running in the loop**: issuance and provisioning flows, linkage values and the LA split, MBR upload, a corroborating MA, linkage-seed CRLs on the real calendar, CTLs from electors, multiple providers. This is needed by Approaches 1–3 and by research-plan D1–D3.
2. **Real receiver validation.** It makes attacks that pass or fail real checks meaningful: GNSS framing, relays, insiders with valid credentials.
3. **Many pseudonyms per device, change policies and a passive observer** with ground truth (Approach 3). Caveat: the observer's link counting was corrected only in fix wave 3, and no change-rich linkability result exists yet (`testing/measure.md` §2).
4. **Security bytes and crypto time on the channel**, plus verification policies. This is needed by any overhead or PQ study, and VESNOS has neither.
5. **LTE-V2X and NR-V2X SPS MACs** (Approach 3's reservation-continuity leak).
6. **Detectors and attackers with credentials** (TS 103 759, SM-MBR, insiders).
7. **Validation as credibility**: vectors, oracle, real CTLs. It is not a contribution in itself. It is what lets a reviewer trust a revocation or linkability number.

**Engineering-only** (true, but not novelty):
1. Rust, single process, byte-identical across thread counts. OMNeT++ is deterministic too.
2. RustCrypto and `rasn` instead of hand-rolled arithmetic and hex strings.
3. The finer 802.11p PHY (windowed SINR, correlated shadowing, NLOSv), when the question is not about radio.
4. J2735/ETSI ASN.1 messages, except where message size drives the result.
5. The Studio, the agent, 64 maps, about 300 settings, 91 metrics, the compare and sensitivity tools.
6. The licence, documentation and test discipline.
7. Scale, where the claimed result does not depend on density. The full-fleet run is also unfinished, so scale is not yet a usable advantage.

---

## 7. Claims: which survive, die or get stronger

### 7.1 Claims about the engine as a simulator

| Claim | Before this read | Now | Why |
|---|---|---|---|
| "First simulator integrating an SCMS with traffic and network simulation" | Dead (abstract) | **Dead** | VESNOS's title and abstract claim it, and the code does sign with an SCMS-issued certificate format |
| "First open simulator in which the SCMS **backend** runs in a traffic and radio loop": issuance, linkage, MBR→MA, linkage-seed revocation, CTL | Pending a full read | **Survives, stronger** | VESNOS runs no backend entity (§3.7). mushrall2018emulab has no traffic or radio. chen2023performance and chen2026comparison have no traffic or radio. A4MD is ETSI with no US SCMS. Still check SAE and industry tools before writing "first" |
| "Real 1609.2-signed messages in co-simulation" | Dead (Artery/Vanetza) | **Dead**, now twice over | VESNOS also signs, with a real certificate format |
| "First with butterfly keys or implicit certificates in a V2X network simulator" | Not checked | **Dies** | VESNOS expands one butterfly key per vehicle and reconstructs ECQV keys at every receiver |
| "Butterfly **provisioning flow** in a network simulator, validated against CAMP vectors" | n/a | **Survives** | VESNOS has no flow and no vectors |
| "SCMS fidelity validated against official vectors and the real SCMS Manager trust lists, in a closed loop" | Survived (pending) | **Survives, stronger** | The only prior SCMS co-simulator has no tests, an expired shared certificate and a receiver that trusts in-message keys. This is a credibility contrast, not a headline |
| "Security overhead (bytes and time) modelled in an SCMS co-simulation" | n/a | **Survives narrowly** | VESNOS adds 0 bytes and 0 time. Overhead studies in other simulators exist (petho2024investigating and others in `claims-we-cannot-make.md`), so claim the combination, not the overhead |
| Determinism as a distinguishing feature | Already demoted | **Engineering only** | OMNeT++ is deterministic too |
| "Closed-loop attack-to-platoon harm" | Dead (amoozadeh2015security and others) | **Dead** | VENTOS lineage |

### 7.2 The research directions

| Direction (file) | Verdict after the full read | Reason |
|---|---|---|
| Approach 1, framability under a proof-based MA (`final-three-approaches.md` §2); = research-plan D3 | **Survives unchanged** | VESNOS has no detector, no MBR, no MA and no LA. Its risks are where they were: park2024enhancingb, SAE J3287 and TS 103 759 v2.2.1, all still unread. The engine's own R-USFP result (26 honest revocations from raw GNSS noise, falling to 0 with a smoothed receiver; 06 §3 item 13) is direct support for RQ1.3, and no VESNOS-class tool could produce it |
| Approach 2, pricing detector errors (§3) | **Survives unchanged** | No detectors in VESNOS. The closed-loop platoon prior art (VENTOS, Plexe) was already listed. The blocking prerequisite is still the engine's missing warning-response driver model, not anything in VESNOS |
| Approach 3, who can follow an honest vehicle (§4) | **Survives unchanged** | VESNOS has one shared pseudonym and no C-V2X, so no privacy question can be posed in it. The open risks are SAE J3161/1, chipset reselection practice and the engine's own observer (corrected in fix wave 3, not yet re-validated on change-rich runs) |
| Research-plan D1, harvest now, link later (`pqc-direction-final.md`) | **Survives unchanged** | VESNOS has no provisioning channel, no ECIES or TLS, and no PQ |
| Research-plan D2, SCMS-Bench | **Survives, slightly stronger** | VESNOS is the closest "SCMS + simulation" tool and cannot score any part of the detector→MBR→MA→CRL loop. "No open tool closes that loop" can now cite code. The competitors that matter remain VeReMi NextGen (Ulm), BSMAttacker and F2MD |
| Runner-up C, resilience and crypto-agility | **Survives** | VESNOS has no root, no CTL and no electors |

**Nothing gets weaker because of VESNOS.** The only new constraint concerns wording: never write "first SCMS simulator", "first co-simulation with SCMS" or "first with butterfly keys or implicit certificates in a network simulator".

### 7.3 Is there a paper in "the engine versus VESNOS"?

- **As a security paper: no.** Being a more complete simulator is engineering.
- **As a tool paper: possibly.** It would go to SIGSIM-PADS (VESNOS's venue), ACM TOMACS or WSC, under a title like "a validated US SCMS backend in a city-scale V2X simulator".
  - It would have to lead with a result that VESNOS-class tools structurally cannot produce. The best existing candidate is R-USFP: a standards-default detector profile revoking honest vehicles because of GNSS receiver output. The revocation window on the real calendar is a second one, but `critique-framed.md` §2.6 warns that its stages are policy arithmetic.
  - It would also have to show validation numbers, not features.
  - Do it only as a companion to one of the research papers, and only after the engine is public: the paper needs an artefact.

---

## 8. How to cite VESNOS

- **Describe what it is.** Cite it as the closest prior co-simulation platform that adds 1609.2-style message signing with SCMS-format certificates to SUMO and OMNeT++.
- **Back the contrast with the commit.** State the contrast factually and pin it to the code: "as released (commit `997913b5`), VESNOS signs beacons with a single test-PKI pseudonym certificate shared by all vehicles and runs no SCMS backend entity, so issuance, linkage, misbehaviour reporting and revocation cannot be studied with it."
- **Avoid two things.** Do not call it broken in a submission. Do not list its defects (pointer signing, trusted in-message key, RSU crash) unless a reviewer asks; they are code-quality points and can read as hostile. Keep them in this file as evidence.
- **Read the paper first.** Get the full text through a library or the authors before submission, and check whether it claims features absent from the code, such as pseudonym change or CRL checking. If it does, cite the code commit alongside the paper.

---

## 9. Side finding for the engine (CF-IPERIOD)

The hard-coded ISS test pseudonym certificate (§3.2) has iCert 122 and starts at Time32 **421,405,200**. Three ways of reading the start time give different answers:

| Reading | Start for i = 122 |
|---|---|
| The certificate itself (= 2015-01-06 09:00 UTC on a leap-second-free count, 347,619,600, plus 122 weeks) | 421,405,200 |
| The engine's candidate uniform Time32 grid from the epoch's Time32 (347,619,603) | 421,405,203 |
| The engine's current UTC 09:00 boundary as a TAI instant (+2 s, `validation/crypto-formats.md` row 10) | 421,405,205 |

**Neither of the engine's two candidates matches this artefact.** The ISS test PCA counted Time32 without leap seconds, that is, UTC seconds since 2004, POSIX-style.

This is one 2017 test certificate, so it is not proof of production practice. It is a third data point for the open owner decision CF-IPERIOD. Before choosing, check production pseudonym certificates if any are available.

---

## 10. Owed

- **Read the paper.** Its full text is blocked by ACM's Cloudflare (403). Get it through a library or interlibrary loan, or ask the authors. Then check §3.13 and whether the paper claims more than the code.
  - **Second attempt, 2026-10-23 (`check-realdata-and-vesnos.md` §5).** Still blocked: ACM pdf/epdf/fullHtml/`?download=true` 403; Semantic Scholar PDF cache empty; no eScholarship, NSF PAR or arXiv copy; Wayback and scholar.archive.org unavailable. Search-engine excerpts (unverified) say the paper models SCMS "bootstrapping, certificate provisioning, misbehavior reporting, and CRL retrieval" and measures "detection performance against the man-in-the-middle attack". Commit `997913b5` implements none of those four flows (§3.7). So the paper likely **claims more than the code**: cite the paper with the commit, and do not claim "first misbehaviour reporting in a co-simulator".
- **Correct three other files.** They should be corrected in their own commit; this commit changes only this file and the summary:
  - `claims-we-cannot-make.md`, row "First simulator integrating an SCMS…": replace "(pending a full read of VESNOS)" with the narrowed backend-in-the-loop claim in §7.1, citing this file. Add "first butterfly keys or implicit certificates in a network simulator" as a claim we cannot make.
  - `landscape.md` §5, row abdo2024vehicular: PKI backend "none (one shared test certificate; signing only)"; revocation in loop "no"; determinism "OMNeT++ seeded".
  - `novelty-check.md`, lines 52 and 277: abdo2024vehicular is now read and does not affect the revocation-loop verdict.
- **Not inspected:** the one fork, `NishantDakua/VESNOS`, pushed 2026-02-10.

---

## Appendix A. The credential consistency check (§3.2)

Run with `python3 -I vesnos_vector_check.py`. It needs the `cryptography` package for AES-128. The values are transcribed from `src/nodes/vehicle/01_Beacon.cc:281-338` at `997913b5`. Output on 2026-10-23:

```
PCA name in issuer cert: b'v2xpca-test.ghsiss.com'
reconstructed private key d: 0x962f3f0ef867224eb10d42d84b5562797eb0eacbc5997ae6997c655fcc080ed
d*G == e*P_U + Q_PCA (key pair consistent): True
HashedId8 of pseudonym cert: 2adc2bdf93cfe301
HashedId8 of PCA cert: da766b0e278fd23d (pseudonym issuer field: da766b0e278fd23d)
```

```python
# Independent check of the credential set hard-coded in VESNOS src/nodes/vehicle/01_Beacon.cc:281-342.
# Pure-Python P-256 + AES-128 (cryptography lib). Values transcribed from the VESNOS source.
import hashlib
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

p = 0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF
a = p - 3
b = 0x5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B
n = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
G = (0x6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296,
     0x4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5)

def add(P, Q):
    if P is None: return Q
    if Q is None: return P
    if P[0] == Q[0] and (P[1] + Q[1]) % p == 0: return None
    if P == Q: l = (3 * P[0] * P[0] + a) * pow(2 * P[1], -1, p) % p
    else: l = (Q[1] - P[1]) * pow(Q[0] - P[0], -1, p) % p
    x = (l * l - P[0] - Q[0]) % p
    return (x, (l * (P[0] - x) - P[1]) % p)

def mul(k, P):
    R = None
    while k:
        if k & 1: R = add(R, P)
        P = add(P, P); k >>= 1
    return R

def decompress(x, ybit):
    y = pow((x**3 + a*x + b) % p, (p + 1) // 4, p)
    if y & 1 != ybit: y = p - y
    assert (y*y - (x**3 + a*x + b)) % p == 0
    return (x, y)

h = lambda s: bytes.fromhex(''.join(s.split()))
pca_cert = h("""8003 0080 fabd 443d bf85 85fa 5981 1676 3278 7063 612d 7465 7374 2e67 6873 6973
 732e 636f 6d5e 6f5b 0002 18f3 4861 8600 0a83 0103 8000 7c80 01e4 8003 4801 0180
 0123 8003 8500 0101 0100 8001 0200 0120 0001 2600 8082 42ac 6bc3 42c4 93d2 a6a8
 2169 fc25 2ebf 6c86 ba6a 3285 b143 2376 1a43 de15 ff80 8080 827c 5c5a d2e4 4129
 9c7e 87cd 60f4 05dd 4de6 8e46 e7ed 1239 dd9e 8e39 188f a57f ef80 8000 e93d b970
 f630 d6f5 c4f0 a9e2 7a57 85f1 43e3 e82f 9090 a76a 882f 08c6 3f79 51ec b93a c48b
 4f5b 6aac b052 35c8 230b 5c2a b624 f0df 36cb f0f0 2f33 01b9 cf5f 69""")
pseudo_cert = h("""0003 0180 da76 6b0e 278f d23d 5080 8000 7a8e 4d44 3b14 03b3 9ffc 0000 000f 8e4d
 443b 1403 b39f fc5e 6f5b 0001 191e 2210 8400 a983 0103 8000 7c80 01e4 8003 4801
 0200 0120 0001 2681 837a 06e6 dab3 cb6c c0b3 7657 1681 7212 3854 690a de9a d8e7
 f1aa 9286 6fc6 c7bd 79""")
tbs = pseudo_cert[12:]
pca_pub_x = int("7c5c5ad2e441299c7e87cd60f405dd4de68e46e7ed1239dd9e8e39188fa57fef", 16)
pub_recon_x = int("7a06e6dab3cb6cc0b37657168172123854690ade9ad8e7f1aa92866fc6c7bd79", 16)
prv_recon = int("08fa4ce52c68b12bb8baf94a15d57aedc82bf842799775ec520ac28b31e7d907", 16)
seed_prv = int("46555a862db44758e8a9cbcbb0abaec6bf91d38dac2411f53f5918674a1cb1ad", 16)
exp_key = h("9d53 e9d9 626e 647c edd7 bd6a a7fd e192")
i, j = 0x7A, 0

print("PCA name in issuer cert:", pca_cert[15:15+22])
# butterfly f_k(x_cert), IEEE 1609.2 / CAMP bfkeyexp
x = ((i << 32) + j) << 32
enc = Cipher(algorithms.AES(exp_key), modes.ECB()).encryptor()
blocks = b''.join((int.from_bytes(enc.update(((x + t) % 2**128).to_bytes(16, 'big')), 'big') ^ (x + t)).to_bytes(16, 'big') for t in (1, 2, 3))
bf_prv = (seed_prv + int.from_bytes(blocks, 'big') % n) % n
# implicit-cert reconstruction, e = H(H(tbs) || H(issuer cert))
sha = lambda d: hashlib.sha256(d).digest()
e = int.from_bytes(sha(sha(tbs) + sha(pca_cert)), 'big')
prv = (e * bf_prv + prv_recon) % n
Q = mul(prv, G)
Qrec = add(mul(e, decompress(pub_recon_x, 1)), decompress(pca_pub_x, 0))
print("reconstructed private key d:", hex(prv))
print("d*G == e*P_U + Q_PCA (key pair consistent):", Q == Qrec)
print("HashedId8 of pseudonym cert:", sha(pseudo_cert)[-8:].hex())
print("HashedId8 of PCA cert:", sha(pca_cert)[-8:].hex(), "(pseudonym issuer field: da766b0e278fd23d)")
```
