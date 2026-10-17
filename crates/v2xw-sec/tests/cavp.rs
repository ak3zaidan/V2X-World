//! NIST CAVP samples for the WP-A2 primitives (docs/design/scms-real/00 §4.2): ECDSA
//! P-384/SHA-384 signature verification and SHA-384 (gap row 6, the elector curve), and
//! AES-128-CCM (gap row 8).
//!
//! # Provenance (values transcribed, nothing fetched at test time)
//!
//! NIST CAVP archives, https://csrc.nist.gov/projects/cryptographic-algorithm-validation-program
//! (US Government works), fetched 2026-10-16:
//!
//! - `dss/186-4ecdsatestvectors.zip` (sha256
//!   `fe47cc92b4cee418236125c9ffbcd9bb01c8c34e74a4ba195d954bcb72824752`), `SigVer.rsp`
//!   section `[P-384,SHA-384]`, all 15 cases (3 pass; 12 fail with R, S, Q or the
//!   message changed).
//! - `shs/shabytetestvectors.zip` (sha256
//!   `929ef80b7b3418aca026643f6f248815913b60e01741a44bba9e118067f4c9b8`),
//!   `SHA384ShortMsg.rsp`, Len = 0, 8, 512, 1024.
//! - `mac/ccmtestvectors.zip` (sha256
//!   `0e3c96761bd8f1a0b4dfe666146c32f7d367a49968375ff091b8da9ecaa673aa`): `VNT128.rsp`
//!   `[Nlen = 12]` counts 50–54 (the 1609.2 nonce length, with 32 bytes of associated
//!   data) and `DVPT128.rsp` `[Alen = 0, Plen = 24, Nlen = 13, Tlen = 16]` counts 0–9
//!   (decryption-verification; 8 of the 10 must be rejected).

use v2xw_sec::{ccm, p384};

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

#[test]
fn sigver_p384_sha384_all_fifteen() {
    let mut passes = 0;
    for (i, (msg, qx, qy, r, s, ok)) in SIGVER_P384_SHA384.iter().enumerate() {
        let mut q = vec![0x04];
        q.extend(hex(qx));
        q.extend(hex(qy));
        let sig = p384::P384Signature {
            r: arr(r),
            s: arr(s),
        };
        assert_eq!(
            p384::verify(&q, &hex(msg), &sig),
            *ok,
            "SigVer [P-384,SHA-384] case {i}"
        );
        passes += usize::from(*ok);
    }
    assert_eq!(passes, 3);
}

/// Signing is the inverse of verification on the CAVP keys' curve: a key signs and the
/// CAVP verifier path accepts it, and the signature does not verify under a CAVP key.
#[test]
fn p384_sign_then_verify() {
    let d = arr::<48>(&"07".repeat(48));
    let key = p384::P384SigningKey::from_be48(&d).unwrap();
    let sig = key.sign(b"ctl");
    assert!(p384::verify(&key.public_sec1(false), b"ctl", &sig));
    let (_, qx, qy, ..) = SIGVER_P384_SHA384[1];
    let mut q = vec![0x04];
    q.extend(hex(qx));
    q.extend(hex(qy));
    assert!(!p384::verify(&q, b"ctl", &sig));
}

#[test]
fn sha384_short_messages() {
    for (bits, msg, md) in SHA384_SHORT {
        assert_eq!(hex(msg).len() * 8, bits);
        assert_eq!(
            p384::sha384(&hex(msg)).to_vec(),
            hex(md),
            "SHA-384 Len = {bits}"
        );
    }
}

#[test]
fn ccm_vnt128_twelve_byte_nonce() {
    let key = arr::<16>(VNT128_N12_KEY);
    for (i, (n, a, p, ct)) in VNT128_N12.iter().enumerate() {
        let (n, a, p, ct) = (hex(n), hex(a), hex(p), hex(ct));
        assert_eq!(
            ccm::seal_raw(&key, &n, &a, &p).unwrap(),
            ct,
            "VNT128 N12 #{i}"
        );
        assert_eq!(
            ccm::open_raw(&key, &n, &a, &ct).unwrap(),
            p,
            "VNT128 N12 #{i} open"
        );
    }
}

#[test]
fn ccm_dvpt128_no_aad() {
    let key = arr::<16>(DVPT128_A0_P24_N13_KEY);
    let mut rejected = 0;
    for (i, (n, ct, p)) in DVPT128_A0_P24_N13.iter().enumerate() {
        let got = ccm::open_raw(&key, &hex(n), &[], &hex(ct));
        match p {
            Some(p) => assert_eq!(got.unwrap(), hex(p), "DVPT128 #{i}"),
            None => {
                assert_eq!(
                    got,
                    Err(ccm::CcmError::AuthenticationFailed),
                    "DVPT128 #{i}"
                );
                rejected += 1;
            }
        }
    }
    assert_eq!(rejected, 8);
}

// 186-4ecdsatestvectors/SigVer.rsp [P-384,SHA-384]: (Msg, Qx, Qy, R, S, pass).
const SIGVER_P384_SHA384: [(&str, &str, &str, &str, &str, bool); 15] = [
    (
        "4132833a525aecc8a1a6dea9f4075f44feefce810c4668423b38580417f7bdca5b21061a45eaa3cbe2a7035ed189523a\
        f8002d65c2899e65735e4d93a16503c145059f365c32b3acc6270e29a09131299181c98b3c76769a18faf21f6b4a8f27\
        1e6bf908e238afe8002e27c63417bda758f846e1e3b8e62d7f05ebd98f1f9154",
        "1f94eb6f439a3806f8054dd79124847d138d14d4f52bac93b042f2ee3cdb7dc9e09925c2a5fee70d4ce08c61e3b19160",
        "1c4fd111f6e33303069421deb31e873126be35eeb436fe2034856a3ed1e897f26c846ee3233cd16240989a7990c19d8c",
        "3c15c3cedf2a6fbff2f906e661f5932f2542f0ce68e2a8182e5ed3858f33bd3c5666f17ac39e52cb004b80a0d4ba73cd",
        "9de879083cbb0a97973c94f1963d84f581e4c6541b7d000f9850deb25154b23a37dd72267bdd72665cc7027f88164fab",
        false,
    ),
    (
        "9dd789ea25c04745d57a381f22de01fb0abd3c72dbdefd44e43213c189583eef85ba662044da3de2dd8670e632515448\
        0155bbeebb702c75781ac32e13941860cb576fe37a05b757da5b5b418f6dd7c30b042e40f4395a342ae4dce05634c336\
        25e2bc524345481f7e253d9551266823771b251705b4a85166022a37ac28f1bd",
        "cb908b1fd516a57b8ee1e14383579b33cb154fece20c5035e2b3765195d1951d75bd78fb23e00fef37d7d064fd9af144",
        "cd99c46b5857401ddcff2cf7cf822121faf1cbad9a011bed8c551f6f59b2c360f79bfbe32adbcaa09583bdfdf7c374bb",
        "33f64fb65cd6a8918523f23aea0bbcf56bba1daca7aff817c8791dc92428d605ac629de2e847d43cee55ba9e4a0e83ba",
        "4428bb478a43ac73ecd6de51ddf7c28ff3c2441625a081714337dd44fea8011bae71959a10947b6ea33f77e128d3c6ae",
        true,
    ),
    (
        "9c4479977ed377e75f5cc047edfa689ef232799513a2e70280e9b124b6c8d166e107f5494b406853aec4cff0f2ca00c6\
        f89f0f4a2d4ab0267f44512dfff110d1b1b2e5e78832022c14ac06a493ab789e696f7f0f060877029c27157ce40f8125\
        8729caa4d9778bae489d3ab0259f673308ae1ec1b1948ad2845f863b36aedffb",
        "9b3c48d924194146eca4172b6d7d618423682686f43e1dbc54ed909053d075ca53b68ae12f0f16a1633d5d9cb17011ec",
        "695039f837b68e59330ee95d11d5315a8fb5602a7b60c15142dbba6e93b5e4aba8ae4469eac39fa6436323eccc60dcb6",
        "202da4e4e9632bcb6bf0f6dafb7e348528d0b469d77e46b9f939e2fa946a608dd1f166bcbcde96cfad551701da69f6c2",
        "db595b49983882c48df8a396884cd98893a469c4d590e56c6a59b6150d9a0acdf142cf92151052644702ed857a5b7981",
        false,
    ),
    (
        "21eb31f2b34e4dde8d6c701e976d3fbbf4de6a3384329118d4ddb49adb2bb44465598abf6df25858b450c7767e282cca\
        ca494088274e37353674eef58f583937d3d184ef727317d3672397a74c8fe327919a3df8fd65af0bc8cebbc40095adf8\
        9f1bf2c5e6dc6ba44633fd8433b25f065f5e3eb4840af23cc534415406745a31",
        "5140108b93b52d9ad572d6129ed6564766f8df3755e49fa53eba41a5a0d6c1d24a483c90070583a66e3cfa52b6fb1f31",
        "ff52498446a40c61e60c97554256472625633eda0c1a8b4061481fecfbe9c4503e99dfc69e86c9e85c8cc53dca6b8dc4",
        "b2726b2ba9da02de35e9953fc283d1e78700860d4c33dce8db04dd41499d904866c1b8debb377f6c0dfcb0704252174f",
        "0775b027068d7ad55121a278a819f52099ace750d5e996eaec9dee7be72758736cf769650148fbd5c411beb9b88f979e",
        false,
    ),
    (
        "58ea3b1e82f97708053d0b41441d0aa9619050e86ac6c4f7781164e5da3019c47a839366509fa95812e4f64afdc62b62\
        7c7a98f633dd05db45c1d8954fc83bdb5042679378bb7e4c7863aacf2026360ca58314983e6c726cf02bb347706b844d\
        dc66aee4177c309cb700769553480cdd6b1cd77341c9a81c05fbb80819bc623f",
        "31f4fc2fac3a163a5796f5e414af6f8107ab5e4a98c755d81efa9d5a83c10128c16c863190112fc29d3d5f3057a2edf1",
        "fe208743f3e96c3a34b5fff78c9716c074a1ce3dc01c3f0e471ddfae91cd88e7dda38dd0e5e1f91b00b8539da3cc10bc",
        "706911812ec9e7370234efd57b2855975eab81e9c2fe783aa8e442dc6e7d681dab2dc0dfc6765f87ab67001108e3facf",
        "42c89efa22d853d32f619c9fe13e9852889ac98a9fed5d4fa47fed238e1cbe70d7970af9f7bdf84e51176af4885f2490",
        false,
    ),
    (
        "188cd53097ef3e64b78b9260bf461708c836f25f2bcc98b534af98b96ee4b324e2203a7e62dbc396966f56419fb5135c\
        b124369aaa025f396eac72f05ab45950d9e02cd5a2357eafab9f816117b7f1de192468895327802ec79f5d6b5a3d44d7\
        afbed7b4a308e365655b8db2bde75e143062ee48b7c51688ac5db0bc7c83ec9c",
        "1f7911dcfe63a6f270cf75b8584d9b1b4a00afc1fa43543c945945b8a821ebeb37fbc705a000f9cc7c35f7d27027b7bb",
        "f11835ec80c4ac06d99247e73bf72522109ac255e6109262de4dfbf9619244f74fb6c9ee57694537d7e79c248db34dc4",
        "3587c9c6885adf3be1086825f9a41ccd2edfa0bd95e7fc4dba5a9710f41d539132de7772f14c18e318f8992b66d2a86c",
        "73a844d729599d4e3e3c1b63e9c4bf5a73d1f69e0160857fe63a56c381c051f5c37ea6b4cc4caacb6ff26ef9699efe30",
        false,
    ),
    (
        "6462bc8c0181db7d596a35aa25d5d323dd3b2798054c2af6c22e841b1ccf3dc3ee514f86d4a0cef7a6f7f566ae448b24\
        dcc8d11eb7a585d44923ea1a06c774a2b3eb7409ab17a0065d5834ab00309ad44312a7317259219543e80ddb0cc2a438\
        1bf6e53cd1bb357eba82e11c59f82e446c4b79314119182c0de96a1b5bae0b08",
        "2039661db813d494a9ecb2c4e0cdd7b54068aae8a5d0597009f67f4f36f32c8ee939abe03716e94970bba69f595fead6",
        "e2d5236e7e357744514e66a3fb111073336de929598eb79fb4368c5bf80814e7584a3b94118faac9321df37452a846fc",
        "164b8ac2b34c4c499b9d6727e130b5ef37c296bd22c306d1396c6aa54ca661f729aa6353b55d7cf1793b80b5a485115f",
        "4e7187f8f735b7272f2c0985315b5602bb9b1a09f32233aa10570c82d1ccedef6e725800336511e47f88ddbbbdc08f54",
        false,
    ),
    (
        "13c63a3cb61f15c659720658a77869145ae8a176c6d93d3a8aa9946236d9fb0463db9e48c667cba731afaa814ba0d583\
        57524f8de28d4c4bbe2691dac9b32632a7dd0f99fd4cb240290878305011f7d3e37ecc410cc1fed601e7901e8be6414e\
        a44317584843a2d2ca2e15103e1ea49365bc384355b3c6fa6ccdd452543e9769",
        "46dcf8ee848c6459fa66d1cae91ccd471401a5782cb2d3b9b9264189f0e9ddf7197b05c694931bde3306240cf9d24b7e",
        "79d9508f82c5ead05c3f9392f3b1458f6d6c02f44420b9021d656e59402e2645bf3ba1a6b244ddb12edbb69516d5873b",
        "5ffba3b5bd7c3a89ec40b47884b0b3464e8abb78608c6d61e1e62c2ca98d44fcdf61825d69dffee8408d0849d0623bac",
        "0d2597b5fc3842ffce1957172253a8c9c0e4dbe770ce54f70f139e0545dc34ec639d609e14175bdb2b812ccfda00c9d4",
        false,
    ),
    (
        "6939a9118adc307107aa6b0057c280d10fa44a64700c7bd23e1f33a478ad2cfe596c05f72b540cbdb696aac6ab98d9ca\
        8c62f33e182657130b8317a76275a5996333a5d3547e2293b401d0adf60f91e91d2137e34f3336e017c3c6dba6bf5b13\
        dd0de288f9b20a896a92c48e984fbc09f920fab82f3f915d6524b0c11236aca4",
        "097cea75f685cf4d54324ad2124ce3f77b1e490bbaa1ffacde40dd988f7591e1c5d158e6f232500d958762831914af7f",
        "716d8bc056daf69ca2edd21b89a6ae9923cfcae87bfda5f9a6e514dd4b9d28d164fcc613ca2afb9660adfece59f09b66",
        "1c5d4561d2a3af8835839b543098c101c715c545eb7d00300c5cb05bb08dac29e732ffdc31c50915e691999ad505104c",
        "c3442f2fb1498fd47c2f959edff37a19783e3ccee80dc6955ca64db087fd188e67358e7b9223535bbb858d21ba6a978c",
        false,
    ),
    (
        "c82071e42c45ac3597f255ba27766afe366e31a553a4d2191360b88a2a349ee077291454bf7b323cb3c9d7fec5533e4e\
        4bf4fb5bc2eb16c6319e9378a3d8a444b2d758123438dbb457b26b14b654b3c88d66838adfa673067c0552d1b8a3ade3\
        a9cb777986c00f65cace53f852c1121acf19516a7cf0ba3820b5f51f31c539a2",
        "d2e2b3d262bb1105d914c32c007ea23d15a98197f0ed90b46a17f3d403e406a76c8f752be1a8cd01a94fd45157f6511a",
        "e585fba180017b9983b4c853ad3a5dd52e079c5f0ef792d1a0213b6085e390b073de1a4b01749ceab27806e5604980fe",
        "49c001c47bbcee10c81c0cdfdb84c86e5b388510801e9c9dc7f81bf667e43f74b6a6769c4ac0a38863dc4f21c558f286",
        "1fb4ff67340cc44f212404ba60f39a2cb8dcd3f354c81b7219289d32e849d4915e9d2f91969ba71e3dd4414f1e8f18f7",
        false,
    ),
    (
        "137b215c0150ee95e8494b79173d7ae3c3e71efcc7c75ad92f75659ce1b2d7eb555aad8026277ae3709f46e896963964\
        486946b9fe269df444a6ea289ec2285e7946db57ff18f722a583194a9644e863ae452d1457dc5db72ee20c486475f358\
        dc575c621b5ab865c662e483258c7191b4cc218e1f9afeeb3e1cb978ce9657dc",
        "cd887c65c01a1f0880bf58611bf360a8435573bc6704bfb249f1192793f6d3283637cd50f3911e5134b0d6130a1db60e",
        "f2b3cbf4fe475fd15a7897561e5c898f10caa6d9d73fef10d4345917b527ce30caeaef138e21ac6d0a49ef2fef14bee6",
        "addfa475b998f391144156c418561d323bdfd0c4f416a2f71a946712c349bb79ba1334c3de5b86c2567b8657fe4ca1f1",
        "1c314b1339f73545ff457323470695e0474c4b6860b35d703784fbf66e9c665de6ca3acb60283df61413e0740906f19e",
        false,
    ),
    (
        "93e7e75cfaf3fa4e71df80f7f8c0ef6672a630d2dbeba1d61349acbaaa476f5f0e34dccbd85b9a815d908203313a22fe\
        3e919504cb222d623ad95662ea4a90099742c048341fe3a7a51110d30ad3a48a777c6347ea8b71749316e0dd1902facb\
        304a76324b71f3882e6e70319e13fc2bb9f3f5dbb9bd2cc7265f52dfc0a3bb91",
        "a370cdbef95d1df5bf68ec487122514a107db87df3f8852068fd4694abcadb9b14302c72491a76a64442fc07bd99f02c",
        "d397c25dc1a5781573d039f2520cf329bf65120fdbe964b6b80101160e533d5570e62125b9f3276c49244b8d0f3e44ec",
        "c6c7bb516cc3f37a304328d136b2f44bb89d3dac78f1f5bcd36b412a8b4d879f6cdb75175292c696b58bfa9c91fe6391",
        "6b711425e1b14f7224cd4b96717a84d65a60ec9951a30152ea1dd3b6ea66a0088d1fd3e9a1ef069804b7d969148c37a0",
        true,
    ),
    (
        "15493aa10cfb804b3d80703ca02af7e2cfdc671447d9a171b418ecf6ca48b450414a28e7a058a78ab0946186ad2fe297\
        e1b7e20e40547c74f94887a00f27dde7f78a3c15eb1115d704972b35a27caf8f7cdcce02b96f8a72d77f36a20d3f829e\
        915cd3bb81f9c2997787a73616ed5cb0e864231959e0b623f12a18f779599d65",
        "d1cf635ca04f09b58879d29012f2025479a002bda590020e6a238bccc764478131cac7e6980c67027d92ece947fea5a6",
        "21f7675c2be60c0a5b7d6df2bcc89b56212a2849ec0210c59316200c59864fd86b9a19e1641d206fd8b29af7768b61d3",
        "6101d26e76690634b7294b6b162dcc1a5e6233813ba09edf8567fb57a8f707e024abe0eb3ce948675cd518bb3bfd4383",
        "4e2a30f71c8f18b74184837f981a90485cd5943c7a184aba9ac787d179f170114a96ddbb8720860a213cc289ae340f1f",
        false,
    ),
    (
        "bc5582967888a425fb757bd4965900f01e6695d1547ed967c1d4f67b1b1de365d203f407698761699fec5f5a614c21e3\
        6a9f57a8aaf852e95538f5615785534568811a9a9ccc349843f6c16dc90a4ac96a8f72c33d9589a860f4981d7b4ee717\
        3d1db5d49c4361368504c9a6cbbaedc2c9bff2b12884379ba90433698ceb881d",
        "d15ca4b2d944d5539658a19be8ef85874f0c363b870f1cd1f2dc9cb68b2a43a10d37064697c84543e60982ab62bb32c8",
        "062fb7dfc379fc6465302ac5d8d11d3b957b594c9ef445cfe856765dd59e6f10f11809e115ac64969baa23543f2e5661",
        "e2cf123ce15ca4edad5f087778d483d9536e4a37d2d55599541c06f878e60354aa31df250b2fc4ed252b80219552c958",
        "696707a7e3f9a4b918e7c994e7332103d8e816bbe6d0d1cf72877318e087ed0e230b0d1269902f369acb432b9e97a389",
        true,
    ),
    (
        "4f31331e20a3273da8fce6b03f2a86712ed5df41120a81e994d2b2f370e98ef35b847f3047d3cf57e88350e27b9ac3f0\
        2073ac1838db25b5ad477aee68930882304fc052f273821056df7500dc9eab037ed3ac3c75396e313bf0f4b89b26675a\
        f55f3378cf099d9d9a25a4887c1cfd2448f5b2188c41d6fa26045c5e974bf3e4",
        "c83d30de9c4e18167cb41c990781b34b9fceb52793b4627e696796c5803515dbc4d142977d914bc04c153261cc5b537f",
        "42318e5c15d65c3f545189781619267d899250d80acc611fe7ed0943a0f5bfc9d4328ff7ccf675ae0aac069ccb4b4d6e",
        "b567c37f7c84107ef72639e52065486c2e5bf4125b861d37ea3b44fc0b75bcd96dcea3e4dbb9e8f4f45923240b2b9e44",
        "d06266e0f27cfe4be1c6210734a8fa689a6cd1d63240cb19127961365e35890a5f1b464dcb4305f3e8295c6f842ef344",
        false,
    ),
];

// shabytetestvectors/SHA384ShortMsg.rsp: (Len bits, Msg, MD).
const SHA384_SHORT: [(usize, &str, &str); 4] = [
    (
        0,
        "",
        "38b060a751ac96384cd9327eb1b1e36a21fdb71114be07434c0cc7bf63f6e1da274edebfe76f65fbd51ad2f14898b95b",
    ),
    (
        8,
        "c5",
        "b52b72da75d0666379e20f9b4a79c33a329a01f06a2fb7865c9062a28c1de860ba432edfd86b4cb1cb8a75b46076e3b1",
    ),
    (
        512,
        "93035d3a13ae1b06dd033e764aca0124961da79c366c6c756bc4bcc11850a3a8d120854f34290fff7c8d6d83531dbdd1\
        e81cc4ed4246e00bd4113ef451334daa",
        "8d46cc84b6c2deb206aa5c861798798751a26ee74b1daf3a557c41aebd65adc027559f7cd92b255b374c83bd55568b45",
    ),
    (
        1024,
        "3bf52cc5ee86b9a0190f390a5c0366a560b557000dbe5115fd9ee11630a62769011575f15881198f227876e8fe685a69\
        39bc8b89fd48a34ec5e71e131462b2886794dffa68ccc6d564733e67ffef25e627c6f4b5460796e3bce67bf58ca6e8e5\
        55bc916a8531697ac948b90dc8616f25101db90b50c3d3dbc9e21e42ff387187",
        "12b6cb35eda92ee37356ddee77781a17b3d90e563824a984faffc6fdd1693bd7626039635563cfc3b9a2b00f9c65eefd",
    ),
];

// ccmtestvectors/VNT128.rsp [Nlen = 12] (Alen 32, Plen 24, Tlen 16), Key = 005e8f4d8e0cbf4e1ceeb5d87a275848: (Nonce, Adata, Payload, CT).
const VNT128_N12_KEY: &str = "005e8f4d8e0cbf4e1ceeb5d87a275848";
const VNT128_N12: [(&str, &str, &str, &str); 5] = [
    (
        "0ec3ac452b547b9062aac8fa",
        "2f1821aa57e5278ffd33c17d46615b77363149dbc98470413f6543a6b749f2ca",
        "b6f345204526439daf84998f380dcfb4b4167c959c04ff65",
        "9575e16f35da3c88a19c26a7b762044f4d7bbbafeff05d754829e2a7752fa3a14890972884b511d8",
    ),
    (
        "472711261a9262bef077c0b7",
        "17c87889a2652636bcf712d111c86b9d68d64d18d531928030a5ec97c59931a4",
        "9d63df773b3799e361c5328d44bbb12f4154747ecf7cc667",
        "53323b82d7a754d82cebf0d4bc930ef06d11e162c5c027c4715a641834bbb75bb6572ca5a45c3183",
    ),
    (
        "6a7b80b6738ff0a23ad58fb2",
        "26c12e5cdfe225a5be56d7a8aaf9fd4eb327d2f29c2ebc7396022f884f33ce54",
        "ba1978d58492c7f827cafef87d00f1a137f3f05a2dedb14d",
        "aa1d9eacabdcdd0f54681653ac44042a3dd47e338d15604e86a0e926daf21d17b359253d0d5d5d00",
    ),
    (
        "d8e133e7ff8e0a0ec6c4096e",
        "ef9e432c15d8c93a4b5c0666608e61c824cd466d7940d642acd3dc33057c0395",
        "2836de99c0f641cd55e89f5af76638947b8227377ef88bfb",
        "5edb056d85dafeaaf74bdf4caa47339d6a75bf1ee998565e9f9cdf6ab825f6e026f5be2ad895033e",
    ),
    (
        "2fa8120398d1a946f391367c",
        "377cd407ad28dc02bd3835a31d92f8295c9dbe597f56662ceda112c588dc73a5",
        "7a37255b682766a0bfecf78e5162528885a339174c2a4932",
        "701f5f506fc7e9ea4a27a4db5cb890f7be3b4f6bcb20f97ed3021f6ad620648b8196ab1693710398",
    ),
];

// ccmtestvectors/DVPT128.rsp [Alen = 0, Plen = 24, Nlen = 13, Tlen = 16], Key = 90929a4b0ac65b350ad1591611fe4829: (Nonce, CT, Payload or None for Fail).
const DVPT128_A0_P24_N13_KEY: &str = "90929a4b0ac65b350ad1591611fe4829";
const DVPT128_A0_P24_N13: [(&str, &str, Option<&str>); 10] = [
    (
        "5a8aa485c316e9403aff859fbb",
        "4bfe4e35784f0a65b545477e5e2f4bae0e1e6fa717eaf2cb6a9a970b9beb2ac1bd4fd62168f8378a",
        Some("a16a2e741f1cd9717285b6d882c1fc53655e9773761ad697"),
    ),
    (
        "49004912fdd7269279b1f06a89",
        "0c56a503aa2c12e87450d45a7b714db980fd348f327c0065a65666144994bad0c8195bcb4ade1337",
        None,
    ),
    (
        "efeb82c8c68d6600b24dd6d8ee",
        "5f69d6c21f771eb98dc724f891f530b1c045f49a054de103a85f868739404b64a7cbdd61b577c388",
        None,
    ),
    (
        "7b93d368dc551640b00ba3cbb5",
        "d335ba572520c336f711edf27ea738ba5e6b0d772ea443b8b2b164f3c255b699cbf75330d96c3c13",
        None,
    ),
    (
        "24b7a65391f88bea38fcd54a9a",
        "9fa846ef8d198c538f84f856bab8f7f9c3bed90b53acb6a32658e077687315eaf11458bdf6e3c36a",
        Some("43419715cef9a48dc7280bc035082a6581afd1d82bee9d1a"),
    ),
    (
        "6aa3f731522fce7e366ba59945",
        "b7095030acdc5fbb8fea2c24717c1c236231f9737bcc78f463db3756abba1feef626a956794d7e56",
        None,
    ),
    (
        "a11cf5bed0041ee3cb1fef4b43",
        "d6911d5831163c8ebad0916af1833051b885aae822f9f6657d6fee1de626bc7c93f2caa27a3ecaa0",
        None,
    ),
    (
        "273cc5013785baeb5abc79c8bd",
        "6b10a098c96c2bbf9aeb5c9adcf91e4812838dff319f8be989e2d235192f33ba0f357492112d98f4",
        None,
    ),
    (
        "d2d4482ea8e98c1cf309671895",
        "aecd11cbac04e1f79b0fd24052c8cedf393dce9df350d24f800b81e834ea5dd2bdc2c688d9505359",
        None,
    ),
    (
        "a8849b44adb48d271979656930",
        "d3a7a25f71b1988482dc852ed713d55abdcc4bb1129ddcae430889cd5c97343cc0dedfbd62e6b6eb",
        None,
    ),
];
