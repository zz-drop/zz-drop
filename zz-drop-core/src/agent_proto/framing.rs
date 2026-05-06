use std::io::{self, Read, Write};

use thiserror::Error;

use super::types::{
    AgentRequest, AgentResponse, PROTOCOL_VERSION, VersionedRequest, VersionedResponse,
};

pub const FRAME_LIMIT: usize = 1 << 20;

#[derive(Debug, Error)]
pub enum FramingError {
    #[error("frame too large: {size} bytes (limit {limit})")]
    FrameTooLarge { size: usize, limit: usize },

    #[error("io error: {0}")]
    Io(String),

    #[error("encode error: {0}")]
    Encode(String),

    #[error("decode error: {0}")]
    Decode(String),

    #[error("unsupported protocol version (got {got}, expected {expected})")]
    UnsupportedVersion { got: u16, expected: u16 },

    #[error("truncated frame")]
    Truncated,
}

impl From<io::Error> for FramingError {
    fn from(value: io::Error) -> Self {
        if value.kind() == io::ErrorKind::UnexpectedEof {
            Self::Truncated
        } else {
            Self::Io(value.to_string())
        }
    }
}

fn frame_payload(payload: Vec<u8>) -> Result<Vec<u8>, FramingError> {
    if payload.len() > FRAME_LIMIT {
        return Err(FramingError::FrameTooLarge {
            size: payload.len(),
            limit: FRAME_LIMIT,
        });
    }
    let len = u32::try_from(payload.len()).map_err(|_| FramingError::FrameTooLarge {
        size: payload.len(),
        limit: FRAME_LIMIT,
    })?;
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

pub fn encode_request_body(req: &AgentRequest) -> Result<Vec<u8>, FramingError> {
    let envelope = VersionedRequest {
        version: PROTOCOL_VERSION,
        request: req.clone(),
    };
    postcard::to_allocvec(&envelope).map_err(|e| FramingError::Encode(e.to_string()))
}

pub fn encode_response_body(resp: &AgentResponse) -> Result<Vec<u8>, FramingError> {
    let envelope = VersionedResponse {
        version: PROTOCOL_VERSION,
        response: resp.clone(),
    };
    postcard::to_allocvec(&envelope).map_err(|e| FramingError::Encode(e.to_string()))
}

pub fn decode_request_body(body: &[u8]) -> Result<AgentRequest, FramingError> {
    let envelope: VersionedRequest =
        postcard::from_bytes(body).map_err(|e| FramingError::Decode(e.to_string()))?;
    if envelope.version != PROTOCOL_VERSION {
        return Err(FramingError::UnsupportedVersion {
            got: envelope.version,
            expected: PROTOCOL_VERSION,
        });
    }
    Ok(envelope.request)
}

pub fn decode_response_body(body: &[u8]) -> Result<AgentResponse, FramingError> {
    let envelope: VersionedResponse =
        postcard::from_bytes(body).map_err(|e| FramingError::Decode(e.to_string()))?;
    if envelope.version != PROTOCOL_VERSION {
        return Err(FramingError::UnsupportedVersion {
            got: envelope.version,
            expected: PROTOCOL_VERSION,
        });
    }
    Ok(envelope.response)
}

pub fn encode_request(req: &AgentRequest) -> Result<Vec<u8>, FramingError> {
    let payload = encode_request_body(req)?;
    frame_payload(payload)
}

pub fn encode_response(resp: &AgentResponse) -> Result<Vec<u8>, FramingError> {
    let payload = encode_response_body(resp)?;
    frame_payload(payload)
}

fn split_frame(frame: &[u8]) -> Result<&[u8], FramingError> {
    if frame.len() < 4 {
        return Err(FramingError::Truncated);
    }
    let len_bytes: [u8; 4] = frame[..4].try_into().expect("4 bytes available");
    let len = u32::from_be_bytes(len_bytes) as usize;
    if len > FRAME_LIMIT {
        return Err(FramingError::FrameTooLarge {
            size: len,
            limit: FRAME_LIMIT,
        });
    }
    if frame.len() < 4 + len {
        return Err(FramingError::Truncated);
    }
    Ok(&frame[4..4 + len])
}

pub fn decode_request(frame: &[u8]) -> Result<AgentRequest, FramingError> {
    let body = split_frame(frame)?;
    decode_request_body(body)
}

pub fn decode_response(frame: &[u8]) -> Result<AgentResponse, FramingError> {
    let body = split_frame(frame)?;
    decode_response_body(body)
}

pub fn write_frame<W: Write>(w: &mut W, payload: &[u8]) -> Result<(), FramingError> {
    if payload.len() > FRAME_LIMIT {
        return Err(FramingError::FrameTooLarge {
            size: payload.len(),
            limit: FRAME_LIMIT,
        });
    }
    let len = u32::try_from(payload.len()).map_err(|_| FramingError::FrameTooLarge {
        size: payload.len(),
        limit: FRAME_LIMIT,
    })?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(payload)?;
    Ok(())
}

pub fn read_frame<R: Read>(r: &mut R) -> Result<Vec<u8>, FramingError> {
    let mut len_bytes = [0u8; 4];
    r.read_exact(&mut len_bytes)?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    if len > FRAME_LIMIT {
        return Err(FramingError::FrameTooLarge {
            size: len,
            limit: FRAME_LIMIT,
        });
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}
