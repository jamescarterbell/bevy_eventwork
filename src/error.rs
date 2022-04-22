use std::fmt::Display;

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

    /// An error occured when trying to listen for connections.
    Listen(std::io::Error),

    /// An error occured when trying to connect.
    Connection(std::io::Error),
}

impl Display for NetworkError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            Self::Accept(e) => f.write_fmt(format_args!("An error occured when accepting a new connnection: {0}", e)),
            Self::ConnectionNotFound(id) => f.write_fmt(format_args!("Could not find connection with id: {0}", id)),
            Self::ChannelClosed(id) => f.write_fmt(format_args!("Connection closed with id: {0}", id)),
            Self::Listen(e) => f.write_fmt(format_args!("An error occured when trying to start listening for new connections: {0}", e)),
            Self::Connection(e) => f.write_fmt(format_args!("An error occured when trying to connect: {0}", e)),
        }
    }
}