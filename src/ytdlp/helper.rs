use std::{env, fs, io};
use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use std::process::Command;
use anyhow::{Result, anyhow};
use regex::Regex;
use tokio::io::AsyncReadExt;
use tonic::Status;
use tracing::{error, info};
use crate::downloader::download_response::Payload;
use crate::downloader::{download_response, DownloadResponse};

#[cfg(windows)]
pub const YT_DLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";

#[cfg(not(windows))]
pub const YT_DLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp";

pub async fn check_for_update() -> Result<String> {
    const GITHUB_REPO: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
    let client = reqwest::Client::new();

    let response = client
        .get(GITHUB_REPO)
        .header("User-Agent", "rust_downloader")
        .send()
        .await?;

    let latest_version = response.json::<serde_json::Value>()
        .await?
        ["tag_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    Ok(latest_version)
}

pub async fn get_local_version() -> Option<String> {
    let pattern = r"yt-dlp_([\d\.]+)(?:\.exe)?$";
    let re = Regex::new(pattern).unwrap();

    let current_folder_path =  env::current_dir().unwrap();

    let all_files = fs::read_dir(current_folder_path.as_os_str())
        .unwrap()
        .filter_map(|res| res.ok());

    for file in all_files {
        let filename = file.file_name();
        let name_str = filename.to_string_lossy();

        if let Some(caps) = re.captures(&name_str) {
            // println!("{}", caps.get(1).unwrap().as_str());
            return Some(caps[1].to_string());
        }
    }

    None
}

pub fn generate_filename(vesrion: &str) -> String {
    #[cfg(windows)]
    {
        format!("yt-dlp-{}.exe", vesrion)
    }
    #[cfg(not(windows))]
    {
        format!("yt-dlp-{}", vesrion)
    }
}

pub fn get_local_filename(version: &str) -> String {
    #[cfg(windows)]
    {
        format!("yt-dlp-{}.exe", version)
    }
    #[cfg(not(windows))]
    {
        format!("yt-dlp-{}", version)
    }
}

pub fn cleanup_old_version(old_filename: &str) {
    let current_folder_path =  env::current_dir().unwrap();

    let all_files = fs::read_dir(current_folder_path.as_os_str())
        .unwrap()
        .filter_map(|res| res.ok());

    for file in all_files {
        let filename = file.file_name();
        let name_str = filename.to_string_lossy();

        // 刪除舊的 yt-dlp，並且確保刪除的不是剛剛下載完得版本
        if name_str.contains(old_filename) && name_str != old_filename {
            if let Err(e) = std::fs::remove_file(file.path()) {
                error!("無法刪除舊的版本 yt-dlp。{}", e );
            } else {
                info!("已清除舊版本");
            }
        }
    }
}

pub async fn download_yt_dlp(filename: &String) -> Result<(), Box<dyn std::error::Error>> {
    let filename_str: &str = filename.as_str();
    let path = Path::new(filename_str);
    if path.exists() {
        println!("yt-dlp 已存在。");
        return Ok(());
    }

    println!("🚀 正在從 GitHub 下載最新的 yt-dlp...");
    let response = reqwest::get(YT_DLP_URL).await?;
    let mut file = File::create(filename_str)?;
    let mut content = Cursor::new(response.bytes().await?);
    io::copy(&mut content, &mut file)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(filename_str)?.permissions();
        perms.set_mode(0o755); // 賦予執行權限
        fs::set_permissions(filename_str, perms)?;
    }

    println!("✅ 下載成功並已設定權限。");
    Ok(())
}

async fn get_mp4_filenames() -> Result<Vec<String>> {
   let mut mp4_files = Vec::new();

    let current_folder_path =  env::current_dir()?;
    let mut entries = tokio::fs::read_dir(current_folder_path.as_os_str()).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("mp4") {
            if let Some (file_name) = path.file_name().and_then(|s| s.to_str()) {
                mp4_files.push(file_name.to_string());
            }
        }
    }

    Ok(mp4_files)
}

pub async fn start_download_video(
    filename: &String,
    url: &String,
    tx: &tokio::sync::mpsc::Sender<std::result::Result<DownloadResponse, Status>>
) -> Result<String> {
    let mut child = tokio::process::Command::new(format!("./{}", &filename))
        .args(["-f", "bv+ba/b"])
        .args(["-S", "br,res,fps"])
        .args(["--merge-output-format", "mp4"])
        .arg(url)
        .spawn()?;

    let status = child.wait().await?;

    let videos = get_mp4_filenames()
        .await?;

    if videos.len() != 1 {
        error!("當前目錄存在多個 mp4，不確定何者為當前正在下載的值");
        return Err(anyhow!("系統內部問題"));
    }

    let video_filename = videos.into_iter()
        .next()
        .ok_or_else(|| anyhow!("Videos iter 發生問題"))?;

    let mut last_progress = -1;
    if status.success() {

        // initialize value
        let mut file = tokio::fs::File::open(&video_filename).await?;
        let total_size = file.metadata()
            .await?
            .len() as f64;
        let mut sent_size = 0f64;
        let mut buffer = [0u8; 64 * 1024];

        // starting transfer
        while let Ok(n) = file.read(&mut buffer).await {
            if n == 0 { break; }

            sent_size += n as f64;
            let current_progress = ((sent_size / total_size) * 100.0) as i32;
            if current_progress > last_progress {
                tx.send(Ok(DownloadResponse {
                    payload: Some(download_response::Payload::Progress(current_progress)),
                    filename: filename.clone(),
                })).await?;
                last_progress = current_progress;
            }

            let _ = tx
                .send(Ok(DownloadResponse {
                    filename: filename.clone(),
                    payload: Some(Payload::FileChunk(buffer[..n].to_vec())),
                }))
                .await;
        }
    }
    Ok(video_filename)
}
