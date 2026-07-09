use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use object_store::ObjectStore as ObjectStoreBackend;
use object_store::ObjectStoreExt;
use object_store::PutPayload;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectStorePath;

/// Object storage is intentionally separated from the repository layer.
/// The repository works with domain records while this store handles raw binary payloads.
#[async_trait::async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put_bytes(
        &self, object_key: &str, bytes: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn get_bytes(
        &self, object_key: &str,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>>;

    async fn list_objects(
        &self, prefix: Option<&str>,
    ) -> Result<Vec<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>>;

    async fn delete_object(
        &self, object_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn head_object(
        &self, object_key: &str,
    ) -> Result<Option<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ObjectMetadata {
    pub key: String,
    pub size: u64,
    pub last_modified: Option<String>,
    pub e_tag: Option<String>,
    pub version: Option<String>,
}

#[derive(Clone)]
pub struct FileBackedObjectStore {
    inner: Arc<dyn ObjectStoreBackend>,
    base_dir: PathBuf,
}

impl FileBackedObjectStore {
    pub fn new(
        base_dir: impl Into<PathBuf>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let base_dir = base_dir.into();
        fs::create_dir_all(&base_dir)?;
        let inner: Arc<dyn ObjectStoreBackend> =
            Arc::new(LocalFileSystem::new_with_prefix(&base_dir)?);

        Ok(Self { inner, base_dir })
    }

    pub fn default_path() -> PathBuf {
        crate::config::OBJECTS_DIR.to_path_buf()
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[async_trait::async_trait]
impl ObjectStore for FileBackedObjectStore {
    async fn put_bytes(
        &self, object_key: &str, bytes: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = ObjectStorePath::from(object_key);
        self.inner
            .put(&path, PutPayload::from_bytes(bytes.into()))
            .await?;
        Ok(())
    }

    async fn get_bytes(
        &self, object_key: &str,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let path = ObjectStorePath::from(object_key);
        match self.inner.get(&path).await {
            | Ok(result) => Ok(Some(result.bytes().await?.to_vec())),
            | Err(object_store::Error::NotFound { .. }) => Ok(None),
            | Err(err) => Err(Box::new(err)),
        }
    }

    async fn list_objects(
        &self, prefix: Option<&str>,
    ) -> Result<Vec<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let prefix = prefix.map(ObjectStorePath::from).unwrap_or_default();
        let mut results = Vec::new();
        let mut stream = self.inner.list(Some(&prefix));
        while let Some(result) = stream.next().await {
            let meta = result?;
            results.push(ObjectMetadata {
                key: meta.location.to_string(),
                size: meta.size,
                last_modified: Some(meta.last_modified.to_rfc3339()),
                e_tag: meta.e_tag.clone(),
                version: meta.version.clone(),
            });
        }
        Ok(results)
    }

    async fn delete_object(
        &self, object_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = ObjectStorePath::from(object_key);
        self.inner.delete(&path).await?;
        Ok(())
    }

    async fn head_object(
        &self, object_key: &str,
    ) -> Result<Option<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let path = ObjectStorePath::from(object_key);
        match self.inner.head(&path).await {
            | Ok(meta) => Ok(Some(ObjectMetadata {
                key: meta.location.to_string(),
                size: meta.size,
                last_modified: Some(meta.last_modified.to_rfc3339()),
                e_tag: meta.e_tag.clone(),
                version: meta.version.clone(),
            })),
            | Err(object_store::Error::NotFound { .. }) => Ok(None),
            | Err(err) => Err(Box::new(err)),
        }
    }
}

pub struct InMemoryObjectStore {
    data: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemoryObjectStore {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl ObjectStore for InMemoryObjectStore {
    async fn put_bytes(
        &self, object_key: &str, bytes: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data
            .lock()
            .unwrap()
            .insert(object_key.to_string(), bytes);
        Ok(())
    }

    async fn get_bytes(
        &self, object_key: &str,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.data.lock().unwrap().get(object_key).cloned())
    }

    async fn list_objects(
        &self, prefix: Option<&str>,
    ) -> Result<Vec<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let prefix = prefix.unwrap_or("");
        let data = self.data.lock().unwrap();
        let results: Vec<ObjectMetadata> = data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| ObjectMetadata {
                key: k.clone(),
                size: v.len() as u64,
                last_modified: None,
                e_tag: None,
                version: None,
            })
            .collect();
        Ok(results)
    }

    async fn delete_object(
        &self, object_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data.lock().unwrap().remove(object_key);
        Ok(())
    }

    async fn head_object(
        &self, object_key: &str,
    ) -> Result<Option<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let data = self.data.lock().unwrap();
        Ok(data.get(object_key).map(|v| ObjectMetadata {
            key: object_key.to_string(),
            size: v.len() as u64,
            last_modified: None,
            e_tag: None,
            version: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_uses_project_data_directory() {
        let path = FileBackedObjectStore::default_path();
        let rendered = path.to_string_lossy();

        assert!(rendered.ends_with("objects"));
        assert!(rendered.contains("vidly"));
    }

    #[tokio::test]
    async fn puts_and_reads_bytes_from_disk() {
        let temp_dir =
            std::env::temp_dir().join(format!("vidly-object-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        let store = FileBackedObjectStore::new(&temp_dir).unwrap();
        let payload = vec![0xde, 0xad, 0xbe, 0xef];

        store
            .put_bytes("sample.bin", payload.clone())
            .await
            .unwrap();
        let retrieved = store.get_bytes("sample.bin").await.unwrap().unwrap();

        assert_eq!(retrieved, payload);
    }

    #[tokio::test]
    async fn puts_and_reads_bytes_in_memory() {
        let store = InMemoryObjectStore::new();
        let payload = vec![0xca, 0xfe, 0xba, 0xbe];

        store
            .put_bytes("sample.bin", payload.clone())
            .await
            .unwrap();
        let retrieved = store.get_bytes("sample.bin").await.unwrap().unwrap();

        assert_eq!(retrieved, payload);
    }
}
