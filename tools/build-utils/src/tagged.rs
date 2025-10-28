use ark_ec::AffineRepr;
use ark_serialize::CanonicalSerialize;
use sha2::{Digest, Sha256};

use crate::types::Sha256Digest;

pub fn hash_point(p: impl AffineRepr) -> Sha256Digest {
    let mut buffer = Vec::<u8>::new();
    let (x, y) = p.xy().unwrap();
    x.serialize_uncompressed(&mut buffer).unwrap();
    y.serialize_uncompressed(&mut buffer).unwrap();
    buffer.reverse();
    Sha256::digest(&buffer).into()
}

pub fn tagged_struct(tag: &str, down: &[Sha256Digest]) -> Sha256Digest {
    let tag_digest = Sha256::digest(tag.as_bytes());

    let mut tag_struct =
        Vec::<u8>::with_capacity(tag_digest.len() * (down.len() + 1) + size_of::<u16>());
    tag_struct.extend_from_slice(&tag_digest);

    for digest in down {
        tag_struct.extend_from_slice(digest);
    }

    let down_count: u16 = down
        .len()
        .try_into()
        .expect("struct defined with more than 2^16 fields");
    tag_struct.extend_from_slice(&down_count.to_le_bytes());

    Sha256::digest(tag_struct).into()
}

pub fn tagged_iter(tag: &str, iter: impl DoubleEndedIterator<Item = Sha256Digest>) -> Sha256Digest {
    iter.rfold([0u8; 32], |list_digest, elem| {
        tagged_list_cons(tag, elem, list_digest)
    })
}

fn tagged_list_cons(tag: &str, head: Sha256Digest, tail: Sha256Digest) -> Sha256Digest {
    tagged_struct(tag, &[head, tail])
}

#[cfg(test)]
mod tests {
    use super::tagged_struct;

    #[test]
    fn test_tagged_struct() {
        let digest1 = tagged_struct("foo", &[]);
        let digest2 = tagged_struct("bar", &[digest1, digest1]);
        let digest3 = tagged_struct("baz", &[digest1, digest2, digest1]);

        assert_eq!(
            hex::encode(digest3),
            "2228eb06bfbeaeb2cc12de86fd13373cb5ccdc8afac9af4299dd5a86a72afc4b"
        );
    }
}
