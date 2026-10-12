use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use url::Url;

pub struct Reply {
    pub status: u16,
    pub mime: &'static str,
    pub body: &'static str,
}

impl Reply {
    pub fn json(body: &'static str) -> Self {
        Self {
            status: 200,
            mime: "application/json",
            body,
        }
    }
    pub fn html(body: &'static str) -> Self {
        Self {
            status: 200,
            mime: "text/html",
            body,
        }
    }
    pub fn error(status: u16) -> Self {
        Self {
            status,
            mime: "text/plain",
            body: "API unavailable",
        }
    }
}

pub struct Server {
    pub base: Url,
    requests: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Server {
    pub async fn start(replies: impl IntoIterator<Item = Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let mut replies: VecDeque<_> = replies.into_iter().collect();
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                let request = String::from_utf8(request).unwrap();
                recorded.lock().unwrap().push(
                    request
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .to_owned(),
                );
                let reply = replies.pop_front().unwrap_or_else(|| Reply::error(500));
                let response = format!("HTTP/1.1 {} Test\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", reply.status, reply.mime, reply.body.len(), reply.body);
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        Self {
            base,
            requests,
            task,
        }
    }

    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    pub fn searx_url(&self, path: &str) -> String {
        self.base
            .join(path)
            .unwrap()
            .as_str()
            .replacen("http:", "searx+http:", 1)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
