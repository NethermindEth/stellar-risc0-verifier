// This build script helps us generate the VerificationKey for the
// RiscZeroGroth16Verifier during the compilation. The key is fetched from
// `config.json` and makes it available as a const to the contract. This way, the
// verification key gets included in the contract at compile time, so we don't
// have to initialize the contract and spend resources on reading from the
// ledger the verification key.

use std::{env, fs, path::PathBuf, str::FromStr};

use ark_bn254::{Fq, Fq2, G1Affine, G2Affine};
use build_utils::{Sha256Digest, hash_point, tagged_iter, tagged_struct};
use serde::Deserialize;

#[derive(Deserialize)]
struct VerificationKeyJson {
    alpha: PointG1Json,
    beta: PointG2Json,
    gamma: PointG2Json,
    delta: PointG2Json,
    #[serde(rename = "IC")]
    ic: Vec<PointG1Json>,
}

#[derive(Deserialize)]
struct PointG1Json {
    x: String,
    y: String,
}

impl PointG1Json {
    pub fn into_g1_affine(&self) -> G1Affine {
        let x = Fq::from_str(&self.x).expect("Invalid field element for G1.x");
        let y = Fq::from_str(&self.y).expect("Invalid field element for G1.x");

        G1Affine::new_unchecked(x, y)
    }
}

#[derive(Deserialize)]
struct PointG2Json {
    x1: String,
    x2: String,
    y1: String,
    y2: String,
}

impl PointG2Json {
    pub fn into_g2_affine(&self) -> G2Affine {
        let x1 = Fq::from_str(&self.x1).expect("Invalid field element for G2.x1");
        let x2 = Fq::from_str(&self.x2).expect("Invalid field element for G2.x2");
        let y1 = Fq::from_str(&self.y1).expect("Invalid field element for G2.y1");
        let y2 = Fq::from_str(&self.y2).expect("Invalid field element for G2.y2");

        let x = Fq2::new(x1, x2);
        let y = Fq2::new(y1, y2);

        G2Affine::new_unchecked(x, y)
    }
}

#[derive(Deserialize)]
struct VerifierParameters {
    version: String,
    control_root: String,
    bn254_control_id: String,
    verification_key: VerificationKeyJson,
}

fn fq(s: &str) -> String {
    format!("ark_ff::MontFp!(\"{}\")", s)
}

fn fq2(x1: &str, x2: &str) -> String {
    format!("ark_bn254::Fq2::new({}, {})", fq(x1), fq(x2))
}

fn g1(p: &PointG1Json) -> String {
    format!(
        "ark_bn254::G1Affine::new_unchecked({}, {})",
        fq(&p.x),
        fq(&p.y)
    )
}

fn g2(p: &PointG2Json) -> String {
    format!(
        "ark_bn254::G2Affine::new_unchecked({}, {})",
        fq2(&p.x1, &p.x2),
        fq2(&p.y1, &p.y2)
    )
}

fn compute_vk_digest(vk: &VerificationKeyJson) -> Sha256Digest {
    let alpha = vk.alpha.into_g1_affine();
    let beta = vk.beta.into_g2_affine();
    let gamma = vk.gamma.into_g2_affine();
    let delta = vk.delta.into_g2_affine();
    let ic = vk.ic.iter().map(|point| {
        let p = point.into_g1_affine();
        hash_point(&p)
    });

    tagged_struct(
        "risc0_groth16.VerifyingKey",
        &[
            hash_point(&alpha),
            hash_point(&beta),
            hash_point(&gamma),
            hash_point(&delta),
            tagged_iter("risc0_groth16.VerifyingKey.IC", ic),
        ],
    )
}

fn compute_selector(
    control_root: &str,
    bn254_control_id: &str,
    vk: &VerificationKeyJson,
) -> [u8; 4] {
    let control_root_bytes =
        hex::decode(control_root).expect("Invalid hex string for control_root");
    let control_root: Sha256Digest = control_root_bytes
        .try_into()
        .expect("control_root must be exactly 32 bytes");

    let bn254_control_id_bytes =
        hex::decode(bn254_control_id).expect("Invalid hex string for bn254_control_id");
    let mut bn254_control_id: Sha256Digest = bn254_control_id_bytes
        .try_into()
        .expect("bn254_control_id must be exactly 32 bytes");
    bn254_control_id.reverse();

    let tag_struct = tagged_struct(
        "risc0.Groth16ReceiptVerifierParameters",
        &[control_root, bn254_control_id, compute_vk_digest(vk)],
    );

    [tag_struct[0], tag_struct[1], tag_struct[2], tag_struct[3]]
}

fn format_byte_array<const N: usize>(bytes: &[u8; N]) -> String {
    let formatted: Vec<String> = bytes.iter().map(|b| format!("{:#04x}", b)).collect();
    format!("[{}]", formatted.join(", "))
}

fn compute_control_roots(control_root: &str) -> ([u8; 16], [u8; 16]) {
    let mut bytes = hex::decode(control_root).expect("Invalid hex string for control_root");
    bytes.reverse();
    let mut control_root_0 = [0u8; 16];
    let mut control_root_1 = [0u8; 16];

    control_root_0.copy_from_slice(&bytes[0..16]);
    control_root_1.copy_from_slice(&bytes[16..32]);

    (control_root_0, control_root_1)
}

fn main() {
    let path = PathBuf::from("parameters.json");
    let data = fs::read_to_string(path).unwrap();
    let params: VerifierParameters = serde_json::from_str(&data).unwrap();

    let vk = &params.verification_key;
    let selector = compute_selector(&params.control_root, &params.bn254_control_id, vk);
    let (control_root_0, control_root_1) = compute_control_roots(&params.control_root);

    // Generate the VerificationKey IC array
    let ic: Vec<String> = vk.ic.iter().map(|p| g1(p)).collect();
    let ic = ic.join(", ");

    let vk_code = format!(
        "VerificationKey {{
    alpha: {},
    beta: {},
    gamma: {},
    delta: {},
    ic: [{}],
}}",
        g1(&vk.alpha),
        g2(&vk.beta),
        g2(&vk.gamma),
        g2(&vk.delta),
        ic
    );
    let selector_code = format_byte_array(&selector);
    let control_root_0_code = format_byte_array(&control_root_0);
    let control_root_1_code = format_byte_array(&control_root_1);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("verification_key.rs"), vk_code)
        .expect("failed to write verification_key.rs");
    fs::write(out_dir.join("selector.rs"), selector_code).expect("failed to write selector.rs");

    fs::write(out_dir.join("control_root_0.rs"), control_root_0_code)
        .expect("failed to write control_root_0.rs");
    fs::write(out_dir.join("control_root_1.rs"), control_root_1_code)
        .expect("failed to write control_root_1.rs");
}
