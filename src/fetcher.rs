use tonic::Status;
use crate::downloader::download_response::Payload;
use crate::downloader::DownloadResponse;
use crate::ytdlp;
use anyhow::{anyhow, Context, Result};
use tracing::{info, error};

pub async fn download_and_cleanup(
    url: String,
    tx: tokio::sync::mpsc::Sender<Result<DownloadResponse, Status>>
) -> Result<()> {
    let download_result = ytdlp::run(url, &tx)
        .await;

    let filename = match download_result {
        Ok(fname) => fname,
        Err(e) => {
            let _ = tx.send(Ok(DownloadResponse {
                filename: "".into(),
                payload: Some(Payload::ErrorMes(e.to_string())),
            })).await;
            return Err(anyhow!(e));
        }
    };

    info!("串流傳輸完成，準備清除檔案。{}", filename);

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    tokio::fs::remove_file(&filename)
        .await
        .with_context(|| format!("無法移除檔案: {}", filename))?;

    Ok(())
}