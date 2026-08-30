use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use async_trait::async_trait;

use super::{
    StoredBlob,
    digest::bytes_digest,
    ports::{ArtifactStore, ArtifactStoreError, PutArtifact},
};

#[derive(Default)]
struct MemoryArtifactStore {
    blobs: Mutex<HashMap<String, Vec<u8>>>,
}

#[async_trait]
impl ArtifactStore for MemoryArtifactStore {
    async fn put(&self, artifact: PutArtifact) -> Result<StoredBlob, ArtifactStoreError> {
        let actual = bytes_digest(&artifact.contents);
        if actual != artifact.expected_sha256 {
            return Err(ArtifactStoreError::DigestMismatch);
        }
        let storage_key = format!("blob:{actual}");
        self.blobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(storage_key.clone(), artifact.contents.clone());
        Ok(StoredBlob {
            storage_key,
            size_bytes: artifact.contents.len() as u64,
            sha256: actual,
        })
    }

    async fn open_verified(
        &self,
        storage_key: &str,
        expected_sha256: &str,
    ) -> Result<Vec<u8>, ArtifactStoreError> {
        let value = self
            .blobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(storage_key)
            .cloned()
            .ok_or(ArtifactStoreError::NotFound)?;
        if bytes_digest(&value) != expected_sha256 {
            return Err(ArtifactStoreError::DigestMismatch);
        }
        Ok(value)
    }
}

pub(super) fn memory_artifact_store() -> Arc<dyn ArtifactStore> {
    Arc::new(MemoryArtifactStore::default())
}
