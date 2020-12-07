extern crate log;

mod client;
mod protocol;
mod server;

use clap::Clap;
use client::Client;
use server::Server;
use std::net::SocketAddr;
use std::path::PathBuf;

/// A delta patch utility
/// Updates a directory from a given server instance with the use of delta diffs
#[derive(Clap)]
#[clap(version = "1.0", author = "Loukas A. <agorglouk@gmail.com>")]
struct Opts {
    /// Set the mode to either client or server
    #[clap(subcommand)]
    mode: Mode,
}

#[derive(Clap)]
enum Mode {
    Client(ClientOpts),
    Server(ServerOpts),
}

#[derive(Clap)]
struct ClientOpts {
    /// Sets the patch server
    #[clap()]
    server: String,
    /// Sets the target directory.
    #[clap()]
    directory: PathBuf,
}

#[derive(Clap)]
struct ServerOpts {
    /// Sets the bind address
    #[clap()]
    bind: SocketAddr,
}

fn main() {
    // Initialize logging
    let log_env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info");
    env_logger::init_from_env(log_env);

    // Parse command line arguments and act accordingly
    let opts: Opts = Opts::parse();
    match opts.mode {
        Mode::Client(opts) => Client::new(opts.server, opts.directory).run(),
        Mode::Server(opts) => Server::new(opts.bind).run(),
    }
}
