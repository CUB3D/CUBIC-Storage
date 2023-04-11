use crate::path::{BlobPath, PathDoesntExist, PathExists};
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::Deserialize;
use serde::Serialize;
use std::os::unix::ffi::OsStrExt;

#[derive(Serialize, Deserialize, Debug)]
pub struct BlobMetadata {
    pub content_type: String,
    pub access_key: String,
    pub deletion_date: Option<DateTime<Utc>>,
}

impl Default for BlobMetadata {
    fn default() -> Self {
        let key = (0..48)
            .map(|_| rand::thread_rng().gen_range('A'..='Z'))
            .collect();
        Self {
            content_type: "text".to_string(),
            access_key: key,
            deletion_date: None,
        }
    }
}

pub struct MetadataManager {
    sled: sled::Db,
}

impl MetadataManager {
    pub fn new() -> anyhow::Result<Self> {
        let sled = sled::open("./storage_root/metadata.db")?;

        Ok(Self { sled })
    }

    pub fn get_metadata(&self, blob_path: &BlobPath<PathExists>) -> anyhow::Result<BlobMetadata> {
        let meta = self.sled.get(blob_path.as_os_str().as_bytes())?;

        let meta = match meta {
            Some(data) => {
                let data_str = String::from_utf8(data.to_vec())?;
                serde_json::from_str(&data_str)?
            }
            None => BlobMetadata::default(),
        };

        Ok(meta)
    }

    pub fn remove_metadata(&self, blob_path: &BlobPath<PathExists>) -> anyhow::Result<()> {
        self.sled.remove(blob_path.as_os_str().as_bytes())?;
        Ok(())
    }

    pub fn create_metadata(
        &self,
        blob_path: &BlobPath<PathDoesntExist>,
        metadata: &BlobMetadata,
    ) -> anyhow::Result<()> {
        let meta_json = serde_json::to_string(&metadata)?;
        assert!(!self.sled.contains_key(blob_path.as_os_str().as_bytes())?);
        self.sled
            .insert(blob_path.as_os_str().as_bytes(), meta_json.as_bytes())?;
        Ok(())
    }

    pub fn save_metadata(
        &self,
        blob_path: &BlobPath<PathExists>,
        metadata: BlobMetadata,
    ) -> anyhow::Result<()> {
        self.sled.insert(
            blob_path.as_os_str().as_bytes(),
            serde_json::to_string(&metadata)?.as_bytes(),
        )?;
        Ok(())
    }
}
