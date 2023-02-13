use sqlx::postgres::{PgPool, PgPoolOptions};
use std::cmp::max;
use std::error::Error;
use std::process::{Command, Stdio};
use tonic::server;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, error, info};

use crate::wiresync::wiresync_server_api_server::{WiresyncServerApi, WiresyncServerApiServer};
use crate::wiresync::{CheckReply, CheckRequest, DeleteReply, DeleteRequest};
use crate::wiresync::{HelloReply, HelloRequest};
use crate::wiresync::{PullReply, PullRequest};
use crate::wiresync::{SubmitReply, SubmitRequest};

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
            result: "aaa".to_string(),
        };
        Ok(Response::new(reply))
    }
    async fn submit(&self, req: Request<SubmitRequest>) -> Result<Response<SubmitReply>, Status> {
        let result = submit_request(&req.get_ref(), &self.pool).await;
        match &result {
            Ok(()) => {
                let reply = super::wiresync::SubmitReply {
                    result: format!("Ok"),
                };
                Ok(Response::new(reply))
            }
            Err(e) => {
                let reply = super::wiresync::SubmitReply {
                    result: e.to_string(),
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

async fn submit_request(req: &SubmitRequest, pool: &PgPool) -> Result<(), Box<dyn Error>> {
    let in_wg_ip = decide_in_wg_ip(pool).await?;
    let duplication_check = check_duplication(pool, &req.endpoint, &req.port).await?;
    if !duplication_check {
        submit_new_server(
            pool,
            &req.endpoint,
            &req.port,
            &req.private_key,
            &req.nic_name,
            &in_wg_ip,
        )
        .await?;
        return Ok(());
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

fn interface_config_factory(
    in_wg_ip: &String,
    private_key: &String,
    port: &i32,
    nic_name: &String,
) -> Result<String, Box<dyn Error>> {
    let config_text = format!("
    [Interface]\n
    Address: {}\n
    PrivateKey: [{}]\n
    ListenPort: {}\n
    \n
    PostUp = iptables -A FORWARD -i wg0 -j ACCEPT; iptables -t nat -A POSTROUTING -o {} -j MASQUERADE\n
    PostDown = iptables -D FORWARD -i wg0 -j ACCEPT; iptables -t nat -D POSTROUTING -o {} -j MASQUERADE\n
    ", in_wg_ip, private_key, port, nic_name, nic_name);
    Ok(config_text)
}

fn peer_config_factory(
    endpoint: &String,
    port: &i32,
    private_key: &String,
    in_wg_ip: &String,
) -> Result<String, Box<dyn Error>> {
    let mut config_text = "".to_string();
    config_text = config_text
        + format!(
            "
        [Peer]\n
        Endpoint = {}:{}\n
        PublickKey = {}\n
        AllowedIPs = {}/32\n
        ",
            endpoint,
            port,
            publick_key_factory(private_key)?,
            in_wg_ip,
        )
        .as_str();
    Ok(config_text)
}

fn publick_key_factory(private_key: &String) -> Result<String, Box<dyn Error>> {
    let echo_output = Command::new("echo")
        .args([format!("{}", private_key)])
        .stdout(Stdio::piped())
        .spawn()?;
    let wg_output = Command::new("wg")
        .args(["pubkey"])
        .stdin(Stdio::from(echo_output.stdout.unwrap()))
        .output()?;
    Ok(String::from_utf8_lossy(&wg_output.stdout).to_string())
}
