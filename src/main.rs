mod grpc;
mod fetcher;
mod ytdlp;

// my module
pub mod downloader { // include grpc module
    tonic::include_proto!("video");
}

use std::net::SocketAddr;
use tonic::transport::Server;
use crate::grpc::GrpcService;
use tracing::{info};
use tracing_subscriber;
use crate::downloader::video_service_server::VideoServiceServer;

const IP_ADDRESS: &str = "0.0.0.0";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // initialized log module
    tracing_subscriber::fmt::init();

    // launch a service
    let addr: SocketAddr = format!("{}:50051", IP_ADDRESS).parse()?;
    info!("Service in listening in {}:{}", IP_ADDRESS, 50051);

    // grpc service
    let service: GrpcService = GrpcService::new();
    Server::builder()
        .add_service(VideoServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
