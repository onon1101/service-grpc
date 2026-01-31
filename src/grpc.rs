

use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use crate::downloader::{DownloadRequest, DownloadResponse};
use crate::downloader::video_service_server::VideoService;
use crate::fetcher::download_and_cleanup;

pub struct GrpcService {}

impl GrpcService {
    pub fn new() -> Self {
        Self {}
    }
}

#[tonic::async_trait]
impl VideoService for GrpcService {
    type DownloadAndStreamStream = ReceiverStream<Result<DownloadResponse, Status>>;

    async fn download_and_stream(&self, request: Request<DownloadRequest>) -> Result<Response<Self::DownloadAndStreamStream>, Status> {

        // GRPC request
        let req = request.into_inner();

        // MPSC
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        // spawn a Task for
        let url = req.url;
        tokio::spawn(download_and_cleanup(url, tx));

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}