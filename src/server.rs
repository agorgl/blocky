use std::net::SocketAddr;
use std::path::PathBuf;

use base64;
use serde_json;
use walkdir;

use super::protocol::{Listing, ListingEntry, PatchRequest};
use fast_rsync::{diff, Signature};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode};
use pretty_bytes::converter::convert as bytes_pretty;
use sha2::{Digest, Sha256};

type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct Server {
    bind_addr: SocketAddr,
}

impl Server {
    pub fn new(addr: SocketAddr) -> Self {
        Self { bind_addr: addr }
    }

    #[tokio::main]
    pub async fn run(&self) {
        // Log mode info
        log::info!("Running in server mode...");

        // For every connection, we must make a `Service` to handle all
        // incoming HTTP requests on said connection.
        let make_svc = make_service_fn(|_conn| async {
            // This is the `Service` that will handle the connection.
            // `service_fn` is a helper to convert a function that
            // returns a Response into a `Service`.
            let service = service_fn(Self::handler);
            Ok::<_, Error>(service)
        });

        // We'll bind to given address
        //let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let addr = self.bind_addr;
        let server = hyper::Server::bind(&addr).serve(make_svc);

        // Run this server for... forever!
        if let Err(e) = server.await {
            log::error!("{}", e);
        }
    }

    async fn handler(req: Request<Body>) -> Result<Response<Body>, Error> {
        // Pass request to router
        let response = Self::router(req).await;

        // Generic internal error responses
        if let Err(e) = response {
            let response = Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Error: {}", e)))
                .unwrap();
            return Ok(response);
        }

        // Result
        response
    }

    async fn router(req: Request<Body>) -> Result<Response<Body>, Error> {
        match (req.method(), req.uri().path()) {
            (&Method::GET, "/") => Self::route_home(req).await,
            (&Method::GET, "/list") => Self::route_list(req).await,
            (&Method::POST, "/patch") => Self::route_patch(req).await,
            _ => Self::route_notfound(req).await,
        }
    }

    async fn route_home(_req: Request<Body>) -> Result<Response<Body>, Error> {
        // Greeting body
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from("Hello there"))
            .unwrap();
        Ok(response)
    }

    async fn route_list(_req: Request<Body>) -> Result<Response<Body>, Error> {
        // Fetch current working directory
        log::info!("Listing request");
        let dir = std::env::current_dir()?;

        // Gather files
        log::info!("Gathering files");
        let paths = Self::list_entries(&dir);

        // Make entries
        log::info!("Populating listing entries");
        let files = paths
            .into_iter()
            .filter_map(|path| Self::list_entry_for_file(&path).ok())
            .collect();

        // Serialize body data
        let body = move || -> Result<_, Error> {
            let listing = Listing { files };
            Ok(serde_json::to_vec_pretty(&listing).unwrap()) // TODO
        }()?;

        // Return response
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(body))
            .unwrap();
        Ok(response)
    }

    async fn route_patch(req: Request<Body>) -> Result<Response<Body>, Error> {
        // Deserialize request body
        let req_body = hyper::body::to_bytes(req.into_body()).await?;
        let patch_req = serde_json::from_slice::<PatchRequest>(&req_body)?;

        // Make path from param
        log::info!("Patch request for file {:?}", patch_req.file);
        let path = PathBuf::from(patch_req.file);

        // Decode signature into bytes
        let sigb = base64::decode(&patch_req.sig)?;

        // Create delta patch for file according to given signature
        log::info!("Making patch for file {:?}", path);
        let patch = Self::make_patch(&path, &sigb[..])?;

        // Respond with the patch
        log::info!("Patch size {}", bytes_pretty(patch.len() as f64));
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(patch))
            .unwrap();
        Ok(response)
    }

    async fn route_notfound(_req: Request<Body>) -> Result<Response<Body>, Error> {
        // Default 404 response
        let response = Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not found."))
            .unwrap();
        Ok(response)
    }

    fn list_entry_for_file(path: &PathBuf) -> Result<ListingEntry, Error> {
        // Load data
        let data = std::fs::read(path)?;

        // Calculate hash
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hasher.finalize();

        // Build entry
        Ok(ListingEntry {
            path: path.clone(),
            hash: base64::encode(&hash),
        })
    }

    fn list_entries(dir: &PathBuf) -> Vec<PathBuf> {
        walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let p = e.into_path();
                p.metadata()
                    .unwrap()
                    .is_file()
                    .then(|| p.strip_prefix(dir).unwrap().to_path_buf())
            })
            .collect()
    }

    fn make_patch(file: &PathBuf, sigb: &[u8]) -> Result<Vec<u8>, Error> {
        let data = std::fs::read(file)?;
        let sig = Signature::deserialize(&sigb)?.index();
        let mut patch = Vec::new();
        diff(&sig, &data[..], &mut patch)?;
        Ok(patch)
    }
}
