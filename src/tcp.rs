use std::{net::SocketAddr, pin::Pin};

use crate::{
    async_channel::{Receiver, Sender},
    async_trait,
    error::NetworkError,
    managers::NetworkProvider,
    NetworkPacket,
};
use async_net::{TcpListener, TcpStream};
use bevy::{
    log::{debug, error, info, trace},
    prelude::Resource,
};
use futures_lite::{AsyncReadExt, AsyncWriteExt, FutureExt, Stream};
use std::future::Future;

#[derive(Default, Debug)]
/// Provides a tcp stream and listener for eventwork.
pub struct TcpProvider;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl NetworkProvider for TcpProvider {
    type NetworkSettings = NetworkSettings;

    type Socket = TcpStream;

    type ReadHalf = TcpStream;

    type WriteHalf = TcpStream;

    type ConnectInfo = SocketAddr;

    type AcceptInfo = SocketAddr;

    type AcceptStream = OwnedIncoming;

    async fn accept_loop(
        accept_info: Self::AcceptInfo,
        _: Self::NetworkSettings,
    ) -> Result<Self::AcceptStream, NetworkError> {
        let listener = TcpListener::bind(accept_info)
            .await
            .map_err(NetworkError::Listen)?;

        Ok(OwnedIncoming::new(listener))
    }

    async fn connect_task(
        connect_info: Self::ConnectInfo,
        _: Self::NetworkSettings,
    ) -> Result<Self::Socket, NetworkError> {
        info!("Beginning connection");
        let stream = TcpStream::connect(connect_info)
            .await
            .map_err(NetworkError::Connection)?;

        info!("Connected!");

        let addr = stream
            .peer_addr()
            .expect("Could not fetch peer_addr of existing stream");

        debug!("Connected to: {:?}", addr);
        return Ok(stream);
    }

    async fn recv_loop(
        mut read_half: Self::ReadHalf,
        messages: Sender<NetworkPacket>,
        settings: Self::NetworkSettings,
    ) {
        let mut buffer = vec![0; settings.max_packet_length];
        loop {
            info!("Reading message length");
            let length = match read_half.read(&mut buffer[..8]).await {
                Ok(0) => {
                    // EOF, meaning the TCP stream has closed.
                    info!("Client disconnected");
                    // TODO: probably want to do more than just quit the receive task.
                    //       to let eventwork know that the peer disconnected.
                    break;
                }
                Ok(8) => {
                    let bytes = &buffer[..8];
                    u64::from_le_bytes(
                        bytes
                            .try_into()
                            .expect("Couldn't read bytes from connection!"),
                    ) as usize
                }
                Ok(n) => {
                    error!(
                        "Could not read enough bytes for header. Expected 8, got {}",
                        n
                    );
                    break;
                }
                Err(err) => {
                    error!("Encountered error while fetching length: {}", err);
                    break;
                }
            };
            info!("Message length: {}", length);

            if length > settings.max_packet_length {
                error!(
                    "Received too large packet: {} > {}",
                    length, settings.max_packet_length
                );
                break;
            }

            info!("Reading message into buffer");
            match read_half.read_exact(&mut buffer[..length]).await {
                Ok(()) => (),
                Err(err) => {
                    error!(
                        "Encountered error while fetching stream of length {}: {}",
                        length, err
                    );
                    break;
                }
            }
            info!("Message read");

            let packet: NetworkPacket = match bincode::deserialize(&buffer[..length]) {
                Ok(packet) => packet,
                Err(err) => {
                    error!("Failed to decode network packet from: {}", err);
                    break;
                }
            };

            if messages.send(packet).await.is_err() {
                error!("Failed to send decoded message to eventwork");
                break;
            }
            info!("Message deserialized and sent to eventwork");
        }
    }

    async fn send_loop(
        mut write_half: Self::WriteHalf,
        messages: Receiver<NetworkPacket>,
        _settings: Self::NetworkSettings,
    ) {
        while let Ok(message) = messages.recv().await {
            let encoded = match bincode::serialize(&message) {
                Ok(encoded) => encoded,
                Err(err) => {
                    error!("Could not encode packet {:?}: {}", message, err);
                    continue;
                }
            };

            let len = encoded.len() as u64;
            debug!("Sending a new message of size: {}", len);

            match write_half.write(&len.to_le_bytes()).await {
                Ok(_) => (),
                Err(err) => {
                    error!("Could not send packet length: {:?}: {}", len, err);
                    break;
                }
            }

            trace!("Sending the content of the message!");

            match write_half.write_all(&encoded).await {
                Ok(_) => (),
                Err(err) => {
                    error!("Could not send packet: {:?}: {}", message, err);
                    break;
                }
            }

            trace!("Succesfully written all!");
        }
    }

    fn split(combined: Self::Socket) -> (Self::ReadHalf, Self::WriteHalf) {
        (combined.clone(), combined)
    }
}

#[derive(Clone, Debug, Resource)]
#[allow(missing_copy_implementations)]
/// Settings to configure the network, both client and server
pub struct NetworkSettings {
    /// Maximum packet size in bytes. If a client ever exceeds this size, they will be disconnected
    ///
    /// ## Default
    /// The default is set to 10MiB
    pub max_packet_length: usize,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            max_packet_length: 10 * 1024 * 1024,
        }
    }
}

/// A special stream for recieving tcp connections
pub struct OwnedIncoming {
    inner: TcpListener,
    stream: Option<Pin<Box<dyn Future<Output = Option<TcpStream>>>>>,
}

impl OwnedIncoming {
    fn new(listener: TcpListener) -> Self {
        Self {
            inner: listener,
            stream: None,
        }
    }
}

impl Stream for OwnedIncoming {
    type Item = TcpStream;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let incoming = self.get_mut();
        if incoming.stream.is_none() {
            let listener: *const TcpListener = &incoming.inner;
            incoming.stream = Some(Box::pin(async move {
                unsafe {
                    listener
                        .as_ref()
                        .expect("Segfault when trying to read listener in OwnedStream")
                }
                .accept()
                .await
                .map(|(s, _)| s)
                .ok()
            }));
        }
        if let Some(stream) = &mut incoming.stream {
            if let std::task::Poll::Ready(res) = stream.poll(cx) {
                incoming.stream = None;
                return std::task::Poll::Ready(res);
            }
        }
        std::task::Poll::Pending
    }
}

unsafe impl Send for OwnedIncoming {}
