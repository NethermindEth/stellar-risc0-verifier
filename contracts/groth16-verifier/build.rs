// This build script helps us generate the VerificationKey for the
// RiscZeroGroth16Verifier during the compilation. The key is fetched from
// `verification_key.json` and makes it available as a const to the contract. This way, the
// verification key gets included in the contract at compile time, so we don't
// have to initialize the contract and spend resources on reading from the
// ledger the verification key.

use std::{env, fs, path::PathBuf, str::FromStr};

use ark_bn254::{Fq, Fq2, G1Affine, G2Affine};
use build_utils::{Sha256Digest, tagged_iter};
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

fn main() {
    let path = PathBuf::from("vk.json");
    let data = fs::read_to_string(path).unwrap();
    let vk_json: VerificationKeyJson = serde_json::from_str(&data).unwrap();

    // Generate the VerificationKey IC array
    let ic: Vec<String> = vk_json.ic.iter().map(|p| g1(p)).collect();
    let ic = ic.join(", ");

    let code = format!(
        "VerificationKey {{
    alpha: {},
    beta: {},
    gamma: {},
    delta: {},
    ic: [{}],
}}",
        g1(&vk_json.alpha),
        g2(&vk_json.beta),
        g2(&vk_json.gamma),
        g2(&vk_json.delta),
        ic
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("verification_key.rs"), code)
        .expect("failed to write verification_key.rs");
}
