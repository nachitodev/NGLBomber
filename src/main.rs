use reqwest::{
    Client, Proxy,
    header::{HeaderMap, HeaderValue},
};
use serde_json::json;
use std::{
    fs::File,
    io::{self, BufRead, Write},
    path::Path,
    sync::Arc,
};
use tokio::task;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let mut username = String::new();
    let mut message = String::new();
    let mut request_count = String::new();
    let mut thread_count = String::new();

    print!("Enter username: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut username).unwrap();

    print!("Enter message: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut message).unwrap();

    print!("How many requests to send?: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut request_count).unwrap();

    print!("How many concurrent threads to use?: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut thread_count).unwrap();

    let username = username.trim().to_string();
    let message = message.trim().to_string();
    let request_count: usize = request_count.trim().parse().unwrap_or(1);
    let thread_count: usize = thread_count.trim().parse().unwrap_or(1);

    let proxies = load_proxies("proxys.txt");
    if proxies.is_empty() {
        eprintln!("No valid proxies found in 'proxys.txt'");
        return;
    }

    let proxies = Arc::new(proxies);
    let url = Arc::new("https://ngl.link/api/submit".to_string());
    let mut handles = vec![];

    for thread_id in 0..thread_count {
        let username = username.clone();
        let message = message.clone();
        let url = Arc::clone(&url);
        let proxies = Arc::clone(&proxies);
        let requests_per_thread = request_count / thread_count;

        let handle = task::spawn(async move {
            let mut proxy_index = thread_id % proxies.len();
            for _ in 0..requests_per_thread {
                loop {
                    let current_proxy = &proxies[proxy_index];
                    let client = match Client::builder()
                        .proxy(Proxy::all(current_proxy).unwrap())
                        .build()
                    {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!(
                                "[Thread {}] Proxy error {}: {}",
                                thread_id, current_proxy, e
                            );
                            proxy_index = (proxy_index + 1) % proxies.len();
                            continue;
                        }
                    };
                    let uuid = Uuid::new_v4().to_string();
                    let payload = json!({
                        "username": username,
                        "question": message,
                        "deviceId": uuid
                    });
                    let mut headers = HeaderMap::new();
                    headers.insert(
                        "content-type",
                        HeaderValue::from_static("application/x-www-form-urlencoded"),
                    );
                    let res = client
                        .post(&*url)
                        .headers(headers.clone())
                        .form(&payload)
                        .send()
                        .await;
                    match res {
                        Ok(resp) => {
                            if resp.status().as_u16() == 200 {
                                match resp.json::<serde_json::Value>().await {
                                    Ok(json) => {
                                        println!("[Thread {}] Success: {:?}", thread_id, json);
                                        break;
                                    }
                                    Err(e) => {
                                        eprintln!("[Thread {}] JSON parse error: {}", thread_id, e);
                                    }
                                }
                            } else {
                                eprintln!(
                                    "[Thread {}] Status not 200: {}. Switching proxy...",
                                    thread_id,
                                    resp.status()
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!("[Thread {}] Request error: {}", thread_id, e);
                        }
                    }
                    proxy_index = (proxy_index + 1) % proxies.len();
                }
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        let _ = handle.await;
    }
    println!("Process finished.");
}

fn load_proxies<P: AsRef<Path>>(path: P) -> Vec<String> {
    match File::open(path) {
        Ok(file) => io::BufReader::new(file)
            .lines()
            .filter_map(|line| {
                let line = line.ok()?.trim().to_string();
                if line.is_empty() {
                    return None;
                }
                if line.starts_with("http://") || line.starts_with("https://") {
                    return Some(line);
                }
                if line.contains('@') {
                    let proxy_url = format!("http://{}", line);
                    return Some(proxy_url);
                } else if line.contains(':') {
                    let proxy_url = format!("http://{}", line);
                    return Some(proxy_url);
                }
                None
            })
            .collect(),
        Err(e) => {
            eprintln!("Error reading proxy file: {}", e);
            vec![]
        }
    }
}
