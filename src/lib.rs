#![deny(
    missing_docs,
    missing_debug_implementations,
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

Using this plugin is meant to be straightforward and highly configurable. You have one server and multiple clients.
You simply add either the `ClientPlugin` or the `ServerPlugin` to the respective bevy app, the runtime you wish to use,
and the netowrking provider you wish to use.  Then,
register which kind of messages can be received through `listen_for_client_message` or `listen_for_server_message`
(provided respectively by `AppNetworkClientMessage` and `AppNetworkServerMessage`), as well as which provider you want
to handle these messages and you
can start receiving packets as events of `NetworkData<T>`.

## Example Client
```rust,no_run
use bevy::prelude::*;
use bevy_eventwork::{ClientPlugin, NetworkData, NetworkMessage, ServerMessage, ClientNetworkEvent, AppNetworkServerMessage};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct WorldUpdate;

#[typetag::serde]
impl NetworkMessage for WorldUpdate {}

impl ServerMessage for WorldUpdate {
    const NAME: &'static str = "example:WorldUpdate";
}

fn main() {
     let mut app = App::build();
     app.add_plugin(ClientPlugin);
     // We are receiving this from the server, so we need to listen for it
     app.listen_for_server_message::<WorldUpdate>();
     app.add_system(handle_world_updates.system());
     app.add_system(handle_connection_events.system());
}

fn handle_world_updates(
    mut chunk_updates: EventReader<NetworkData<WorldUpdate>>,
) {
    for chunk in chunk_updates.iter() {
        info!("Got chunk update!");
    }
}

fn handle_connection_events(mut network_events: EventReader<ClientNetworkEvent>,) {
    for event in network_events.iter() {
        match event {
            &ClientNetworkEvent::Connected => info!("Connected to server!"),
            _ => (),
        }
    }
}

```

## Example Server
```rust,no_run
use bevy::prelude::*;
use bevy_eventwork::{ServerPlugin, NetworkData, NetworkMessage, NetworkServer, ServerMessage, ClientMessage, ServerNetworkEvent, AppNetworkClientMessage};

use serde::{Serialize, Deserialize};
#[derive(Serialize, Deserialize)]
struct UserInput;

#[typetag::serde]
impl NetworkMessage for UserInput {}

impl ClientMessage for UserInput {
    const NAME: &'static str = "example:UserInput";
}

fn main() {
     let mut app = App::build();
     app.add_plugin(ServerPlugin);
     // We are receiving this from a client, so we need to listen for it!
     app.listen_for_client_message::<UserInput>();
     app.add_system(handle_world_updates.system());
     app.add_system(handle_connection_events.system());
}

fn handle_world_updates(
    net: Res<NetworkServer>,
    mut chunk_updates: EventReader<NetworkData<UserInput>>,
) {
    for chunk in chunk_updates.iter() {
        info!("Got chunk update!");
    }
}

#[derive(Serialize, Deserialize)]
struct PlayerUpdate;

#[typetag::serde]
impl NetworkMessage for PlayerUpdate {}

impl ClientMessage for PlayerUpdate {
    const NAME: &'static str = "example:PlayerUpdate";
}

impl PlayerUpdate {
    fn new() -> PlayerUpdate {
        Self
    }
}

fn handle_connection_events(
    net: Res<NetworkServer>,
    mut network_events: EventReader<ServerNetworkEvent>,
) {
    for event in network_events.iter() {
        match event {
            &ServerNetworkEvent::Connected(conn_id) => {
                net.send_message(conn_id, PlayerUpdate::new());
                info!("New client connected: {:?}", conn_id);
            }
            _ => (),
        }
    }
}

```
As you can see, they are both quite similar, and provide everything a basic networked game needs.

Currently, Bevy's [TaskPool] is the default runtime used by Eventwork.
*/

/// Contains error enum.
pub mod error;
mod network_message;

/// Contains all functionality for starting a server or client, sending, and recieving messages from clients.
pub mod managers;
pub use managers::{Network, network::AppNetworkMessage};

mod runtime;
use managers::NetworkProvider;
use runtime::JoinHandle;
pub use runtime::Runtime;

use std::{fmt::{Debug, Display}, marker::PhantomData};

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
///
/// Use [`ConnectionId::is_server`] whether it is a connection to a server
/// or another. In most client/server applications this is not required as there
/// is no ambiguity.
pub struct ConnectionId {
    /// The key of the connection.
    pub id: u32
}

impl Display for ConnectionId{
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

#[derive(Debug)]
/// A network event originating from a [`NetworkClient`]
pub enum NetworkEvent {
    /// A new client has connected
    Connected(ConnectionId),
    /// A client has disconnected
    Disconnected(ConnectionId),
    /// An error occured while trying to do a network operation
    Error(NetworkError),
}

#[derive(Debug)]
/// [`NetworkData`] is what is sent over the bevy event system
///
/// Please check the root documentation how to up everything
pub struct NetworkData<T> {
    source: ConnectionId,
    inner: T,
}

impl<T> Deref for NetworkData<T>{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> NetworkData<T> {
    pub(crate) fn new(source: ConnectionId, inner: T) -> Self {
        Self { source, inner }
    }

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
    }
}
#[derive(Default, Copy, Clone, Debug)]
/// The plugin to add to your bevy [`App`](bevy::prelude::App) when you want
/// to instantiate a server
pub struct EventworkPlugin<NP: NetworkProvider, RT: Runtime = bevy::tasks::TaskPool>(
    PhantomData<(NP, RT)>,
);

impl<NP: NetworkProvider + Default, RT: Runtime> Plugin for EventworkPlugin<NP, RT> {
    fn build(&self, app: &mut App) {
        app.insert_resource(managers::Network::new(NP::default()));
        app.add_event::<NetworkEvent>();
        app.add_system_to_stage(
            CoreStage::PreUpdate,
            managers::network::handle_new_incoming_connections::<NP, RT>,
        );
    }
}