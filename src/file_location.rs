use serde::Deserialize;

#[derive(Deserialize)]
pub struct FileLocation {
    /// The bucket that contains this file
    pub bucket_name: String,

    // The path to this file within the bucket
    pub file_name: String,
}
