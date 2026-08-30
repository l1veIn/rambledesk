use sha2::{Digest, Sha256};

const IMPORT_DIGEST_VERSION: &str = "rambledesk-v2-import-v1";

pub(crate) struct CanonicalDigest(Sha256);

impl CanonicalDigest {
    pub(crate) fn new(kind: &str) -> Self {
        let mut digest = Self(Sha256::new());
        digest.field("schema", IMPORT_DIGEST_VERSION.as_bytes());
        digest.field("kind", kind.as_bytes());
        digest
    }

    pub(crate) fn field(&mut self, label: &str, value: &[u8]) {
        self.0.update((label.len() as u64).to_be_bytes());
        self.0.update(label.as_bytes());
        self.0.update((value.len() as u64).to_be_bytes());
        self.0.update(value);
    }

    pub(crate) fn finish(self) -> String {
        format!("sha256:{}", hex::encode(self.0.finalize()))
    }
}

pub(crate) fn bytes_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

pub(crate) fn deterministic_id(kind: &str, source: &str) -> String {
    let mut digest = CanonicalDigest::new("identity");
    digest.field("identity_kind", kind.as_bytes());
    digest.field("source", source.as_bytes());
    let digest = digest.finish();
    format!("migrated-{kind}-{}", &digest[7..39])
}
