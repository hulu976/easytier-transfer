use anyhow::Result;
use prost::Message;

/// 重新导出 prost-build 生成的消息类型（位于 `OUT_DIR/easyshare.rs`）。
pub mod easyshare {
    include!(concat!(env!("OUT_DIR"), "/easyshare.rs"));
}

use easyshare::{Envelope, MessageType};

/// 消息帧格式：`[4 字节大端长度][Protobuf payload]`
///
/// 长度前缀用于解决 TCP 粘包 / 拆包问题：读取端先读 4 字节得到后续
/// payload 的字节数，再精确读取对应长度，得到一个完整的 Protobuf 消息。
pub fn encode_envelope(env: &Envelope) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(env.encoded_len() + 4);
    env.encode(&mut buf)?;
    let len = buf.len() as u32;
    let mut frame = len.to_be_bytes().to_vec();
    frame.extend(buf);
    Ok(frame)
}

/// 构造一个带长度前缀的完整消息帧。
///
/// `msg_type` 为 [`MessageType`] 对应的 `i32` 值，`payload` 为任意
/// `prost::Message`（如 `ClipboardSync`、`FileChunk` 等）。
pub fn make_envelope(msg_type: i32, payload: &impl Message) -> Result<Vec<u8>> {
    let mut payload_buf = Vec::new();
    payload.encode(&mut payload_buf)?;
    let env = Envelope {
        version: 1,
        r#type: msg_type,
        payload: payload_buf,
    };
    encode_envelope(&env)
}

/// 解码一个 Protobuf payload 为 [`Envelope`]。调用方需先按长度前缀剥离出 payload。
pub fn decode_envelope(data: &[u8]) -> Result<Envelope> {
    Envelope::decode(data).map_err(Into::into)
}

/// 便捷：将 `i32` 消息类型转换为 [`MessageType`] 枚举（未知值回退为 `Heartbeat`）。
pub fn message_type_from_i32(v: i32) -> MessageType {
    MessageType::try_from(v).unwrap_or(MessageType::Heartbeat)
}

/// 剪切板内容类型（`ClipboardSync.clip_type` 取值）。
pub const CLIP_TYPE_TEXT: i32 = 0;
/// 图片类型：载荷为 PNG 编码字节（RGBA 经 PNG 压缩）。
pub const CLIP_TYPE_IMAGE: i32 = 1;
