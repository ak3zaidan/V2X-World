//! IEEE Std 1609.2a-2017 Annex D.6 cryptographic test vectors, all 21 of them, against
//! the primitives in `v2xw_sec::{ccm, ecies, kdf}` (WP-A2,
//! docs/design/scms-real/00 §4.2; gap rows 7 and 8).
//!
//! # Provenance (values transcribed, nothing fetched at test time)
//!
//! - **S4**: IEEE Std 1609.2a-2017, Annex D.6 (pp. 96–121), as hosted by ETSI:
//!   https://docbox.etsi.org/STF/Archive/STF538_TC_ITS/STFworkarea/libaries/IEEE_Std_1609_2a-2017.pdf
//!   (sha256 `aae52bc1ab7101a1993c2282e157086cfefa1e44146ee93e61104c3ece19bc48`).
//!   D.6.1 AES-CCM ×6, D.6.2 ECIES ×4, D.6.3 MAC1 ×7, D.6.4 KDF2 ×4 inputs. Every value
//!   prints twice in the annex (a hex string and a C byte array); the transcription
//!   script compared the two for every value and found them equal.
//! - **ECIES P1 erratum** (02 §8.2): the annex, like CAMP `ecies.txt`, prints `P1 = k`.
//!   The real P1 values are the `p11`/`p12` constants of CAMP `ecies.py`
//!   (https://github.com/conz27/crypto-test-vectors, commit `dd5014e6`, file sha256
//!   `49f8927753b978f88ccf0dcb7c3b581f678e0df33cdad2d31d15e2102efb5e2e`, lines 119 and
//!   125). With them every printed `C` and `T` reproduces; with the printed P1 none does
//!   ([`d6_2_the_printed_p1_does_not_reproduce`]).
//! - **D.6.4 outputs**: the annex text gives the inputs; the derived keys did not survive
//!   PDF text extraction. The four inputs are NIST CAVP SP 800-135 ANS X9.63 vectors, so
//!   the outputs are transcribed from `ansx963_2001.rsp` `[SHA-256]`
//!   (https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/components/800-135testvectors/ansx963_2001.zip,
//!   sha256 `6aa7c067660f388fd2418c010bf592ed856a50761e4acf384bc7e0db3f20d161`), matched
//!   by shared secret, and they equal the `known_key1..4` of CAMP `kdf.py`.
//!
//! Before transcription every vector was also recomputed with the Python `cryptography`
//! package (AESCCM, ECDH) and `hashlib`/`hmac`; all 21 agreed. That is a transcription
//! check, not the independent oracle of WP-A0.

use p256::Scalar;
use v2xw_sec::ec::{Point, scalar_from_be32};
use v2xw_sec::{ccm, ecies, kdf};

fn hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd hex length");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn arr<const N: usize>(s: &str) -> [u8; N] {
    hex(s).try_into().expect("width")
}

struct Ecies {
    v: &'static str,
    k: &'static str,
    p1_printed: &'static str,
    p1_real: &'static str,
    r: &'static str,
    rx: &'static str,
    ry: &'static str,
    vx: &'static str,
    vy: &'static str,
    c: &'static str,
    t: &'static str,
}

fn point(x: &str, y: &str) -> Point {
    let mut sec1 = vec![0x04];
    sec1.extend(hex(x));
    sec1.extend(hex(y));
    Point::from_sec1(&sec1).expect("on the curve")
}

fn scalar(s: &str) -> Scalar {
    scalar_from_be32(&arr::<32>(s)).expect("scalar")
}

#[test]
fn d6_1_aes_ccm_all_six() {
    for (i, (k, n, p, ct)) in D61_CCM.iter().enumerate() {
        let (k, n, p, ct) = (arr::<16>(k), arr::<12>(n), hex(p), hex(ct));
        assert_eq!(
            ccm::encrypt(&k, &n, &p).unwrap(),
            ct,
            "D.6.1 #{} encrypt",
            i + 1
        );
        assert_eq!(
            ccm::decrypt(&k, &n, &ct).unwrap(),
            p,
            "D.6.1 #{} decrypt",
            i + 1
        );
        let mut bad = ct.clone();
        *bad.last_mut().unwrap() ^= 0x80;
        assert!(
            ccm::decrypt(&k, &n, &bad).is_err(),
            "D.6.1 #{} tamper",
            i + 1
        );
    }
}

#[test]
fn d6_2_ecies_all_four_with_the_real_p1() {
    for (i, e) in D62_ECIES.iter().enumerate() {
        let r = scalar(e.r);
        let recipient = point(e.rx, e.ry);
        assert_eq!(Point::mul_base(&r), recipient, "D.6.2 #{}: R = r·G", i + 1);
        let p1 = arr::<32>(e.p1_real);
        let k = arr::<16>(e.k);
        let enc = ecies::encrypt(&recipient, &k, &p1, &scalar(e.v)).unwrap();
        assert_eq!(enc.v, point(e.vx, e.vy), "D.6.2 #{}: V", i + 1);
        assert_eq!(enc.c, arr::<16>(e.c), "D.6.2 #{}: C", i + 1);
        assert_eq!(enc.t, arr::<16>(e.t), "D.6.2 #{}: T", i + 1);
        // Decrypting the printed (V, C, T) with r gives back k.
        let printed = ecies::EciesP256EncryptedKey {
            v: point(e.vx, e.vy),
            c: arr(e.c),
            t: arr(e.t),
        };
        assert_eq!(
            ecies::decrypt(&r, &printed, &p1).unwrap(),
            k,
            "D.6.2 #{}: k",
            i + 1
        );
    }
}

/// `(C, T)` of D.6.2 vector `e` with an arbitrary KDF parameter, composed from the
/// crate's own point arithmetic, KDF2 and MAC1 (the printed P1 is 16 bytes, which the
/// 32-byte `ecies` API cannot take).
fn c_t_with_kdf_param(e: &Ecies, kdp: &[u8]) -> ([u8; 16], [u8; 16]) {
    let (z, _) = point(e.rx, e.ry).mul(&scalar(e.v)).xy().unwrap();
    let k = kdf::kdf2_sha256(&z, kdp, 48);
    let mut c = arr::<16>(e.k);
    for (b, x) in c.iter_mut().zip(&k[..16]) {
        *b ^= x;
    }
    (c, kdf::mac1(&k[16..], &c))
}

/// The erratum (02 §8.2): the P1 the annex prints is the AES key `k`, and with it no
/// vector reproduces; with the real P1 from CAMP `ecies.py` every one does.
#[test]
fn d6_2_the_printed_p1_does_not_reproduce() {
    for (i, e) in D62_ECIES.iter().enumerate() {
        assert_eq!(e.p1_printed, e.k, "D.6.2 #{}: printed P1 is k", i + 1);
        let printed = c_t_with_kdf_param(e, &hex(e.p1_printed));
        assert_ne!(printed.0, arr::<16>(e.c), "D.6.2 #{}: C", i + 1);
        assert_ne!(printed.1, arr::<16>(e.t), "D.6.2 #{}: T", i + 1);
        assert_eq!(
            c_t_with_kdf_param(e, &hex(e.p1_real)),
            (arr(e.c), arr(e.t)),
            "D.6.2 #{}: the helper is ECIES",
            i + 1
        );
        // Decryption under a P1 other than the real one is refused.
        let mut wrong = arr::<32>(e.p1_real);
        wrong[..16].copy_from_slice(&hex(e.p1_printed));
        let printed_enc = ecies::EciesP256EncryptedKey {
            v: point(e.vx, e.vy),
            c: arr(e.c),
            t: arr(e.t),
        };
        assert_eq!(
            ecies::decrypt(&scalar(e.r), &printed_enc, &wrong),
            Err(ecies::EciesError::AuthenticationFailed),
            "D.6.2 #{}",
            i + 1
        );
    }
}

#[test]
fn d6_3_mac1_all_seven() {
    for (i, (k, m, t)) in D63_MAC1.iter().enumerate() {
        let t = arr::<16>(t);
        assert_eq!(kdf::mac1(&hex(k), &hex(m)), t, "D.6.3 #{}", i + 1);
        assert!(
            kdf::mac1_verify(&hex(k), &hex(m), &t),
            "D.6.3 #{} verify",
            i + 1
        );
        let mut bad = t;
        bad[0] ^= 1;
        assert!(
            !kdf::mac1_verify(&hex(k), &hex(m), &bad),
            "D.6.3 #{} tamper",
            i + 1
        );
    }
}

#[test]
fn d6_4_kdf2_all_four() {
    for (i, (ss, kdp, dl, key)) in D64_KDF2.iter().enumerate() {
        assert_eq!(
            kdf::kdf2_sha256(&hex(ss), &hex(kdp), *dl),
            hex(key),
            "D.6.4 #{}",
            i + 1
        );
    }
}

// D.6.1 AES-CCM-128: (K, N, P, C || T).
const D61_CCM: [(&str, &str, &str, &str); 6] = [
    (
        "e58d5c8f8c9ed9785679e08abc7c8116",
        "a9f593c09eaeea8bf0c1cf6a",
        "0653b5714d1357f4995bddacbe10873951a1eba663718d1af35d2f0d52c79de49be622c4a6d90647ba2b004c3e8ae422\
        fd27063afa19ad883dccbd97d98b8b0461b5671e75f19701c24042b8d3af79b9ff62bc448ef9440b1ea3f7e5c0f4bfef\
        e3e326e62d5ee4cb4b4cfff30ad5f49a7981abf71617245b96e522e1add78a",
        "5f82b9fce34b94835395dd89d71fb758d2a3907fbf2fd58994a2b9cf8725af26f0b23853c27a06e35ee72cad827713c1\
        8fa5dda971d9baa7b42a301ff60c6e4ad651c1bb6ed4f25f7d0ff387a11627934cd11f86984ea3ac969dda9a020ad642\
        4b0d393e3fb4b1119adf5cdb012a59753e41d47e5e5a8c3a118ed407049b56d53bf56cb38c0b20a2502d1da70b9761",
    ),
    (
        "e58d5c8f8c9ed9785679e08abc7c8116",
        "a9f593c09eaeea8bf0c1cf6a",
        "aca650ccccda604e16a8b54a3335e0bc2fd9444f33e3d9b82afe6f445357634974f0f1728cf113452321cbe5858304b0\
        1d4a14ae7f3b45980ee8033ad2a8599b78c29494c9e5f8945a8cade3eb5a30d156c0d83271626daddb650954093443fb\
        ac9701c02e5a973f39c2e1761a4b48c764bf6db215a54b285a06eca3af0a83f7",
        "f5775c416282a339dc66b56f5a3ad0ddacdb3f96efbd812b4d01f98686b5518b1fa4ebe5e85213e1c7ede704397ef353\
        6fc8cf3df4fb52b7870e8eb2fd2fbcd5cf263231d2c09dcae5c31cdc99e36efbe5737bf067d58a0a535b242bcbca2a56\
        04791e183cb0c2e5e851425e11b4e528237f123b5de8e349dd6d1a4506465f7257001080003872271900d3f39c9661fd",
    ),
    (
        "e58d5c8f8c9ed9785679e08abc7c8116",
        "a9f593c09eaeea8bf0c1cf6a",
        "d1aa8bbc04dfc92ffe2cb7748e70b02f5a91da14781223a712d44c4ba14a1c78eb02387fe73fdcbca8447056acaa9b5f\
        94d5208972b706df9fc4c803eabb2bc58c3d8df4ac496c34cb6bab939478cb417995b2314daf7af3f4c8a8d5d57a03f0\
        eb2b7bbd2d16babbf22c5b1eebff72c7dd4f912d5821f9a6bfa2d063ce6f6648df",
        "887b8731aa870a5834e2b751e77f804ed993a1cda44c7b34752bda8974a82eba805622e8839cdc184c885cb710576cbc\
        e657fb1af97711f01622458bc53cce8b3bd92b51b76c096a74241aace6c1956bca2611f35b189d547cf685aa17846a5d\
        43c564653ffcef6123bff836e000df289a8feea4106c51c738c926856723bacdb3f5d0f87f7e29d94bf1b41de8063e10\
        71",
    ),
    (
        "b8453a728060f8d517baceed3829f4d9",
        "cfbce69c884d5babbbaaf9a3",
        "f7629b73dae85a9bca45c42eb7fc1818dc74a60e13ae65a043e24b5a4d3ae04c273e7d6f42710f2d223d09eb7c131571\
        8a5a1293d482e4c45c3e852e5106aad7b695a02c4854801a5efe937a6540bce8734e8141558c3433b1d4c733dc5ef9c4\
        7b5279aa46ee3d8bd33b0950be5c9ebdf18bcf069b6daf82ff1186912f0aba",
        "dede575b6efe390f2cbb4f368a711f6cdf69abd11af580b2bf4029f85eb835d1abddb30e9e9cf3f13cba3bcc2e918713\
        d218af0d07cc614af69892afa986af2d5e60edb05d09d3b29e2a65b543ad6f26e5d76b660fe9184906a6315cd6b5355f\
        a291a1e90c510df20e46c116e2180009c287659db8d45cc3968049fa29f08de5d156edf7b0dbc84e410f292868c4be",
    ),
    (
        "b8453a728060f8d517baceed3829f4d9",
        "cfbce69c884d5babbbaaf9a3",
        "29b4013f552fbce993544cc6605cb05c62a7894c4c99e6a12c5f9f2ee4dfbebad70cdd0893542240f28bb5fbb9090332\
        ed110abfae6c4c6460d916f8994136575b5a6fd8db605fdf14cb81977aff7f99b5272580bf220133c691b09badc4d1fe\
        7125fd17fdbfc103e3f00a4d8e5a6f1e3d3af2a908535de858e1ccd3db4d1835",
        "0008cd17e139df7d75aac7de5dd1b72861ba849345c203b3d0fdfd8cf75d6b275bef13694fb9de9cec0c87dceb8b9150\
        b553b7217d22c9eaca7f017961c133adb3af2244ce3d0c77d41f77585c12ac5723becfa7e5472d4971e346f4a72f1d65\
        a8e62554b700f17a3e8dc20bd21ef1aa0e3658322beaaea9317003b8ddb72ffdfa0834974152b95bade2df83d7eec455",
    ),
    (
        "b8453a728060f8d517baceed3829f4d9",
        "cfbce69c884d5babbbaaf9a3",
        "1d76bdf0626a7134beb28a90d54ed7796c4c9535465c090c4b583a8cd40ef0a3864e7c07ccaed140df6b9d73234e652f\
        8ff425fc206f63dfab7dcdbbbe30411a14695e72a2bd8c4bfb1d6991db4f99eea7435e55261e37fdf57ce79df725c810\
        192f5e6e0331ed62eb8a72c5b9da6dfd9748b3d168a69bab33319efd1e84ef2570",
        "34ca71d8d67c12a0584c0188e8c3d00d6f5198ea4f07ec1eb7fa582ec78c253e0aadb26610432d9cc1ecaf5471ccf74d\
        d7b69862f321e65101dbda3a46b044e0fc9c13eeb7e0dfe33bc99f5efda24a2031dab4727c7b1b87420e11f2fdce048b\
        c0ec862d498edd1b36f7ba83e59ef349a444194a4b1f68ea5aa05196187ed8ed684826c0c356a9b8eda55bd91c2ba102\
        2b",
    ),
];

// D.6.2 ECIES: v, k, printed P1, real P1 (CAMP ecies.py), r, Rx, Ry, Vx, Vy, C, T.
const D62_ECIES: [Ecies; 4] = [
    Ecies {
        v: "1384c31d6982d52bca3bed8a7e60f52fecdab44e5c0ea166815a8159e09ffb42",
        k: "9169155b08b07674cbadf75fb46a7b0d",
        p1_printed: "9169155b08b07674cbadf75fb46a7b0d",
        p1_real: "a6b7b52554b4203f7e3acfdb3a3ed8674ee086ce5906a7cac2f8a398306d3be9",
        r: "060e41440a4e35154ca0efcb52412145836ad032833e6bc781e533bf14851085",
        rx: "8c5e20fe31935f6fa682a1f6d46e4468534ffea1a698b14b0b12513eed8deb11",
        ry: "1270fec2427e6a154dfcae3368584396c8251a04e2ae7d87b016ff65d22d6f9e",
        vx: "f45a99137b1bb2c150d6d8cf7292ca07da68c003daa766a9af7f67f5ee916828",
        vy: "f6a25216f44cb64a96c229ae00b479857b3b81c1319fb2adf0e8db2681769729",
        c: "a6342013d623ad6c5f6882469673ae33",
        t: "80e1d85d30f1bae4ecf1a534a89a0786",
    },
    Ecies {
        v: "d418760f0cb2dcb856bc3c7217ad3aa36db6742ae1db655a3d28df88cbbf84e1",
        k: "9169155b08b07674cbadf75fb46a7b0d",
        p1_printed: "9169155b08b07674cbadf75fb46a7b0d",
        p1_real: "a6b7b52554b4203f7e3acfdb3a3ed8674ee086ce5906a7cac2f8a398306d3be9",
        r: "060e41440a4e35154ca0efcb52412145836ad032833e6bc781e533bf14851085",
        rx: "8c5e20fe31935f6fa682a1f6d46e4468534ffea1a698b14b0b12513eed8deb11",
        ry: "1270fec2427e6a154dfcae3368584396c8251a04e2ae7d87b016ff65d22d6f9e",
        vx: "ee9cc7fbd9edecea41f7c8bd258e8d2e988e75bd069addca1e5a38e534ac6818",
        vy: "5ae3c8d9fe0b1fc7438f29417c240f8bf81c358ec1a4d0c6e98d8edbcc714017",
        c: "dd530be3bcd149e881e09f06e160f5a0",
        t: "06c1f0f5eaed453caf78e01a3d16a001",
    },
    Ecies {
        v: "1384c31d6982d52bca3bed8a7e60f52fecdab44e5c0ea166815a8159e09ffb42",
        k: "687e9757debfd87b0c267330c183c7b6",
        p1_printed: "687e9757debfd87b0c267330c183c7b6",
        p1_real: "05bed5f867b89f30fe5552df414b65b9dd4073fc385d14921c641a145aa12051",
        r: "da5e1d853fcc5d0c162a245b9f29d38eb6059f0db172fb7fda6663b925e8c744",
        rx: "8008b06fc4c9f9856048da186e7dc390963d6a424e80b274fb75d12188d7d73f",
        ry: "2774fb9600f27d7b3bbb2f7fcd8d2c96d4619ef9b4692c6a7c5733b5bac8b27d",
        vx: "f45a99137b1bb2c150d6d8cf7292ca07da68c003daa766a9af7f67f5ee916828",
        vy: "f6a25216f44cb64a96c229ae00b479857b3b81c1319fb2adf0e8db2681769729",
        c: "1f6346edaeaf57561fc9604febeff44e",
        t: "373c0fa7c52a0798ec36eadfe387c3ef",
    },
    Ecies {
        v: "4624a6f9f6bc6bd088a71ed97b3aee983b5cc2f574f64e96a531d2464137049f",
        k: "687e9757debfd87b0c267330c183c7b6",
        p1_printed: "687e9757debfd87b0c267330c183c7b6",
        p1_real: "05bed5f867b89f30fe5552df414b65b9dd4073fc385d14921c641a145aa12051",
        r: "da5e1d853fcc5d0c162a245b9f29d38eb6059f0db172fb7fda6663b925e8c744",
        rx: "8008b06fc4c9f9856048da186e7dc390963d6a424e80b274fb75d12188d7d73f",
        ry: "2774fb9600f27d7b3bbb2f7fcd8d2c96d4619ef9b4692c6a7c5733b5bac8b27d",
        vx: "121aa495c6b2c07a2b2daec36bd207d6620d7e6081050df5de3e9696868fcdca",
        vy: "46c31a1abea0bddaaaaefbba3afdbff1ac8d196bc313fc130926810c05503950",
        c: "6cfd13b76436cd0db70244fae380cba1",
        t: "c8bf18ac796b0b1d3a1256d3a91676c8",
    },
];

// D.6.3 MAC1: (K, M, T).
const D63_MAC1: [(&str, &str, &str); 7] = [
    (
        "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b",
        "4869205468657265",
        "b0344c61d8db38535ca8afceaf0bf12b",
    ),
    (
        "4a656665",
        "7768617420646f2079612077616e7420666f72206e6f7468696e673f",
        "5bdcc146bf60754e6a042426089575c7",
    ),
    (
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd\
        dddd",
        "773ea91e36800e46854db8ebd09181a7",
    ),
    (
        "0102030405060708090a0b0c0d0e0f10111213141516171819",
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd\
        cdcd",
        "82558a389a443c0ea4cc819899f2083a",
    ),
    (
        "0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c",
        "546573742057697468205472756e636174696f6e",
        "a3b6167473100ee06e0c796c2955552b",
    ),
    (
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "54657374205573696e67204c6172676572205468616e20426c6f636b2d53697a65204b6579202d2048617368204b6579\
        204669727374",
        "60e431591ee0b67f0d8a26aacbf5b77f",
    ),
    (
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "5468697320697320612074657374207573696e672061206c6172676572207468616e20626c6f636b2d73697a65206b65\
        7920616e642061206c6172676572207468616e20626c6f636b2d73697a6520646174612e20546865206b6579206e6565\
        647320746f20626520686173686564206265666f7265206265696e6720757365642062792074686520484d414320616c\
        676f726974686d2e",
        "9b09ffa71b942fcb27635fbcd5b0e944",
    ),
];

// D.6.4 KDF2: (ss, kdp, dl, derived key from NIST CAVP ansx963_2001.rsp).
const D64_KDF2: [(&str, &str, usize, &str); 4] = [
    (
        "96c05619d56c328ab95fe84b18264b08725b85e33fd34f08",
        "",
        16,
        "443024c3dae66b95e6f5670601558f71",
    ),
    (
        "96f600b73ad6ac5629577eced51743dd2c24c21b1ac83ee4",
        "",
        16,
        "b6295162a7804f5667ba9070f82fa522",
    ),
    (
        "22518b10e70f2a3f243810ae3254139efbee04aa57c7af7d",
        "75eef81aa3041e33b80971203d2c0c52",
        128,
        "c498af77161cc59f2962b9a713e2b215152d139766ce34a776df11866a69bf2e52a13d9c7c6fc878c50c5ea0bc7b00e0\
        da2447cfd874f6cf92f30d0097111485500c90c3af8b487872d04685d14c8d1dc8d7fa08beb0ce0ababc11f0bd496269\
        142d43525a78e5bc79a17f59676a5706dc54d54d4d1f0bd7e386128ec26afc21",
    ),
    (
        "7e335afa4b31d772c0635c7b0e06f26fcd781df947d2990a",
        "d65a4812733f8cdbcdfb4b2f4c191d87",
        128,
        "c0bd9e38a8f9de14c2acd35b2f3410c6988cf02400543631e0d6a4c1d030365acbf398115e51aaddebdc9590664210f9\
        aa9fed770d4c57edeafa0b8c14f93300865251218c262d63dadc47dfa0e0284826793985137e0a544ec80abf2fdf5ab9\
        0bdaea66204012efe34971dc431d625cd9a329b8217cc8fd0d9f02b13f2f6b0b",
    ),
];
