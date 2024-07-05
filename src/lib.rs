#![deny(
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
    clippy::unwrap_used
)]
#![allow(clippy::type_complexity)]

/*!
A simple networking plugin for Bevy designed to work with Bevy's event architecture.

Using this plugin is meant to be straightforward and highly configurable.
You simply add the `EventworkPlugin` to the respective bevy app with the runtime you wish to use
and the networking provider you wish to use. Next add your runtime to the app as the [`EventworkRuntime`] Resource.
Then, register which kind of messages can be received through [`managers::network::AppNetworkMessage::listen_for_message`],
as well as which provider you wantto handle these messages and you
can start receiving packets as events of [`NetworkData<T>`].

This plugin also supports Request/Response style messages, see that modules documentation for further info: **[Request Documentation](https://docs.rs/bevy_eventwork/latest/bevy_eventwork/managers/network_request/index.html)**

## Example Client
```rust,no_run
use bevy::prelude::*;
use bevy_eventwork::{EventworkRuntime, EventworkPlugin, NetworkData, NetworkMessage, NetworkEvent, AppNetworkMessage, tcp::TcpProvider,tcp::NetworkSettings};
use serde::{Serialize, Deserialize};
use bevy::tasks::TaskPoolBuilder;

#[derive(Serialize, Deserialize)]
struct WorldUpdate;

impl NetworkMessage for WorldUpdate {
    const NAME: &'static str = "example:WorldUpdate";
}

fn main() {
     let mut app = App::new();
     app.add_plugins(EventworkPlugin::<
        TcpProvider,
        bevy::tasks::TaskPool,
    >::default());

    //Insert our runtime and the neccessary settings for the TCP transport
    app.insert_resource(EventworkRuntime(
        TaskPoolBuilder::new().num_threads(2).build(),
    ));
    app.insert_resource(NetworkSettings::default());

    // We are receiving this from the server, so we need to listen for it
     app.listen_for_message::<WorldUpdate, TcpProvider>();
     app.add_systems(Update, (handle_world_updates,handle_connection_events));
}

fn handle_world_updates(
    mut chunk_updates: EventReader<NetworkData<WorldUpdate>>,
) {
    for chunk in chunk_updates.read() {
        info!("Got chunk update!");
    }
}

fn handle_connection_events(mut network_events: EventReader<NetworkEvent>,) {
    for event in network_events.read() {
        match event {
            &NetworkEvent::Connected(_) => info!("Connected to server!"),
            _ => (),
        }
    }
}

```

## Example Server
```rust,no_run
use bevy::prelude::*;
use bevy_eventwork::{EventworkRuntime,
    EventworkPlugin, NetworkData, NetworkMessage, Network, NetworkEvent, AppNetworkMessage,
    tcp::TcpProvider,tcp::NetworkSettings
};
use bevy::tasks::TaskPoolBuilder;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct UserInput;

impl NetworkMessage for UserInput {
    const NAME: &'static str = "example:UserInput";
}

fn main() {
     let mut app = App::new();
     app.add_plugins(EventworkPlugin::<
        TcpProvider,
        bevy::tasks::TaskPool,
    >::default());


    //Insert our runtime and the neccessary settings for the TCP transport
    app.insert_resource(EventworkRuntime(
        TaskPoolBuilder::new().num_threads(2).build(),
    ));
    app.insert_resource(NetworkSettings::default());

     // We are receiving this from a client, so we need to listen for it!
     app.listen_for_message::<UserInput, TcpProvider>();
     app.add_systems(Update, (handle_world_updates,handle_connection_events));
}

fn handle_world_updates(
    mut chunk_updates: EventReader<NetworkData<UserInput>>,
) {
    for chunk in chunk_updates.read() {
        info!("Got chunk update!");
    }
}

#[derive(Serialize, Deserialize)]
struct PlayerUpdate;

impl NetworkMessage for PlayerUpdate {
    const NAME: &'static str = "example:PlayerUpdate";
}

fn handle_connection_events(
    net: Res<Network<TcpProvider>>,
    mut network_events: EventReader<NetworkEvent>,
) {
    for event in network_events.read() {
        match event {
            &NetworkEvent::Connected(conn_id) => {
                net.send_message(conn_id, PlayerUpdate);
                info!("New client connected: {:?}", conn_id);
            }
            _ => (),
        }
    }
}

```
As you can see, they are both quite similar, and provide everything a basic networked game needs.

Currently, Bevy's [TaskPool](bevy::tasks::TaskPool) is the default runtime used by Eventwork.
*/

/// Contains error enum.
pub mod error;
mod network_message;

/// Contains all functionality for starting a server or client, sending, and recieving messages from clients.
pub mod managers;
pub use managers::{network::AppNetworkMessage, Network};

mod runtime;
use managers::NetworkProvider;
pub use runtime::EventworkRuntime;
use runtime::JoinHandle;
pub use runtime::Runtime;

use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
};

pub use async_channel;
use async_channel::{unbounded, Receiver, Sender};
pub use async_trait::async_trait;
use bevy::prelude::*;
use error::NetworkError;
pub use network_message::NetworkMessage;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[cfg(feature = "tcp")]
/// A default tcp provider to help get you started.
pub mod tcp;

struct AsyncChannel<T> {
    pub(crate) sender: Sender<T>,
    pub(crate) receiver: Receiver<T>,
}

impl<T> AsyncChannel<T> {
    fn new() -> Self {
        let (sender, receiver) = unbounded();

        Self { sender, receiver }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
/// A [`ConnectionId`] denotes a single connection
pub struct ConnectionId {
    /// The key of the connection.
    pub id: u32,
}

impl Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Connection with ID={0}", self.id))
    }
}

#[derive(Serialize, Deserialize)]
/// [`NetworkPacket`]s are untyped packets to be sent over the wire
pub struct NetworkPacket {
    kind: String,
    data: Vec<u8>,
}

impl Debug for NetworkPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkPacket")
            .field("kind", &self.kind)
            .finish()
    }
}

#[derive(Debug, Event)]
/// A network event originating from another eventwork app
pub enum NetworkEvent {
    /// A new client has connected
    Connected(ConnectionId),
    /// A client has disconnected
    Disconnected(ConnectionId),
    /// An error occured while trying to do a network operation
    Error(NetworkError),
}

#[derive(Debug, Event)]
/// [`NetworkData`] is what is sent over the bevy event system
///
/// Please check the root documentation how to up everything
pub struct NetworkData<T> {
    source: ConnectionId,
    inner: T,
}

impl<T> Deref for NetworkData<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> NetworkData<T> {
    /// The source of this network data
    pub fn source(&self) -> &ConnectionId {
        &self.source
    }

    /// Get the inner data out of it
    pub fn into_inner(self) -> T {
        self.inner
    }
}

struct Connection {
    receive_task: Box<dyn JoinHandle>,
    map_receive_task: Box<dyn JoinHandle>,
    send_task: Box<dyn JoinHandle>,
    send_message: Sender<NetworkPacket>,
}

impl Connection {
    fn stop(mut self) {
        self.receive_task.abort();
        self.send_task.abort();
        self.map_receive_task.abort();
    }
}
#[derive(Default, Copy, Clone, Debug)]
/// The plugin to add to your bevy [`App``] when you want
/// to instantiate a server
pub struct EventworkPlugin<NP: NetworkProvider, RT: Runtime = bevy::tasks::TaskPool>(
    PhantomData<(NP, RT)>,
);

impl<NP: NetworkProvider + Default, RT: Runtime> Plugin for EventworkPlugin<NP, RT> {
    fn build(&self, app: &mut App) {
        app.insert_resource(Network::new(NP::default()));
        app.add_event::<NetworkEvent>();
        app.add_systems(
            PreUpdate,
            managers::network::handle_new_incoming_connections::<NP, RT>,
        );
    }
}
