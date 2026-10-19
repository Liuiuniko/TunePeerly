// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
use std::io;

pub const MAX_PAYLOAD: usize = 32 * 1024;

#[derive(Debug, Clone)]
pub enum Frame {
    Keepalive,
    Bundle(Vec<u8>),
    Signal(Vec<u8>),
    AudioReq {
        id: u64,
        content_id: String,
        start: u64,
        end: Option<u64>,
    },
    AudioChunk {
        id: u64,
        offset: u64,
        eof: bool,
        data: Vec<u8>,
    },
    AudioErr { id: u64, reason: String },
    AudioCancel { id: u64 },
    AudioHead { id: u64, total: u64 },
    /// The side that was asked to pair has accepted. Empty payload.
    PairAccept,
}

impl Frame {
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let (kind, payload) = match self {
            Self::Keepalive => (1u8, Vec::new()),
            Self::Bundle(bytes) => (2, bytes.clone()),
            Self::Signal(bytes) => (3, bytes.clone()),
            Self::AudioReq { id, content_id, start, end } => {
                let mut payload = Vec::new();
                payload.extend_from_slice(&id.to_be_bytes());
                payload.extend_from_slice(&start.to_be_bytes());
                payload.extend_from_slice(&end.unwrap_or(u64::MAX).to_be_bytes());
                payload.extend_from_slice(content_id.as_bytes());
                (4, payload)
            }
            Self::AudioChunk { id, offset, eof, data } => {
                let mut payload = Vec::new();
                payload.extend_from_slice(&id.to_be_bytes());
                payload.extend_from_slice(&offset.to_be_bytes());
                payload.push(u8::from(*eof));
                payload.extend_from_slice(data);
                (5, payload)
            }
            Self::AudioErr { id, reason } => {
                let mut payload = id.to_be_bytes().to_vec();
                payload.extend_from_slice(reason.as_bytes());
                (6, payload)
            }
            Self::AudioCancel { id } => (7, id.to_be_bytes().to_vec()),
            Self::AudioHead { id, total } => {
                let mut payload = id.to_be_bytes().to_vec();
                payload.extend_from_slice(&total.to_be_bytes());
                (8, payload)
            }
            Self::PairAccept => (9, Vec::new()),
        };
        if payload.len() > MAX_PAYLOAD {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
        }
        let mut out = Vec::with_capacity(5 + payload.len());
        out.push(kind);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(&payload);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < 5 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "short frame"));
        }
        let kind = bytes[0];
        let len = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
        if len > MAX_PAYLOAD || bytes.len() != 5 + len {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad frame length"));
        }
        let payload = &bytes[5..];
        let need = match kind {
            4 => 24,
            5 => 17,
            6 | 7 => 8,
            8 => 16,
            _ => 0,
        };
        if payload.len() < need {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "short payload"));
        }
        match kind {
            1 => Ok(Self::Keepalive),
            2 => Ok(Self::Bundle(payload.to_vec())),
            3 => Ok(Self::Signal(payload.to_vec())),
            4 => {
                let id = u64::from_be_bytes(payload[0..8].try_into().unwrap());
                let start = u64::from_be_bytes(payload[8..16].try_into().unwrap());
                let end_raw = u64::from_be_bytes(payload[16..24].try_into().unwrap());
                let content_id = String::from_utf8_lossy(&payload[24..]).to_string();
                Ok(Self::AudioReq {
                    id,
                    content_id,
                    start,
                    end: if end_raw == u64::MAX { None } else { Some(end_raw) },
                })
            }
            5 => {
                let id = u64::from_be_bytes(payload[0..8].try_into().unwrap());
                let offset = u64::from_be_bytes(payload[8..16].try_into().unwrap());
                let eof = payload[16] == 1;
                Ok(Self::AudioChunk {
                    id,
                    offset,
                    eof,
                    data: payload[17..].to_vec(),
                })
            }
            6 => Ok(Self::AudioErr {
                id: u64::from_be_bytes(payload[0..8].try_into().unwrap()),
                reason: String::from_utf8_lossy(&payload[8..]).to_string(),
            }),
            7 => Ok(Self::AudioCancel {
                id: u64::from_be_bytes(payload[0..8].try_into().unwrap()),
            }),
            8 => Ok(Self::AudioHead {
                id: u64::from_be_bytes(payload[0..8].try_into().unwrap()),
                total: u64::from_be_bytes(payload[8..16].try_into().unwrap()),
            }),
            9 => Ok(Self::PairAccept),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "unknown frame")),
        }
    }
}

pub async fn write_frame(stream: &mut (impl tokio::io::AsyncWriteExt + Unpin), frame: &Frame) -> io::Result<()> {
    let bytes = frame.encode()?;
    stream.write_all(&bytes).await?;
    stream.flush().await
}

pub async fn read_frame(stream: &mut (impl tokio::io::AsyncReadExt + Unpin)) -> io::Result<Frame> {
    let mut header = [0u8; 5];
    stream.read_exact(&mut header).await?;
    let len = u32::from_be_bytes(header[1..5].try_into().unwrap()) as usize;
    if len > MAX_PAYLOAD {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await?;
    let mut all = header.to_vec();
    all.extend_from_slice(&body);
    Frame::decode(&all)
}
