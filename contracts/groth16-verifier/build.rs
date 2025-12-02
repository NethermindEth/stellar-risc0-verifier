// This build script helps us generate the VerificationKey for the
// RiscZeroGroth16Verifier during the compilation. The key is fetched from
// `config.json` and makes it available as a const to the contract. This way, the
// verification key gets included in the contract at compile time, so we don't
// have to initialize the contract and spend resources on reading from the
// ledger the verification key.

use std::{env, fs, path::PathBuf, str::FromStr};

use ark_bn254::{Fq, Fq2, G1Affine, G2Affine};
use build_utils::{Sha256Digest, hash_g1_point, hash_g2_point, tagged_iter, tagged_struct};
use serde::Deserialize;

struct VerificationKey {
    alpha: G1Affine,
    beta: G2Affine,
    gamma: G2Affine,
    delta: G2Affine,
    ic: Vec<G1Affine>,
}

/// JSON representation of a Groth16 verification key.
#[derive(Deserialize)]
struct VerificationKeyJson {
    /// The alpha element in G1, part of the verification key.
    alpha: PointG1Json,
    /// The beta element in G2, part of the verification key.
    beta: PointG2Json,
    /// The gamma element in G2, used in the pairing equation
    /// involving the public inputs.
    gamma: PointG2Json,
    /// The delta element in G2, used in the main pairing check
    /// during proof verification.
    delta: PointG2Json,
    /// The input coefficient (IC) points in G1.
    ///
    /// These are used to compute a linear combination of the
    /// public inputs:
    ///   acc = IC[0] + sum_i public_inputs[i] * IC[i+1].
    ///
    /// The length of this vector is typically `num_public_inputs + 1`.
    #[serde(rename = "IC")]
    ic: Vec<PointG1Json>,
}

impl VerificationKeyJson {
    pub fn to_verification_key(&self) -> VerificationKey {
        let alpha = self.alpha.to_g1_affine();
        let beta = self.beta.to_g2_affine();
        let gamma = self.gamma.to_g2_affine();
        let delta = self.delta.to_g2_affine();

        let ic: Vec<G1Affine> = self.ic.iter().map(|point| point.to_g1_affine()).collect();

        VerificationKey {
            alpha,
            beta,
            gamma,
            delta,
            ic,
        }
    }
}

#[derive(Deserialize)]
struct PointG1Json {
    x: String,
    y: String,
}

impl PointG1Json {
    pub fn to_g1_affine(&self) -> G1Affine {
        let x = Fq::from_str(&self.x).expect("Invalid field element for G1.x");
        let y = Fq::from_str(&self.y).expect("Invalid field element for G1.y");

        let point = G1Affine::new(x, y);
        assert!(point.is_on_curve());
        point
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
    pub fn to_g2_affine(&self) -> G2Affine {
        let x_im = Fq::from_str(&self.x1).expect("Invalid field element for G2.x_im");
        let x_re = Fq::from_str(&self.x2).expect("Invalid field element for G2.x_re");
        let y_im = Fq::from_str(&self.y1).expect("Invalid field element for G2.y_im");
        let y_re = Fq::from_str(&self.y2).expect("Invalid field element for G2.y_re");

        let x = Fq2::new(x_re, x_im);
        let y = Fq2::new(y_re, y_im);

        let point = G2Affine::new(x, y);
        assert!(point.is_on_curve());
        point
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
        fq2(&p.x2, &p.x1),
        fq2(&p.y2, &p.y1)
    )
}

fn compute_vk_digest(vk: &VerificationKeyJson) -> Sha256Digest {
    let alpha = vk.alpha.to_g1_affine();
    let beta = vk.beta.to_g2_affine();
    let gamma = vk.gamma.to_g2_affine();
    let delta = vk.delta.to_g2_affine();

    let alpha_hash = hash_g1_point(&alpha);
    let beta_hash = hash_g2_point(&beta);
    let gamma_hash = hash_g2_point(&gamma);
    let delta_hash = hash_g2_point(&delta);

    let ic: Vec<Sha256Digest> = vk
        .ic
        .iter()
        .map(|point| {
            let p = point.to_g1_affine();
            hash_g1_point(&p)
        })
        .collect();

    let ic_list = tagged_iter("risc0_groth16.VerifyingKey.IC", ic.into_iter());

    tagged_struct(
        "risc0_groth16.VerifyingKey",
        &[alpha_hash, beta_hash, gamma_hash, delta_hash, ic_list],
    )
}

fn compute_selector(
    control_root: &str,
    bn254_control_id: &str,
    vk_digest: Sha256Digest,
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
        &[control_root, bn254_control_id, vk_digest],
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

    // Note: Solidity's splitDigest returns (lower128, upper128) but assigns them as
    // control_root0 = upper128, control_root1 = lower128. We match that convention here.
    control_root_0.copy_from_slice(&bytes[16..32]); // Upper 128 bits
    control_root_1.copy_from_slice(&bytes[0..16]); // Lower 128 bits

    (control_root_0, control_root_1)
}

fn main() {
    let path = PathBuf::from("parameters.json");
    let data = fs::read_to_string(path).unwrap();
    let params: VerifierParameters = serde_json::from_str(&data).unwrap();

    let vk = &params.verification_key;

    // Compute all parameters (this will print intermediate values)
    let vk_digest = compute_vk_digest(vk);
    let selector = compute_selector(&params.control_root, &params.bn254_control_id, vk_digest);
    let (control_root_0, control_root_1) = compute_control_roots(&params.control_root);
    let bn254_control_id: [u8; 32] = hex::decode(params.bn254_control_id.clone())
        .expect("Invalid hex string for bn254_control_id")
        .try_into()
        .expect("bn254_control_id must be exactly 32 bytes");

    // Print key verifier parameters during build
    println!("cargo:warning===========================================");
    println!("cargo:warning=RISC Zero Groth16 Verifier Parameters");
    println!("cargo:warning===========================================");
    println!(
        "cargo:warning=SELECTOR:            {}",
        hex::encode(selector)
    );
    println!(
        "cargo:warning=CONTROL_ROOT:        {}",
        &params.control_root
    );
    println!(
        "cargo:warning=CONTROL_ROOT_0:      {}",
        hex::encode(control_root_0)
    );
    println!(
        "cargo:warning=CONTROL_ROOT_1:      {}",
        hex::encode(control_root_1)
    );
    println!(
        "cargo:warning=BN254_CONTROL_ID:    {}",
        &params.bn254_control_id
    );
    println!(
        "cargo:warning=VERIFIER_KEY_DIGEST: {}",
        hex::encode(vk_digest)
    );
    println!("cargo:warning=VERSION:             {}", &params.version);
    println!("cargo:warning===========================================");

    // Generate the VerificationKey IC array
    let ic: Vec<String> = vk.ic.iter().map(g1).collect();
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
    let bn254_control_id_code = format_byte_array(&bn254_control_id);
    let version_code = format!("\"{}\"", params.version);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("verification_key.rs"), vk_code)
        .expect("failed to write verification_key.rs");

    fs::write(out_dir.join("version.rs"), version_code).expect("failed to write version.rs");
    fs::write(out_dir.join("selector.rs"), selector_code).expect("failed to write selector.rs");

    fs::write(out_dir.join("control_root_0.rs"), control_root_0_code)
        .expect("failed to write control_root_0.rs");
    fs::write(out_dir.join("control_root_1.rs"), control_root_1_code)
        .expect("failed to write control_root_1.rs");

    fs::write(out_dir.join("bn254_control_id.rs"), bn254_control_id_code)
        .expect("failed to write bn254_control_id.rs");
}
