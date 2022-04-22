use crate::ConnectionId;

/// Internal errors used by Spicy
#[derive(Debug)]
pub enum NetworkError {
    /// Error occured when accepting a new connection.
    Accept(std::io::Error),

    /// Connection couldn't be found.
    ConnectionNotFound(ConnectionId),

    /// Failed to send across channel because it was closed.
    ChannelClosed(ConnectionId),

    /// Can't send as there is no connection.
    NotConnected,

    /// An error occured when trying to listen for connections.
    Listen(std::io::Error),

    /// An error occured when trying to connect.
    Connection(std::io::Error),
}
