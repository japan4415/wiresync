use std::error::Error;
use std::process::Command;
use clap::{Parser, Subcommand, Error as ClapError};
use tonic::{transport::Server, Request, Response, Status};
use tracing::log::log;
use sqlx::{Pool, Error as SqlxError, Row};
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::{info, error, debug};
use tracing_subscriber;

mod lib;
mod errors;
mod db;
mod server;
mod client;
mod client_server;
mod wiresync {
    tonic::include_proto!("wiresync");
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli{
    #[command(subcommand)]
    mode: Option<Modes>,
}

#[derive(Subcommand, Debug)]
enum Modes {
    Server (server::ServerArg),
    #[command(subcommand)]
    Client (client::Client),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    info!("command start");
    let cli = Cli::parse();
    match &cli.mode {
        Some(Modes::Server(args)) => {
            let server_result = server::server(args).await?;
        }
        Some(Modes::Client(args)) => {
            info!("client");
            let client_result = client::client(args).await?;
        }
        None => {error!("no mode!")}
    }
    Ok(())
}