use super::protocol::{Listing, PatchRequest};
use fast_rsync::{apply, Signature, SignatureOptions};
use pretty_bytes::converter::convert as bytes_pretty;
use std::fmt::Display;
use std::path::PathBuf;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub struct Client {
    server_base: String,
    workdir: PathBuf,
}

#[derive(Debug)]
struct FilePatchStats {
    file: PathBuf,
    original_size: usize,
    patch_size: usize,
    new_size: usize,
}

impl Display for FilePatchStats {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "File {:?}: original size: {}, patch size: {}, new size: {} ({:.1}% update)",
            self.file,
            bytes_pretty(self.original_size as f64),
            bytes_pretty(self.patch_size as f64),
            bytes_pretty(self.new_size as f64),
            (self.patch_size as f64 / self.new_size as f64) * 100.0,
        )
    }
}

impl Client {
    pub fn new(server: String, directory: PathBuf) -> Self {
        Self {
            server_base: server,
            workdir: directory,
        }
    }

    #[tokio::main]
    pub async fn run(&self) {
        // Log mode info
        log::info!("Running in client mode...");

        // Run update
        if let Err(e) = self.update().await {
            log::error!("{}", e);
        }
    }

    async fn update(&self) -> Result<(), Error> {
        // Fetch list of files
        let listing = self.fetch_listing().await?;

        // Update filelist
        for file in listing.paths {
            log::info!("Updating file {:?}", file);
            let stat = self.update_file(&file).await?;
            log::info!("{}", &stat);
        }
        Ok(())
    }

    async fn fetch_listing(&self) -> Result<Listing, Error> {
        // Construct request url
        let url = format!("{}{}", self.server_base, "/list");

        // Create the client
        let client = reqwest::Client::new();

        // Make the request
        let req = client.get(&url);
        let resp = req.send().await?;
        let body = resp.json::<Listing>().await?;

        // Return result
        Ok(body)
    }

    async fn update_file(&self, file: &PathBuf) -> Result<FilePatchStats, Error> {
        // Calculate file signature
        log::info!("Calculating signature for file {:?}", file);
        let path = self.workdir.join(file);
        let data = std::fs::read(&path).unwrap_or(Vec::new());
        let sigb = Self::make_signature(&data[..]);
        let signature = base64::encode(&sigb);

        // Fetch patch for file
        log::info!("Fetching patch for file {:?}", file);
        let patch = self.fetch_patch(file, &signature).await?;

        // Apply patch
        log::info!("Applying patch for file {:?}", file);
        let mut output = Vec::new();
        apply(&data[..], &patch, &mut output)?;

        // Write file
        std::fs::create_dir_all(&path.parent().unwrap())?;
        std::fs::write(&path, &output)?;

        // Gather update stats
        let stats = FilePatchStats {
            file: file.clone(),
            original_size: data.len(),
            patch_size: patch.len(),
            new_size: output.len(),
        };
        Ok(stats)
    }

    async fn fetch_patch(&self, file: &PathBuf, sig: &String) -> Result<Vec<u8>, Error> {
        // Construct request url and body
        let url = format!("{}{}", self.server_base, "/patch");
        let req_body = PatchRequest {
            file: file.clone(),
            sig: sig.clone(),
        };
        let req_json = serde_json::to_vec_pretty(&req_body).unwrap();

        // Create the client
        let client = reqwest::Client::new();

        // Make the request
        let req = client.post(&url).body(req_json);
        let resp = req.send().await?;
        let bytes = resp.bytes().await?;

        // Return result
        Ok(bytes.to_vec())
    }

    fn make_signature(data: &[u8]) -> Vec<u8> {
        let mut signature = Vec::new();
        Signature::calculate(
            &data[..],
            &mut Vec::new(),
            SignatureOptions {
                block_size: 4096,
                crypto_hash_size: 8,
            },
        )
        .serialize(&mut signature);
        signature
    }
}
