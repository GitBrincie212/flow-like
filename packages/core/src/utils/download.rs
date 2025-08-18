use crate::state::FlowLikeState;
use flow_like_storage::files::store::FlowLikeStore;
use flow_like_storage::{Path, blake3};
use flow_like_types::intercom::{InterComCallback, InterComEvent};
use flow_like_types::reqwest::Client;
use flow_like_types::sync::{mpsc, Mutex};
use flow_like_types::tokio::fs::{self as async_fs, OpenOptions};
use flow_like_types::tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use flow_like_types::tokio::spawn;
use flow_like_types::tokio::sync::{oneshot, Semaphore};
use flow_like_types::tokio::task::yield_now;
use flow_like_types::tokio::time::Instant;
use flow_like_types::{anyhow, bail, reqwest};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::sync::Arc;
use std::time::Duration;
use std::sync::OnceLock;

#[derive(Serialize, Deserialize, Clone)]
pub struct BitDownloadEvent {
    pub max: u64,
    pub downloaded: u64,
    pub path: String,
    pub hash: String,
}

// Global concurrency limit for active downloads.
fn global_download_semaphore() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| {
        let max = std::env::var("FLOW_LIKE_MAX_CONCURRENT_DOWNLOADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(4);
        Semaphore::new(max)
    })
}

// Download job queue and dispatcher.
struct DownloadJob {
    bit: crate::bit::Bit,
    app_state: Arc<Mutex<FlowLikeState>>,
    retries: usize,
    callback: InterComCallback,
    respond_to: oneshot::Sender<flow_like_types::Result<Path>>,
}

fn global_download_queue() -> &'static mpsc::Sender<DownloadJob> {
    static TX: OnceLock<mpsc::Sender<DownloadJob>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, mut rx) = mpsc::channel::<DownloadJob>(1024);

        // Dispatcher: spawns a task per job; semaphore enforces active concurrency.
        spawn(async move {
            while let Some(job) = rx.recv().await {
                spawn(async move {
                    let _permit = match global_download_semaphore().acquire().await {
                        Ok(p) => p,
                        Err(_) => {
                            let _ = job.respond_to.send(Err(anyhow!("Download queue closed")));
                            return;
                        }
                    };

                    let res = process_download_bit(
                        &job.bit,
                        job.app_state.clone(),
                        job.retries,
                        &job.callback,
                    )
                    .await;

                    let _ = job.respond_to.send(res);
                });
            }
        });

        tx
    })
}

async fn get_remote_size(client: &Client, url: &str) -> flow_like_types::Result<u64> {
    let res = client.head(url).send().await?;
    let total_size = res
        .headers()
        .get("content-length")
        .ok_or(anyhow!("No content length"))?;

    println!("Remote file size: {:?}", total_size);

    let total_size = total_size.to_str()?;
    let size = total_size.parse::<u64>()?;
    Ok(size)
}

async fn publish_progress(
    bit: &crate::bit::Bit,
    callback: &InterComCallback,
    downloaded: u64,
    path: &Path,
) -> flow_like_types::Result<()> {
    let event = InterComEvent::with_type(
        format!("download:{}", &bit.hash),
        BitDownloadEvent {
            hash: bit.hash.to_string(),
            max: bit.size.unwrap_or(0),
            downloaded,
            path: path.to_string(),
        },
    );

    if let Err(err) = event.call(callback).await {
        println!("Error publishing progress: {}", err);
    }

    Ok(())
}

async fn feed_hasher_with_existing(
    path: &std::path::Path,
    hasher: &mut blake3::Hasher,
) -> flow_like_types::Result<u64> {
    let mut f = async_fs::File::open(path).await?;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut total = 0u64;

    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;

        // yield occasionally to keep the runtime responsive (Windows)
        if total % (8 * 1024 * 1024) == 0 {
            yield_now().await;
        }
    }

    Ok(total)
}

async fn remove_download(bit: &crate::bit::Bit, app_state: &Arc<Mutex<FlowLikeState>>) {
    let manager = app_state.lock().await.download_manager();
    manager.lock().await.remove_download(bit);
}

pub async fn download_bit(
    bit: &crate::bit::Bit,
    app_state: Arc<Mutex<FlowLikeState>>,
    retries: usize,
    callback: &InterComCallback,
) -> flow_like_types::Result<Path> {
    let (tx, rx) = oneshot::channel();
    let job = DownloadJob {
        bit: bit.clone(),
        app_state,
        retries,
        callback: callback.clone(),
        respond_to: tx,
    };

    global_download_queue()
        .send(job)
        .await
        .map_err(|_| anyhow!("Failed to enqueue download"))?;

    rx.await.map_err(|_| anyhow!("Download worker dropped"))?
}

async fn process_download_bit(
    bit: &crate::bit::Bit,
    app_state: Arc<Mutex<FlowLikeState>>,
    retries: usize,
    callback: &InterComCallback,
) -> flow_like_types::Result<Path> {
    println!("Processing download for: {}", bit.hash);
    let file_store = FlowLikeState::bit_store(&app_state).await?;

    let file_store = match file_store {
        FlowLikeStore::Local(store) => store,
        _ => bail!("Only local store supported"),
    };

    let store_path =
        Path::from(bit.hash.clone()).child(bit.file_name.clone().ok_or(anyhow!("No file name"))?);
    let path_name = file_store.path_to_filesystem(&store_path)?;
    let url = bit
        .download_link
        .clone()
        .ok_or(anyhow!("No download link"))?;

    // Another download of that type already exists
    let exists = {
        let manager = app_state.lock().await.download_manager();
        let manager = manager.lock().await;
        manager.download_exists(bit)
    };

    if exists {
        bail!("Download already exists");
    }

    let client = {
        let manager = app_state.lock().await.download_manager();
        let mut manager = manager.lock().await;
        manager.add_download(bit)
    };

    if client.is_none() {
        let _rem = remove_download(bit, &app_state).await;
        bail!("Download already exists");
    }

    let client = client.ok_or(anyhow!("No client for download"))?;
    let mut resume = false;
    let remote_size = get_remote_size(&client, &url).await;

    if remote_size.is_err() {
        if async_fs::try_exists(&path_name).await.unwrap_or(false) {
            let _rem = remove_download(bit, &app_state).await;
            let local_len = async_fs::metadata(&path_name)
                .await
                .ok()
                .map(|m| m.len())
                .unwrap_or(0);
            let _ = publish_progress(bit, callback, local_len, &store_path).await;
            return Ok(store_path);
        }

        bail!(
            "Error getting remote size for {}: {}",
            &url,
            remote_size.unwrap_err()
        );
    }

    let remote_size = remote_size?;

    let mut local_size = 0;
    if async_fs::try_exists(&path_name).await.unwrap_or(false) {
        local_size = async_fs::metadata(&path_name)
            .await
            .ok()
            .map(|m| m.len())
            .unwrap_or(0);
        if local_size == remote_size {
            let _rem = remove_download(bit, &app_state).await;
            let _ = publish_progress(bit, callback, remote_size, &store_path).await;
            return Ok(store_path);
        }

        if local_size < remote_size {
            resume = true;
            println!("Resuming download: {} to {}", &url, path_name.display());
        }

        if local_size > remote_size {
            println!(
                "Local file is bigger than remote file, deleting: {}",
                path_name.display()
            );
            let _ = async_fs::remove_file(&path_name).await;
        }
    }

    println!("Downloading: {} to {}", &url, store_path);

    // now use range header to resume download
    let mut headers = reqwest::header::HeaderMap::new();

    if resume {
        headers.insert("Range", format!("bytes={}-", local_size).parse()?);
    }

    let res = match client.get(&url).headers(headers).send().await {
        Ok(res) => res,
        Err(e) => {
            let _rem = remove_download(bit, &app_state).await;
            bail!("Error downloading file {}", e);
        }
    };

    if let Some(parent) = path_name.parent() {
        async_fs::create_dir_all(parent).await?;
    }

    let file = match OpenOptions::new()
        .write(true)
        .append(resume)
        .truncate(!resume)
        .create(true)
        .open(&path_name)
        .await
    {
        Ok(file) => file,
        Err(e) => {
            let _rem = remove_download(bit, &app_state).await;
            println!("Error opening file: {:?}", e);
            bail!("Error opening file {}", e);
        }
    };

    let mut file = BufWriter::with_capacity(1 << 20, file);

    let mut downloaded: u64 = 0;
    let mut hasher = blake3::Hasher::new();

    if resume {
        feed_hasher_with_existing(&path_name, &mut hasher).await?;
        downloaded = local_size;
    }

    let mut stream = res.bytes_stream();
    let mut in_buffer = 0;
    let mut since_yield = 0usize;
    let mut last_emit = Instant::now();

    while let Some(item) = stream.next().await {
        let chunk = match item {
            Ok(chunk) => chunk,
            Err(_) => {
                continue;
            }
        };

        hasher.update(&chunk);

        if file.write_all(&chunk).await.is_err() {
            continue;
        }

        in_buffer += chunk.len();
        since_yield += chunk.len();

        let new = min(downloaded + (chunk.len() as u64), remote_size);
        downloaded = new;

        // if buffer is bigger than 20 mb flush
        if in_buffer > 20_000_000 && file.flush().await.is_ok() {
            in_buffer = 0;
        }

        if last_emit.elapsed() >= Duration::from_millis(150) {
            let _ = publish_progress(bit, callback, new, &store_path).await;
            last_emit = Instant::now();
        }

        if since_yield >= 8 * 1024 * 1024 {
            yield_now().await;
            since_yield = 0;
        }
    }

    let _ = file.flush().await;
    let inner = file.get_mut();
    let _ = inner.sync_all().await;

    let _rem = remove_download(bit, &app_state).await;

    let file_hash = hasher.finalize().to_hex().to_string().to_lowercase();
    if file_hash != bit.hash.to_lowercase() {
        println!(
            "Error downloading file, hash does not match, deleting __ {} != {}",
            file_hash, bit.hash
        );
        let _ = async_fs::remove_file(&path_name).await;
        if retries > 0 {
            println!("Retrying download: {}", bit.hash);
            let result = Box::pin(process_download_bit(bit, app_state, retries - 1, callback));
            return result.await;
        }
        bail!("Error downloading file, hash does not match");
    }

    Ok(store_path)
}

