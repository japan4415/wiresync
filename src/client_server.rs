use std::error::Error;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, error, info};

use crate::client::{ClientServerArg};
use crate::wiresync::wiresync_client_server_api_server::{WiresyncClientServerApi, WiresyncClientServerApiServer};
use crate::wiresync::{PullReply, PullRequest, UpdateConfigRequest, UpdateConfigReply, HelloRequest, HelloReply};

use crate::lib::{interface_config_factory, peer_config_factory};

#[derive(Debug)]
pub struct MyWiresyncClientServerApi {}

pub async fn client_server(cs: &ClientServerArg) -> Result<(), Box<dyn Error>> {
    let addr = format!("[::1]:{}", cs.port).parse().unwrap();
    let myWiresyncClientServerApi = MyWiresyncClientServerApi {};
    info!("GreeterServer listening on {}", addr);
    Server::builder()
        .add_service(WiresyncClientServerApiServer::new(myWiresyncClientServerApi))
        .serve(addr)
        .await?;
    Ok(())
}

#[tonic::async_trait]
impl WiresyncClientServerApi for MyWiresyncClientServerApi {
    async fn say_hello(&self, request: Request<HelloRequest>) -> Result<Response<HelloReply>, Status> {
        let reply = HelloReply {
            message: format!("Hello!"),
        };
        Ok(Response::new(reply))
    }
    async fn update_config(
        &self,
        request: Request<UpdateConfigRequest>,
    ) -> Result<Response<UpdateConfigReply>, Status> {
        println!("Got a request from {:?}", request.remote_addr());
        let reply = UpdateConfigReply {
            result: format!("Hello!"),
        };
        Ok(Response::new(reply))
    }
}

async fn update_config(
    request: Request<UpdateConfigRequest>,
) -> Result<Response<UpdateConfigReply>, Status> {
    let config_string = request.into_inner().config;
    let reply = UpdateConfigReply {
        result: format!("Ok"),
    };
    Ok(Response::new(reply))
}