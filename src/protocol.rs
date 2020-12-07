use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Listing {
    pub files: Vec<ListingEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListingEntry {
    pub path: PathBuf,
    pub hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchRequest {
    pub file: PathBuf,
    pub sig: String,
}
