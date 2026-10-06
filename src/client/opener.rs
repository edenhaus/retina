// Copyright (C) The Retina Authors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Caller-supplied RTSP connections; see [`TcpOpener`].

use futures::future::BoxFuture;
use url::Url;

use crate::ConnectionContext;

/// Opens the connections an RTSP session runs over, in place of Retina's
/// built-in TCP connect.
///
/// Set one via [`super::SessionOptions::tcp_opener`]. This lets the caller
/// decide how a connection is made: for example, to speak TLS for `rtsps`
/// URLs (by wrapping the stream with e.g. `tokio-rustls`), to connect to an
/// address it resolved itself, to apply its own socket options or connect
/// timeout, or to tunnel through a proxy or VPN.
///
/// Retina may call [`TcpOpener::open`] more than once per session: first to
/// send `DESCRIBE`, and later to retry `TEARDOWN` on a fresh connection after
/// the session is dropped (see [`super::TeardownPolicy`]). The URL passed is
/// the one the request is for; it is the URL given to
/// [`super::Session::describe`] or one derived from the server's responses
/// (such as its `Content-Base`), so an opener that dials a fixed address
/// should not trust the URL's host blindly. The URL is sent as the
/// `Request-URI` as is.
///
/// Only the RTSP connection itself is opened this way. A session using a
/// custom opener supports only [`super::Transport::Tcp`] (RTP interleaved on
/// the RTSP connection); [`super::Session::setup`] with
/// [`super::Transport::Udp`] fails. UDP support may be added later via
/// another (defaulted) method.
///
/// ## Example
///
/// An opener which connects to an address the caller resolved, ignoring the
/// URL's host. A TLS opener would additionally wrap `stream` before boxing it.
///
/// ```no_run
/// use futures::future::BoxFuture;
/// use retina::ConnectionContext;
/// use retina::client::{Session, SessionOptions, TcpConnection, TcpOpener};
/// use std::{io, net::SocketAddr, pin::Pin, sync::Arc, task::{Context, Poll}};
/// use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
/// use url::Url;
///
/// struct Conn {
///     stream: tokio::net::TcpStream,
///     ctx: ConnectionContext,
/// }
///
/// impl AsyncRead for Conn {
///     fn poll_read(
///         mut self: Pin<&mut Self>,
///         cx: &mut Context<'_>,
///         buf: &mut ReadBuf<'_>,
///     ) -> Poll<io::Result<()>> {
///         Pin::new(&mut self.stream).poll_read(cx, buf)
///     }
/// }
///
/// impl AsyncWrite for Conn {
///     fn poll_write(
///         mut self: Pin<&mut Self>,
///         cx: &mut Context<'_>,
///         buf: &[u8],
///     ) -> Poll<io::Result<usize>> {
///         Pin::new(&mut self.stream).poll_write(cx, buf)
///     }
///
///     fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
///         Pin::new(&mut self.stream).poll_flush(cx)
///     }
///
///     fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
///         Pin::new(&mut self.stream).poll_shutdown(cx)
///     }
/// }
///
/// impl TcpConnection for Conn {
///     fn ctx(&self) -> ConnectionContext {
///         self.ctx
///     }
/// }
///
/// struct FixedAddrOpener(SocketAddr);
///
/// impl TcpOpener for FixedAddrOpener {
///     fn open<'a>(&'a self, _url: &'a Url) -> BoxFuture<'a, io::Result<Box<dyn TcpConnection>>> {
///         Box::pin(async move {
///             let stream = tokio::net::TcpStream::connect(self.0).await?;
///             let ctx = ConnectionContext::new(stream.local_addr()?, stream.peer_addr()?);
///             Ok(Box::new(Conn { stream, ctx }) as Box<dyn TcpConnection>)
///         })
///     }
/// }
///
/// # async fn f() -> Result<(), Box<dyn std::error::Error>> {
/// let addr: SocketAddr = "192.0.2.1:554".parse()?;
/// let options = SessionOptions::default().tcp_opener(Arc::new(FixedAddrOpener(addr)));
/// let url = Url::parse("rtsp://camera.example/stream")?;
/// let session = Session::describe(url, options).await?;
/// # Ok(())
/// # }
/// ```
pub trait TcpOpener: Send + Sync + 'static {
    /// Opens a connection for requests to `url`.
    ///
    /// An error is returned from the operation that needed the connection
    /// (e.g. [`super::Session::describe`]) as a connect error. Any connect
    /// timeout is the opener's business; Retina bounds `TEARDOWN` retries
    /// with its own timeouts.
    fn open<'a>(&'a self, url: &'a Url) -> BoxFuture<'a, std::io::Result<Box<dyn TcpConnection>>>;
}

/// A connection opened by a [`TcpOpener`].
///
/// This is a byte stream carrying RTSP messages and interleaved RTP/RTCP data,
/// such as a `tokio::net::TcpStream` or a TLS stream on top of one.
pub trait TcpConnection:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync
{
    /// Returns the context identifying this connection in error messages.
    ///
    /// Retina calls this once, right after [`TcpOpener::open`] returns. Build
    /// it with [`ConnectionContext::new`].
    fn ctx(&self) -> ConnectionContext;
}
