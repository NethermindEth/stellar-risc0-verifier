use ark_bn254::{Fq, Fq2, Fr as AFr, G1Affine as AG1Affine, G2Affine as AG2Affine};
use ark_ff::BigInteger256;
use soroban_sdk::{BytesN, contracttype};

/// BN254 scalar field element with XDR serialization support.
///
/// Stored as a 32-byte big-endian value.
#[derive(Clone)]
#[contracttype]
pub struct Fr {
    pub value: BytesN<32>,
}

/// BN254 G1 point with XDR serialization support.
///
/// Coordinates are stored as 32-byte big-endian values
#[derive(Clone)]
#[contracttype]
pub struct G1Affine {
    pub x: BytesN<32>,
    pub y: BytesN<32>,
}

/// BN254 G2 point with XDR serialization support.
///
/// G2 points have coordinates in the extension field Fq2, where each coordinate
/// is represented as a pair of base field elements. Each component is stored as
/// a 32-byte big-endian value: `x = x_0 + x_1 * u` and `y = y_0 + y_1 * u`.
///
/// Note: x_0 and y_0 are the real parts (c0), x_1 and y_1 are the imaginary parts (c1).
#[derive(Clone)]
#[contracttype]
pub struct G2Affine {
    pub x_0: BytesN<32>,
    pub x_1: BytesN<32>,
    pub y_0: BytesN<32>,
    pub y_1: BytesN<32>,
}

impl From<&G1Affine> for AG1Affine {
    fn from(point: &G1Affine) -> Self {
        let x_limbs = bytes_to_limbs(&point.x.to_array());
        let y_limbs = bytes_to_limbs(&point.y.to_array());

        let x = Fq::from(x_limbs);
        let y = Fq::from(y_limbs);

        AG1Affine::new(x, y)
    }
}

impl From<&G2Affine> for AG2Affine {
    fn from(point: &G2Affine) -> Self {
        let x0_limbs = bytes_to_limbs(&point.x_0.to_array());
        let x1_limbs = bytes_to_limbs(&point.x_1.to_array());

        let y0_limbs = bytes_to_limbs(&point.y_0.to_array());
        let y1_limbs = bytes_to_limbs(&point.y_1.to_array());

        let x = Fq2::new(Fq::from(x0_limbs), Fq::from(x1_limbs));
        let y = Fq2::new(Fq::from(y0_limbs), Fq::from(y1_limbs));

        AG2Affine::new(x, y)
    }
}

impl From<Fr> for AFr {
    fn from(scalar: Fr) -> Self {
        let limbs = bytes_to_limbs(&scalar.value.to_array());
        AFr::from(limbs)
    }
}

/// Converts 32 bytes in big-endian format to a 4-limb little-endian representation.
///
/// This helper function performs the endianness conversion required by the
/// arkworks library. It takes a 32-byte big-endian array and converts it to
/// a `BigInteger256` with four u64 limbs in little-endian order.
fn bytes_to_limbs(bytes: &[u8; 32]) -> BigInteger256 {
    let mut limbs = [0u64; 4];
    for i in 0..4 {
        let start = i * 8;
        limbs[3 - i] = u64::from_be_bytes(bytes[start..start + 8].try_into().unwrap());
    }
    BigInteger256::new(limbs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::{G1Projective, G2Projective};
    use ark_ec::Group;
    use ark_ff::{BigInt, BigInteger, PrimeField};
    use soroban_sdk::Env;

    #[test]
    fn test_fr_conversion_known_value() {
        let env = Env::default();

        // Create a known scalar value
        let expected = AFr::from(42u64);
        let bigint: BigInt<4> = expected.into_bigint();

        // Convert to big-endian bytes
        let bytes: [u8; 32] = bigint.to_bytes_be().try_into().unwrap();

        let fr = Fr {
            value: BytesN::from_array(&env, &bytes),
        };

        let ark_fr: AFr = fr.into();
        assert_eq!(ark_fr, expected);
    }

    #[test]
    fn test_g1_conversion_generator() {
        let env = Env::default();

        // BN254 G1 generator point
        let generator = AG1Affine::from(G1Projective::generator());
        let (x_bigint, y_bigint) = (generator.x.into_bigint(), generator.y.into_bigint());

        // Convert to big-endian bytes
        let x_bytes: [u8; 32] = x_bigint.to_bytes_be().try_into().unwrap();
        let y_bytes: [u8; 32] = y_bigint.to_bytes_be().try_into().unwrap();

        let g1 = G1Affine {
            x: BytesN::from_array(&env, &x_bytes),
            y: BytesN::from_array(&env, &y_bytes),
        };

        let ark_g1: AG1Affine = (&g1).into();
        assert_eq!(ark_g1, generator);
    }

    #[test]
    fn test_g2_conversion_generator() {
        let env = Env::default();

        // BN254 G2 generator point
        let generator = AG2Affine::from(G2Projective::generator());
        let (x, y) = (generator.x, generator.y);

        // Convert Fq2 coordinates to bytes
        let x0_bigint = x.c0.into_bigint();
        let x1_bigint = x.c1.into_bigint();
        let y0_bigint = y.c0.into_bigint();
        let y1_bigint = y.c1.into_bigint();

        let x0_bytes: [u8; 32] = x0_bigint.to_bytes_be().try_into().unwrap();
        let x1_bytes: [u8; 32] = x1_bigint.to_bytes_be().try_into().unwrap();
        let y0_bytes: [u8; 32] = y0_bigint.to_bytes_be().try_into().unwrap();
        let y1_bytes: [u8; 32] = y1_bigint.to_bytes_be().try_into().unwrap();

        let g2 = G2Affine {
            x_0: BytesN::from_array(&env, &x0_bytes),
            x_1: BytesN::from_array(&env, &x1_bytes),
            y_0: BytesN::from_array(&env, &y0_bytes),
            y_1: BytesN::from_array(&env, &y1_bytes),
        };

        let ark_g2: AG2Affine = (&g2).into();
        assert_eq!(ark_g2, generator);
    }
}
