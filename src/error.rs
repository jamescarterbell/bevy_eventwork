use std::fmt::Display;

use crate::ConnectionId;

/// Internal errors used by Spicy
#[derive(Debug)]
pub enum NetworkError {
    /// A default networking error returned when no other more specific type can be determined
    Error(String),

    /// Error occured when accepting a new connection.
    Accept(std::io::Error),

    /// Connection couldn't be found.
    ConnectionNotFound(ConnectionId),

    /// Failed to send across channel because it was closed.
    ChannelClosed(ConnectionId),

    /// An error occured when trying to listen for connections.
    Listen(std::io::Error),

    /// An error occured when trying to connect.
    Connection(std::io::Error),

    /// Attempted to send data over a closed internal channel.
    SendError,

    /// Serialization error
    Serialization,
}

impl Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error(string) => f.write_fmt(format_args!("Network Error: {0}", string)),
            Self::Accept(e) => f.write_fmt(format_args!(
                "An error occured when accepting a new connnection: {0}",
                e
            )),
            Self::ConnectionNotFound(id) => {
                f.write_fmt(format_args!("Could not find connection with id: {0}", id))
            }
            Self::ChannelClosed(id) => {
                f.write_fmt(format_args!("Connection closed with id: {0}", id))
            }
            Self::Listen(e) => f.write_fmt(format_args!(
                "An error occured when trying to start listening for new connections: {0}",
                e
            )),
            Self::Connection(e) => f.write_fmt(format_args!(
                "An error occured when trying to connect: {0}",
                e
            )),
            Self::SendError => {
                f.write_fmt(format_args!("Attempted to send data over closed channel"))
            }
            Self::Serialization => f.write_fmt(format_args!("Failed to serialize")),
        }
    }
}
