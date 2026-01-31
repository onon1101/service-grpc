mod helper;

use tonic::Status;
use tracing::{error, info};
use crate::downloader::DownloadResponse;
use crate::ytdlp::helper::*;

pub async fn run(
    url: String,
    tx: &tokio::sync::mpsc::Sender<Result<DownloadResponse, Status>>,
) -> Result<String, Box<dyn std::error::Error + Sync + Send>> {

    info!("執行 ytdlp 當中...");

    // 網路上的最新的版本
    let lastest_version: String = check_for_update()
        .await
        .expect("非預期的狀況");

    // 本地當前的版本
    let local_version_wrapped: Option<String> = get_local_version().await;

    // 永遠都會獲取最新的版本當作執行檔案名稱
    let filename: String = generate_filename(&lastest_version);

    // 如果不存在或者版本不同步就更新
    if local_version_wrapped.as_ref() != Some(&lastest_version) {
        if (local_version_wrapped.is_some()) {
            let local_version = local_version_wrapped.unwrap();

            info!("偵測到 yt-dlp 版本更新。{} -> {}", local_version, lastest_version);

            // 清除本地上就得版本
            let old_filename = get_local_filename(local_version.as_str());
            cleanup_old_version(&old_filename);
        }

        // 從網路上下載最新的 ytdlp
        download_yt_dlp(&filename)
            .await
            .map_err(|e| {
                error!("無法下載 yt-dlp。{:?}", e);
                "系統發生問題"
            })?;
    }

    if url.is_empty() {
        error!("URL 為空");
        return Err("URL 為空，".into());
    }

    let real_filename = start_download_video(&filename, &url, &tx)
        .await?;

    Ok(real_filename)
}