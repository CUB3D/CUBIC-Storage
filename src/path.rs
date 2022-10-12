use std::marker::PhantomData;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use actix_web::web::Data;
use crate::settings::AppSettings;


pub struct PathExists;
pub struct PathDoesntExist;

pub struct BlobPath<Marker>(PathBuf, PhantomData<Marker>);

impl<T> Deref for BlobPath<T> {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct BucketPath<Marker>(PathBuf, PhantomData<Marker>);

impl<T> Deref for BucketPath<T> {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct PathManager {
    settings: Data<AppSettings>,
}

impl PathManager {
    pub fn new(settings: Data<AppSettings>) -> Self {
        Self {
            settings
        }
    }


    fn get_root(&self) -> PathBuf {
        PathBuf::from(&self.settings.storage_root)
    }

    /// Safely join a path to the root
    /// This returned path can be assumed to:
    /// - Only point into the specified root directory
    /// - Not point to, or contain, a symlink
    fn safe_join(&self, root: &Path, path: &Path) -> Option<PathBuf> {
        let mut target = root.to_owned();

        for comp in path.components() {
            match comp {
                Component::Prefix(_) =>
                /* Ignored, drive/share on windows is defined by our root */
                    {}
                Component::RootDir =>
                /*Ignored, we always start from *our* root*/
                    {}
                Component::CurDir =>
                /*Do nothing, we are always in current dir*/
                    {}
                Component::ParentDir =>
                /* Ignore, we can't go up a dir */
                    {}
                Component::Normal(path) => {
                    target = target.join(path);
                }
            }

            // Can't have symlinks in safe paths
            if target.is_symlink() {
                return None;
            }
        }


        // Safety check, ALL paths MUST be under the root dir
        assert!(target.starts_with(root.as_os_str()));

        Some(target)
    }

    /// Convert the given bucket name to a new bucket path
    /// This returned path can be assumed to:
    /// - Point to a new, non existent, bucket
    /// - Hold all the assumptions of [Self::safe_join]
    pub fn create_bucket(&self, bucket_name: &Path) -> Option<BucketPath<PathDoesntExist>> {
        let path = self.safe_join(&self.get_root(), &bucket_name)?;

        // End result must *not* exist
        if path.exists() {
            return None;
        }

        Some(BucketPath(path, Default::default()))
    }

    /// Convert the given bucket name to a bucket path
    /// This returned path can be assumed to:
    /// - Point to a valid, existing bucket (note that this bucket could be modified in between this call and its use, check for TOCTOU)
    /// - Hold all the assumptions of [Self::safe_join]
    pub fn get_bucket(&self, bucket_name: &Path) -> Option<BucketPath<PathExists>> {
        let path = self.safe_join(&self.get_root(), &bucket_name)?;

        // End result must exist
        if !path.exists() {
            return None;
        }

        Some(BucketPath(path, Default::default()))
    }

    /// Convert the given bucket name and file to a new blob path
    /// This returned path can be assumed to:
    /// - Point to a non existent, file, within a valid bucket
    /// - Hold all the assumptions of [Self::safe_join]
    pub fn create_bucket_file(&self, bucket: &BucketPath<PathExists>, file: &Path) -> Option<BlobPath<PathDoesntExist>> {
        let path = self.safe_join(&*bucket, file)?;

        // End result must *not* exist
        if path.exists() {
            return None;
        }

        Some(BlobPath(path, Default::default()))
    }

    /// Convert the given bucket name and file to a blob path
    /// This returned path can be assumed to:
    /// - Point to a valid, existing file (note that this file could be modified in between this call and its use, check for TOCTOU)
    /// - Hold all the assumptions of [Self::safe_join]
    pub fn get_bucket_file(&self, bucket: &BucketPath<PathExists>, file: &Path) -> Option<BlobPath<PathExists>> {
        let path = self.safe_join(&*bucket, file)?;

        // End result must exist
        if !path.exists() {
            return None;
        }

        Some(BlobPath(path, Default::default()))
    }
}
