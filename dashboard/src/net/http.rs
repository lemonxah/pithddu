use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("pith-config")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("build reqwest client")
    })
}

pub async fn http_get(url: &str) -> (String, u16) {
    match client()
        .get(url)
        .timeout(Duration::from_secs(25))
        .send()
        .await
    {
        Ok(resp) => {
            let code = resp.status().as_u16();
            match resp.text().await {
                Ok(t) => (t, code),
                Err(_) => (String::new(), code),
            }
        }
        Err(_) => (String::new(), 0),
    }
}

pub async fn http_download_file(url: &str, path: &Path, mut on_progress: impl FnMut(f64)) -> bool {
    let resp = match client()
        .get(url)
        .timeout(Duration::from_secs(60))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return false,
    };
    let code = resp.status();
    let total = resp.content_length();
    let mut file = match tokio::fs::File::create(path).await {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(_) => return false,
        };
        if file.write_all(&chunk).await.is_err() {
            return false;
        }
        downloaded += chunk.len() as u64;
        if let Some(t) = total {
            if t > 0 {
                on_progress(downloaded as f64 / t as f64);
            }
        }
    }
    let _ = file.flush().await;
    code.is_success() || code.as_u16() == 0
}
