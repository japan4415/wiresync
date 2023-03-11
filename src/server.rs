use sqlx::postgres::{PgPool, PgPoolOptions};
use tokio::io::Interest;
use tonic::codegen::http::response;
use std::cmp::max;
use std::error::Error;
use std::process::{Command, Stdio};
use tonic::server;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, error, info};

use crate::wiresync::wiresync_client_server_api_client::WiresyncClientServerApiClient;
use crate::wiresync::wiresync_server_api_server::{WiresyncServerApi, WiresyncServerApiServer};
use crate::wiresync::{CheckReply, CheckRequest, DeleteReply, DeleteRequest, UpdateConfigReply, UpdateConfigRequest};
use crate::wiresync::{HelloReply, HelloRequest};
use crate::wiresync::{PullReply, PullRequest};
use crate::wiresync::{SubmitReply, SubmitRequest};
use crate::lib::{interface_config_factory, peer_config_factory};

use crate::db::{
    check_change, check_duplication, get_all_ip, get_interface_data, get_peer_datas,
    submit_new_server, update_server, delete_server, ServerData,
};
use crate::errors::WireSyncError;

#[derive(Debug, clap::Args)]
pub struct ServerArg {
    #[arg(long, env, default_value_t = 50051)]
    port: i32,
    #[arg(long, env, default_value = "localhost")]
    db_endpoint: String,
    #[arg(long, env, default_value_t = 5432)]
    db_port: i32,
    #[arg(long, env, default_value = "user")]
    db_user: String,
    #[arg(long, env, hide_env_values = true, default_value = "password")]
    db_password: String,
}

#[derive(Debug)]
pub struct ServerConfig {
    endpoint: String,
    port: i32,
    privateKey: String,
    nicName: String,
    inWgIp: String,
}

pub async fn server(s: &ServerArg) -> Result<(), Box<dyn Error>> {
    let pool = super::db::make_pool(&s.db_endpoint, s.db_port, &s.db_user, &s.db_password).await?;
    super::db::init_db(&pool).await?;
    let addr = format!("[::1]:{}", s.port).parse().unwrap();
    let wiresync = MyWiresyncServerApi { pool };
    info!("GreeterServer listening on {}", addr);
    Server::builder()
        .add_service(WiresyncServerApiServer::new(wiresync))
        .serve(addr)
        .await?;
    Ok(())
}

#[derive(Debug)]
pub struct MyWiresyncServerApi {
    pool: PgPool,
}

#[tonic::async_trait]
impl WiresyncServerApi for MyWiresyncServerApi {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        println!("Got a request from {:?}", request.remote_addr());
        let reply = super::wiresync::HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };
        Ok(Response::new(reply))
    }
    async fn check(&self, req: Request<CheckRequest>) -> Result<Response<CheckReply>, Status> {
        let reply = super::wiresync::CheckReply {
            result: "".to_string(),
        };
        Ok(Response::new(reply))
    }
    async fn submit(&self, req: Request<SubmitRequest>) -> Result<Response<SubmitReply>, Status> {
        let result = submit_request(&req.get_ref(), &self.pool).await;
        match &result {
            Ok(config_text) => {
                let reply = super::wiresync::SubmitReply {
                    config: config_text.to_string(),
                };
                Ok(Response::new(reply))
            }
            Err(e) => {
                let reply = super::wiresync::SubmitReply {
                    config: e.to_string(),
                };
                Ok(Response::new(reply))
            }
        }
    }
    async fn pull(&self, req: Request<PullRequest>) -> Result<Response<PullReply>, Status> {
        let result = pull_request(&req.get_ref(), &self.pool).await;
        match &result {
            Ok(s) => {
                let reply = super::wiresync::PullReply {
                    result: s.to_string(),
                };
                Ok(Response::new(reply))
            }
            Err(e) => {
                let reply = super::wiresync::PullReply {
                    result: e.to_string(),
                };
                Ok(Response::new(reply))
            }
        }
    }
    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteReply>, Status> {
        let result = delete_request(&request.get_ref(), &self.pool).await;
        match &result {
            Ok(s) => {
                let reply = super::wiresync::DeleteReply {
                    result: format!("ok"),
                };
                Ok(Response::new(reply))
            }
            Err(e) => {
                let reply = super::wiresync::DeleteReply {
                    result: e.to_string(),
                };
                Ok(Response::new(reply))
            }
        }
    }
}

async fn submit_request(req: &SubmitRequest, pool: &PgPool) -> Result<String, Box<dyn Error>> {
    let in_wg_ip = decide_in_wg_ip(pool).await?;
    let duplication_check = check_duplication(pool, &req.endpoint, &req.port).await?;
    if !duplication_check {
        submit_new_server(
            pool,
            &req.endpoint,
            &req.port,
            &req.wireguard_port,
            &req.private_key,
            &req.nic_name,
            &in_wg_ip,
        )
        .await?;
        let config_text = config_text_factory(&req.endpoint, &req.port, pool).await?;
        return Ok(config_text);
    } else {
        return Err(Box::new(WireSyncError::ServerDuplicatedError((
            req.endpoint.clone(),
            req.port,
        ))));
    }
}

async fn check_request(req: &CheckRequest, pool: &PgPool) -> Result<String, Box<dyn Error>> {
    let check_change_result = check_change(
        pool,
        &req.endpoint,
        &req.port,
        &req.private_key,
        &req.nic_name,
        &req.in_wg_ip,
    )
    .await?;
    if check_change_result {
        update_server(
            pool,
            &req.endpoint,
            &req.port,
            &req.private_key,
            &req.nic_name,
            &req.in_wg_ip,
        )
        .await?;
    }
    Ok(format!(""))
}

async fn pull_request(req: &PullRequest, pool: &PgPool) -> Result<String, Box<dyn Error>> {
    let mut result = "".to_string();
    let interface_data = get_interface_data(&req.endpoint, &req.port, pool).await?;
    result += &interface_config_factory(
        &interface_data.inwgip,
        &interface_data.privatekey,
        &i32::try_from(interface_data.port)?,
        &interface_data.nicname,
    )?;
    let peer_datas = get_peer_datas(&req.endpoint, &req.port, pool).await?;
    for peer_data in peer_datas {
        result += &peer_config_factory(
            &peer_data.endpoint,
            &i32::try_from(peer_data.port)?,
            &peer_data.privatekey,
            &peer_data.inwgip,
        )?
    }
    Ok(result)
}

async fn delete_request(req: &DeleteRequest, pool: &PgPool) -> Result<(), Box<dyn Error>> {
    let delete_server_result = delete_server(pool, &req.endpoint, &req.port).await?;
    Ok(delete_server_result)
}

async fn decide_in_wg_ip(pool: &PgPool) -> Result<String, Box<dyn Error>> {
    let ips = get_all_ip(pool).await?;
    let target_ip = get_next_ip(ips);
    Ok(target_ip)
}

fn get_next_ip(ips: Vec<(i32, i32, i32, i32)>) -> String {
    if ips.len() == 0 {
        return "10.0.0.1".to_string();
    } else {
        let mut ip_tuple: (i32, i32, i32, i32) = (10, 0, 0, 1);
        for ip in &ips {
            ip_tuple = (10, max(ip_tuple.1, ip.1), 0, 1);
        }
        for ip in &ips {
            if ip_tuple.1 == ip_tuple.1 {
                ip_tuple = (10, ip_tuple.1, max(ip_tuple.2, ip.2), 1);
            }
        }
        for ip in &ips {
            if ip_tuple.2 == ip_tuple.2 {
                ip_tuple = (10, ip_tuple.1, ip_tuple.2, max(ip_tuple.3, ip.3));
            }
        }
        return format!(
            "{}.{}.{}.{}",
            ip_tuple.0,
            ip_tuple.1,
            ip_tuple.2,
            ip_tuple.3 + 1
        );
    }
}

async fn config_text_factory(endpoint: &String, port: &i32, pool: &PgPool) -> Result<String, Box<dyn Error>> {
    let mut result = "".to_string();
    let interface_data = get_interface_data(endpoint, port, pool).await?;
    result += &interface_config_factory(&interface_data.inwgip, &interface_data.privatekey, &interface_data.port, &interface_data.nicname)?;
    let peer_datas = get_peer_datas(endpoint, port, pool).await?;
    for peer_data in peer_datas {
        if &peer_data.endpoint != endpoint || &peer_data.port != port {
            result += &peer_config_factory(&peer_data.endpoint, &peer_data.port, &peer_data.privatekey, &peer_data.inwgip)?;
        }
    }
    Ok(result)
}

async fn request_update_config(endpoint: &String, port: &i32, wiresync_port: &i32, pool: &PgPool) -> Result<(), Box<dyn Error>> {
    let config_text = config_text_factory(endpoint, port, pool).await?;
    let mut cli = WiresyncClientServerApiClient::connect(format!("http://{}:{}", endpoint, wiresync_port)).await?;
    let request = UpdateConfigRequest {
        config: config_text,
    };
    let response = cli.update_config(request).await?;
    Ok(())
}