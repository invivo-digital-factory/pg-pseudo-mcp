use std::future::Future;

use tokio::io::{AsyncRead, AsyncWrite};

use super::target_policy::AllowedTarget;
use crate::contexts::connections::domain::errors::SessionError;
use crate::contexts::connections::infrastructure::data_sources::ssh::{SshLink, TunnelStream};

pub trait ChannelOpener: Send + Sync + 'static {
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    fn open_channel(
        &self,
        target: &AllowedTarget,
    ) -> impl Future<Output = Result<Self::Stream, SessionError>> + Send;
}

impl ChannelOpener for SshLink {
    type Stream = TunnelStream;

    fn open_channel(
        &self,
        target: &AllowedTarget,
    ) -> impl Future<Output = Result<Self::Stream, SessionError>> + Send {
        self.forward_to(target.host(), target.port())
    }
}
