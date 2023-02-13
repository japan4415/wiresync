use std::error::Error;
use std::path::Path;
use std::fs::File;
use std::io::{self, Read, Write, BufReader};
use tonic::codegen::http::request;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, error, info};

use crate::Cli;
use crate::client_server::client_server;
use crate::db::delete_server;
use crate::wiresync::wiresync_server_api_client::WiresyncServerApiClient;
use crate::wiresync::{PullReply, PullRequest, wiresync_client_server_api_client, DeleteRequest, DeleteReply};

#[derive(Debug, clap::Subcommand)]
pub enum Client {
    Server(ClientServerArg),
    Delete(ClientDeleteArg),
}

#[derive(Debug, clap::Args)]
pub struct ClientServerArg {
    #[arg(long, env, default_value = "localhost")]
    pub endpoint: String,
    #[arg(short, long, env, default_value_t = 50051)]
    pub port: i32,
    #[arg(short, long, env, default_value = "wg0")]
    pub nic_name: String,
    #[arg(long, env, default_value = "localhost")]
    pub ws_endpoint: String,
    #[arg(long, env, default_value_t = 5432)]
    pub ws_port: i32,
}

#[derive(Debug, clap::Args)]
pub struct ClientDeleteArg {
    #[arg(long, env, default_value = "localhost")]
    endpoint: String,
    #[arg(short, long, env, default_value_t = 50051)]
    port: i32,
    #[arg(long, env, default_value = "localhost")]
    ws_endpoint: String,
    #[arg(long, env, default_value_t = 5432)]
    ws_port: i32,
}

pub async fn client(client_mode: &Client) -> Result<(), Box<dyn Error>> {
    match client_mode {
        Client::Server(args) => {
            let client_server_result = client_server(args).await?;
        }
        Client::Delete(args) => {
            let client_delete_result = client_delete(args).await?;
        }
    }
    Ok(())
}

pub async fn client_delete(cda: &ClientDeleteArg) -> Result<(), Box<dyn Error>> {
    let mut cli = WiresyncServerApiClient::connect(format!("http://{}:{}", cda.endpoint, cda.port)).await?;
    let request = DeleteRequest{
        endpoint: cda.endpoint.clone(),
        port: cda.port,
    };
    let response = cli.delete(request).await?;
    Ok(())
}

fn update_config(config: String) -> Result<(), Box<dyn Error>> {
    if Path::new("/etc/wireguard/ws0.conf").exists() {
        let mut file = File::open("/etc/wireguard/ws0.conf")?;
        file.write_all(config.as_bytes())?;
    } else {
        let mut file = File::create("/etc/wireguard/ws0.conf")?;
        file.write_all(config.as_bytes())?;
    }
    Ok(())
}