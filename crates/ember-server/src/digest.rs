use sha2::{Digest, Sha256};

pub fn update_length_prefixed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(bytes);
}
