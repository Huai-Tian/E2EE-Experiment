//! 线路层：一帧 = 2 字节大端长度 + 载荷。
//! 握手消息与加密后的聊天行都走这个格式。

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// 一帧最长 65535 字节（u16 上限，天然封顶）
const MAX_FRAME: usize = u16::MAX as usize;

/// 发送一帧
pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "这一句太长，说不完"));
    }
    w.write_all(&(payload.len() as u16).to_be_bytes()).await?;
    w.write_all(payload).await?;
    Ok(())
}

/// 接收一帧。
/// Ok(None) 表示对端干干净净地关了连接（EOF 恰好落在帧边界）。
pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0u8; 2];
    if read_exact_or_eof(r, &mut header).await?.is_none() {
        return Ok(None);
    }
    let mut payload = vec![0u8; u16::from_be_bytes(header) as usize];
    if read_exact_or_eof(r, &mut payload).await?.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "话说到一半就断了",
        ));
    }
    Ok(Some(payload))
}

/// 读满 buf。None 表示一开始就是 EOF（干净关闭）；
/// 读了一半断了则报错——区别于善始善终的告别。
async fn read_exact_or_eof<R: AsyncRead + Unpin>(
    r: &mut R,
    buf: &mut [u8],
) -> io::Result<Option<()>> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = r.read(&mut buf[filled..]).await?;
        if n == 0 {
            if filled == 0 {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "话说到一半就断了",
            ));
        }
        filled += n;
    }
    Ok(Some(()))
}
