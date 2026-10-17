use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::RngCore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct AudioPiece {
    pub offset: u64,
    pub eof: bool,
    pub data: Vec<u8>,
    pub error: Option<String>,
    /// Sent once before the first bytes so the proxy can answer with the real length.
    pub total: Option<u64>,
}

impl AudioPiece {
    pub fn chunk(offset: u64, eof: bool, data: Vec<u8>) -> Self {
        Self { offset, eof, data, error: None, total: None }
    }

    pub fn failed(reason: &str) -> Self {
        Self { offset: 0, eof: true, data: Vec::new(), error: Some(reason.to_string()), total: None }
    }

    pub fn head(total: u64) -> Self {
        Self { offset: 0, eof: false, data: Vec::new(), error: None, total: Some(total) }
    }
}

pub enum Pull {
    Open {
        id: u64,
        device_id: String,
        content_id: String,
        start: u64,
        end: Option<u64>,
        reply: mpsc::UnboundedSender<AudioPiece>,
    },
    Cancel {
        id: u64,
        device_id: String,
    },
}

#[derive(Clone)]
struct Route {
    content_id: String,
    device_id: String,
}

pub struct AudioBus {
    waiters: Mutex<HashMap<u64, mpsc::UnboundedSender<AudioPiece>>>,
    routes: Mutex<HashMap<String, Route>>,
    pub base: Mutex<String>,
}

impl AudioBus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            waiters: Mutex::new(HashMap::new()),
            routes: Mutex::new(HashMap::new()),
            base: Mutex::new(String::new()),
        })
    }

    pub fn open_route(&self, content_id: &str, device_id: &str) -> String {
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        let token = bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        self.routes.lock().expect("routes").insert(
            token.clone(),
            Route {
                content_id: content_id.to_string(),
                device_id: device_id.to_string(),
            },
        );
        format!("{}/{token}", self.base.lock().expect("base"))
    }

    /// Drops every playback token and fails transfers in flight.
    pub fn close_routes(&self) {
        self.routes.lock().expect("routes").clear();
        let waiters = std::mem::take(&mut *self.waiters.lock().expect("waiters"));
        for (_, sender) in waiters {
            let _ = sender.send(AudioPiece::failed("cancelled"));
        }
    }

    pub fn register(&self, id: u64, sender: mpsc::UnboundedSender<AudioPiece>) {
        self.waiters.lock().expect("waiters").insert(id, sender);
    }

    pub fn deliver(&self, id: u64, piece: AudioPiece) {
        let sender = self.waiters.lock().expect("waiters").get(&id).cloned();
        if let Some(sender) = sender {
            let last = piece.eof || piece.error.is_some();
            if sender.send(piece).is_err() || last {
                self.waiters.lock().expect("waiters").remove(&id);
            }
        }
    }

    pub fn finish(&self, id: u64) {
        self.waiters.lock().expect("waiters").remove(&id);
    }
}

pub async fn serve(bus: Arc<AudioBus>, pulls: mpsc::UnboundedSender<Pull>) {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0").await else { return };
    if let Ok(addr) = listener.local_addr() {
        *bus.base.lock().expect("base") = format!("http://127.0.0.1:{}", addr.port());
    }
    loop {
        let Ok((socket, _)) = listener.accept().await else { continue };
        let bus = bus.clone();
        let pulls = pulls.clone();
        tokio::task::spawn_local(async move {
            let _ = handle(socket, bus, pulls).await;
        });
    }
}

async fn handle(mut socket: TcpStream, bus: Arc<AudioBus>, pulls: mpsc::UnboundedSender<Pull>) -> std::io::Result<()> {
    let header = read_header(&mut socket).await?;
    let Some(request) = parse_request(&header) else {
        socket.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n").await?;
        return Ok(());
    };
    let route = bus.routes.lock().expect("routes").get(&request.token).cloned();
    let Some(route) = route else {
        socket.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n").await?;
        return Ok(());
    };
    let (start, end) = request.range.unwrap_or((0, None));
    let id = rand::random::<u64>();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let device_id = route.device_id.clone();
    let _ = pulls.send(Pull::Open {
        id,
        device_id: route.device_id,
        content_id: route.content_id,
        start,
        end,
        reply: tx,
    });
    let cancel = |pulls: &mpsc::UnboundedSender<Pull>| {
        let _ = pulls.send(Pull::Cancel { id, device_id: device_id.clone() });
    };

    let first = tokio::time::timeout(Duration::from_secs(15), rx.recv()).await;
    let total = match first {
        Ok(Some(piece)) if piece.error.is_none() && piece.total.is_some() => piece.total.unwrap_or(0),
        Ok(Some(piece)) if piece.error.as_deref() == Some("range") => {
            bus.finish(id);
            socket.write_all(b"HTTP/1.1 416 Range Not Satisfiable\r\nConnection: close\r\n\r\n").await?;
            return Ok(());
        }
        _ => {
            bus.finish(id);
            cancel(&pulls);
            socket.write_all(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\n\r\n").await?;
            return Ok(());
        }
    };
    let stop = end.unwrap_or(total).min(total);
    let length = stop.saturating_sub(start);
    let head = if request.range.is_some() {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Type: application/octet-stream\r\nContent-Length: {length}\r\nContent-Range: bytes {start}-{}/{total}\r\nAccept-Ranges: bytes\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            stop.saturating_sub(1)
        )
    } else {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {length}\r\nAccept-Ranges: bytes\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
        )
    };
    if socket.write_all(head.as_bytes()).await.is_err() || request.head_only {
        bus.finish(id);
        cancel(&pulls);
        return Ok(());
    }

    let mut written = start;
    loop {
        let piece = tokio::time::timeout(Duration::from_secs(30), rx.recv()).await;
        let Ok(Some(piece)) = piece else { break };
        if piece.error.is_some() {
            break;
        }
        if piece.offset > written {
            break;
        }
        let skip = (written - piece.offset) as usize;
        if skip < piece.data.len() {
            let room = stop.saturating_sub(written) as usize;
            let slice = &piece.data[skip..piece.data.len().min(skip + room)];
            if socket.write_all(slice).await.is_err() {
                break;
            }
            written += slice.len() as u64;
        }
        if piece.eof || written >= stop {
            break;
        }
    }
    bus.finish(id);
    if written < stop {
        cancel(&pulls);
    }
    let _ = socket.shutdown().await;
    Ok(())
}

async fn read_header(socket: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    while !buf.windows(4).any(|window| window == b"\r\n\r\n") && buf.len() < 8192 {
        let n = socket.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    Ok(buf)
}

struct Request {
    token: String,
    range: Option<(u64, Option<u64>)>,
    head_only: bool,
}

fn parse_request(bytes: &[u8]) -> Option<Request> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split("\r\n");
    let mut request = lines.next()?.split_whitespace();
    let method = request.next()?;
    if method != "GET" && method != "HEAD" {
        return None;
    }
    let token = request.next()?.trim_start_matches('/').split('?').next()?.to_string();
    let mut range = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else { continue };
        if name.eq_ignore_ascii_case("range") {
            range = parse_range(value.trim());
        }
    }
    Some(Request { token, range, head_only: method == "HEAD" })
}

fn parse_range(value: &str) -> Option<(u64, Option<u64>)> {
    let value = value.strip_prefix("bytes=")?;
    let (start, end) = value.split_once('-')?;
    let start = start.parse().ok()?;
    let end = if end.is_empty() { None } else { Some(end.parse::<u64>().ok()?.saturating_add(1)) };
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_and_closed_ranges() {
        assert_eq!(parse_range("bytes=0-"), Some((0, None)));
        assert_eq!(parse_range("bytes=10-19"), Some((10, Some(20))));
        assert_eq!(parse_range("items=1-2"), None);
    }

    #[test]
    fn parses_request_token_and_method() {
        let request = parse_request(b"HEAD /abc?x=1 HTTP/1.1\r\nRange: bytes=5-\r\n\r\n").expect("request");
        assert_eq!(request.token, "abc");
        assert_eq!(request.range, Some((5, None)));
        assert!(request.head_only);
        assert!(parse_request(b"POST /abc HTTP/1.1\r\n\r\n").is_none());
    }
}
