use async_channel::Sender;
use bevy::{prelude::{debug, App, CoreStage, EventReader, EventWriter, Res, ResMut}, utils::HashMap};
use serde::{Serialize, Deserialize, de::DeserializeOwned};

use crate::{ServerMessage, ClientMessage, NetworkData, ConnectionId, NetworkPacket};

use super::{NetworkServerProvider, NetworkServer, NetworkClientProvider, NetworkClient, server::register_server_message, client::register_client_message};

/// Marks a type as a request type.
pub trait ServerRequestMessage: Clone + Serialize + DeserializeOwned + Send + Sync + 'static{
    /// The response type for the request.
    type ServerResponseMessage: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    /// The label used for the request type, same rules as [`ServerMessage`] in terms of naming.
    const REQUEST_NAME: &'static str;
    /// The label used for the request type, same rules as [`ClientMessage`] in terms of naming.
    const RESPONSE_NAME: &'static str;
}

#[derive(Serialize, Deserialize)]
struct ServerRequestInternal<T>{
    request_id: u64,
    request: T
}

impl<T: ServerRequestMessage> ServerMessage for ServerRequestInternal<T>{
    const NAME: &'static str = T::REQUEST_NAME;
}

/// A wrapper around a request that automatically handles writing
/// the response to eventwork for network transmission.
struct ServerRequest<T: ServerRequestMessage>{
    from: ConnectionId,
    request: T,
    request_id: u64,
    response_tx: Sender<NetworkPacket>,
}

impl<T: ServerRequestMessage> ServerRequest<T>{
    pub fn get_request(&self) -> &T{
        &self.request
    }

    pub fn respond(self, response: T::ServerResponseMessage){
        let packet = NetworkPacket {
            kind: String::from(T::RESPONSE_NAME),
            data: bincode::serialize(&response).unwrap(),
        };

        self.response_tx.try_send(packet);
    }
}

/// A utility trait on [`App`] to easily register [`ServerRequestMessage`]s for servers to recieve
pub trait AppNetworkServerRequestMessage {
    /// Register a server request message type to listen for on the server
    fn listen_for_server_request_message<T: ServerRequestMessage, NSP: NetworkServerProvider>(
        &mut self,
    ) -> &mut Self;
}

impl AppNetworkServerRequestMessage for App {
    fn listen_for_server_request_message<T: ServerRequestMessage, NSP: NetworkServerProvider>(
        &mut self,
    ) -> &mut Self {
        let server = self.world.get_resource::<NetworkServer<NSP>>().expect("Could not find `NetworkServer`. Be sure to include the `ServerPlugin` before listening for server messages.");

        debug!("Registered a new ServerMessage: {}", ServerRequestInternal::<T>::NAME);

        assert!(
            !server.recv_message_map.contains_key(ServerRequestInternal::<T>::NAME),
            "Duplicate registration of ServerMessage: {}",
            ServerRequestInternal::<T>::NAME
        );
        server.recv_message_map.insert(ServerRequestInternal::<T>::NAME, Vec::new());
        self.add_event::<NetworkData<ServerRequestInternal<T>>>();
        self.add_event::<ServerRequest<T>>();
        self.add_system_to_stage(CoreStage::PreUpdate, register_server_message::<ServerRequestInternal::<T>, NSP>);
        self.add_system_to_stage(CoreStage::PreUpdate, create_server_request_handlers::<T, NSP>)
    }
}

fn create_server_request_handlers<T: ServerRequestMessage, NSP: NetworkServerProvider>(
    mut requests: EventReader<NetworkData<ServerRequestInternal<T>>>,
    mut requests_wrapped: EventWriter<ServerRequest<T>>,
    network: Res<NetworkServer<NSP>>,
){
    for request in requests.iter(){
        if let Some(connection) = network.established_connections.get(&request.source){
            requests_wrapped.send(
                ServerRequest { 
                    from: request.source,
                    request: request.request.clone(),
                    request_id: request.request_id,
                    response_tx: connection.send_message.clone()
                }
            );
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ServerResponseInternal<T>{
    response_id: u64,
    response: T
}

impl<T: ServerRequestMessage> ClientMessage for ServerResponseInternal<T>{
    const NAME: &'static str = T::RESPONSE_NAME;
}

/// A utility trait on [`App`] to easily register [`ServerRequestMessage::ServerResponseMessage`]s for clients to recieve
pub trait AppNetworkServerResponseMessage {
    /// Register a server request message type to listen for on the server
    fn listen_for_server_response_message<T: ServerRequestMessage, NSP: NetworkClientProvider>(
        &mut self,
    ) -> &mut Self;
}

impl AppNetworkServerResponseMessage for App {
    fn listen_for_server_response_message<T: ServerRequestMessage, NSP: NetworkClientProvider>(
        &mut self,
    ) -> &mut Self {
        self.init_resource::<HashMap<u64, Sender<T::ServerResponseMessage>>>();
        let client = self.world.get_resource::<NetworkClient<NSP>>().expect("Could not find `NetworkServer`. Be sure to include the `ServerPlugin` before listening for server messages.");

        debug!("Registered a new ServerMessage: {}", ServerResponseInternal::<T>::NAME);

        assert!(
            !client.recv_message_map.contains_key(ServerResponseInternal::<T>::NAME),
            "Duplicate registration of ServerMessage: {}",
            ServerResponseInternal::<T>::NAME
        );
        client.recv_message_map.insert(ServerResponseInternal::<T>::NAME, Vec::new());
        self.add_event::<NetworkData<ServerResponseInternal<T>>>();
        self.add_system_to_stage(CoreStage::PreUpdate, register_client_message::<ServerResponseInternal::<T>, NSP>);
        self.add_system_to_stage(CoreStage::PreUpdate, create_server_response_handlers::<T, NSP>)
    }
}

fn create_server_response_handlers<T: ServerRequestMessage, NSP: NetworkClientProvider>(
    mut responses: EventReader<NetworkData<ServerResponseInternal<T::ServerResponseMessage>>>,
    mut response_map: ResMut<HashMap<u64, Sender<T::ServerResponseMessage>>>,
){
    for response in responses.iter(){
        if let Some(sender) = response_map.remove(&response.response_id){
            sender.try_send(response.response.clone());
        }
    }
}