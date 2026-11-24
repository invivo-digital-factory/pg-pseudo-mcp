use std::io;

use fast_socks5::consts::{
    SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE, SOCKS5_AUTH_METHOD_PASSWORD, SOCKS5_VERSION,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::credentials::ProxyCredentials;

const SUBNEGOTIATION_VERSION: u8 = 0x01;
const STATUS_SUCCEEDED: u8 = 0x00;
const STATUS_FAILED: u8 = 0x01;

pub(super) async fn authenticate<S>(
    stream: &mut S,
    credentials: &ProxyCredentials,
) -> io::Result<bool>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let [version, method_count] = read_array(stream).await?;
    if version != SOCKS5_VERSION {
        return Ok(false);
    }
    let methods = read_field(stream, method_count).await?;
    if !methods.contains(&SOCKS5_AUTH_METHOD_PASSWORD) {
        stream
            .write_all(&[SOCKS5_VERSION, SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE])
            .await?;
        return Ok(false);
    }
    stream
        .write_all(&[SOCKS5_VERSION, SOCKS5_AUTH_METHOD_PASSWORD])
        .await?;
    let [subnegotiation, username_len] = read_array(stream).await?;
    let username = read_field(stream, username_len).await?;
    let [password_len] = read_array(stream).await?;
    let password = read_field(stream, password_len).await?;
    let accepted =
        subnegotiation == SUBNEGOTIATION_VERSION && credentials.matches(&username, &password);
    let status = if accepted {
        STATUS_SUCCEEDED
    } else {
        STATUS_FAILED
    };
    stream.write_all(&[SUBNEGOTIATION_VERSION, status]).await?;
    stream.flush().await?;
    Ok(accepted)
}

async fn read_array<S, const N: usize>(stream: &mut S) -> io::Result<[u8; N]>
where
    S: AsyncRead + Unpin,
{
    let mut buffer = [0u8; N];
    stream.read_exact(&mut buffer).await?;
    Ok(buffer)
}

async fn read_field<S>(stream: &mut S, length: u8) -> io::Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut buffer = vec![0u8; usize::from(length)];
    stream.read_exact(&mut buffer).await?;
    Ok(buffer)
}
