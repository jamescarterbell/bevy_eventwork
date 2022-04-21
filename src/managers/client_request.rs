use std::{marker::PhantomData, sync::atomic::AtomicU64, fmt::Debug};

use async_channel::{Sender, Receiver};
use bevy::{prelude::{debug, App, CoreStage, EventReader, EventWriter, Res, ResMut}, utils::HashMap, ecs::system::SystemParam};
use dashmap::DashMap;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use either::Either;

use crate::{ServerMessage, ClientMessage, NetworkData, ConnectionId, NetworkPacket};

use super::{NetworkServerProvider, NetworkServer, NetworkClientProvider, NetworkClient, server::register_server_message, client::register_client_message};

#[derive(SystemParam, Debug)]
/// A wrapper around [`NetworkServer`] that allows for the sending of [`ClientRequestMessage`]'s.
pub struct ClientRequester<'w, 's, T: ClientRequestMessage, NSP: NetworkServerProvider>{
    server: Res<'w, NetworkServer<NSP>>,
    response_map: Res<'w, ResponseMap<T>>,
    #[system_param(ignore)]
    marker: PhantomData<&'s usize>,
}

impl<'w, 's, T: ClientRequestMessage, NSP: NetworkServerProvider> ClientRequester<'w, 's, T, NSP>{
    /// Sends a request and returns an object that will eventually return the response
    pub fn send_request(&self, client_id: ConnectionId, request: T) -> ClientResponse<T::ClientResponseMessage>{
        let (id, response) = self.response_map.get_responder();
        self.server.send_message(client_id, ClientRequestInternal{id, request});
        response
    }
}

/// todo
#[derive(Debug)]
pub struct ClientResponse<T>{
    rx: Receiver<T>
}

impl<T> ClientResponse<T>{
    /// todo
    pub fn try_recv(self) -> Either<T, ClientResponse<T>>{
        if let Ok(res) = self.rx.try_recv(){
            Either::Left(res)
        } else {
            Either::Right(self)
        }

    }
}

#[derive(Debug)]
/// Technically an internal type, public for use in system pram
pub struct ResponseMap<T: ClientRequestMessage>{
    count: AtomicU64,
    map: DashMap<u64, Sender<T::ClientResponseMessage>>
}

impl<T: ClientRequestMessage> Default for ResponseMap<T>{
    fn default() -> Self {
        Self { count: Default::default(), map: DashMap::new() }
    }
}

impl<T: ClientRequestMessage> ResponseMap<T>{
    fn get_responder(&self) -> (u64, ClientResponse<T::ClientResponseMessage>){
        let id = self.count.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        let (tx, rx) = async_channel::bounded(1);
        self.map.insert(id, tx);
        (id, ClientResponse{
            rx
        })
    }

    fn remove(&self, id: &u64) -> Option<Sender<T::ClientResponseMessage>>{
        self.map.remove(id).map(|inner| inner.1)
    }
}

/// Marks a type as a request type.
pub trait ClientRequestMessage: Clone + Serialize + DeserializeOwned + Send + Sync + Debug + 'static{
    /// The response type for the request.
    type ClientResponseMessage: Clone + Serialize + DeserializeOwned + Send + Sync + Debug + 'static;

    /// The label used for the request type, same rules as [`ServerMessage`] in terms of naming.
    const REQUEST_NAME: &'static str;
    /// The label used for the request type, same rules as [`ClientMessage`] in terms of naming.
    const RESPONSE_NAME: &'static str;
}

#[derive(Serialize, Deserialize)]
struct ClientRequestInternal<T>{
    id: u64,
    request: T
}

impl<T: ClientRequestMessage> ClientMessage for ClientRequestInternal<T>{
    const NAME: &'static str = T::REQUEST_NAME;
}

/// A wrapper around a request that automatically handles writing
/// the response to eventwork for network transmission.
struct ClientRequest<T: ClientRequestMessage>{
    request: T,
    request_id: u64,
    response_tx: Sender<NetworkPacket>,
}

impl<T: ClientRequestMessage> ClientRequest<T>{
    pub fn get_request(&self) -> &T{
        &self.request
    }

    pub fn respond(self, response: T::ClientResponseMessage){
        let packet = NetworkPacket {
            kind: String::from(T::RESPONSE_NAME),
            data: bincode::serialize(&response).unwrap(),
        };

        self.response_tx.try_send(packet);
    }
}

/// A utility trait on [`App`] to easily register [`ClientRequestMessage`]s for servers to recieve
pub trait AppNetworkClientRequestMessage {
    /// Register a server request message type to listen for on the server
    fn listen_for_server_request_message<T: ClientRequestMessage, NCP: NetworkClientProvider>(
        &mut self,
    ) -> &mut Self;
}

impl AppNetworkClientRequestMessage for App {
    fn listen_for_server_request_message<T: ClientRequestMessage, NCP: NetworkClientProvider>(
        &mut self,
    ) -> &mut Self {
        let server = self.world.get_resource::<NetworkClient<NCP>>().expect("Could not find `NetworkServer`. Be sure to include the `ServerPlugin` before listening for server messages.");

        debug!("Registered a new ServerMessage: {}", ClientRequestInternal::<T>::NAME);

        assert!(
            !server.recv_message_map.contains_key(ClientRequestInternal::<T>::NAME),
            "Duplicate registration of ServerMessage: {}",
            ClientRequestInternal::<T>::NAME
        );
        server.recv_message_map.insert(ClientRequestInternal::<T>::NAME, Vec::new());
        self.add_event::<NetworkData<ClientRequestInternal<T>>>();
        self.add_event::<ClientRequest<T>>();
        self.add_system_to_stage(CoreStage::PreUpdate, register_client_message::<ClientRequestInternal::<T>, NCP>);
        self.add_system_to_stage(CoreStage::PreUpdate, create_client_request_handlers::<T, NCP>)
    }
}

fn create_client_request_handlers<T: ClientRequestMessage, NCP: NetworkClientProvider>(
    mut requests: EventReader<NetworkData<ClientRequestInternal<T>>>,
    mut requests_wrapped: EventWriter<ClientRequest<T>>,
    network: Res<NetworkClient<NCP>>,
){
    for request in requests.iter(){
        if let Some(connection) = &network.server_connection{
            requests_wrapped.send(
                ClientRequest { 
                    request: request.request.clone(),
                    request_id: request.id,
                    response_tx: connection.send_message.clone()
                }
            );
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ClientResponseInternal<T>{
    response_id: u64,
    response: T
}

impl<T: ClientRequestMessage> ServerMessage for ClientResponseInternal<T>{
    const NAME: &'static str = T::RESPONSE_NAME;
}

/// A utility trait on [`App`] to easily register [`ClientRequestMessage::ClientResponseMessage`]s for clients to recieve
pub trait AppNetworkClientResponseMessage {
    /// Register a server request message type to listen for on the server
    fn listen_for_client_response_message<T: ClientRequestMessage, NSP: NetworkServerProvider>(
        &mut self,
    ) -> &mut Self;
}

impl AppNetworkClientResponseMessage for App {
    fn listen_for_client_response_message<T: ClientRequestMessage, NSP: NetworkServerProvider>(
        &mut self,
    ) -> &mut Self {
        self.insert_resource(ResponseMap::<T>::default());
        let client = self.world.get_resource::<NetworkServer<NSP>>().expect("Could not find `NetworkServer`. Be sure to include the `ServerPlugin` before listening for server messages.");

        debug!("Registered a new ServerMessage: {}", ClientResponseInternal::<T>::NAME);

        assert!(
            !client.recv_message_map.contains_key(ClientResponseInternal::<T>::NAME),
            "Duplicate registration of ServerMessage: {}",
            ClientResponseInternal::<T>::NAME
        );
        client.recv_message_map.insert(ClientResponseInternal::<T>::NAME, Vec::new());
        self.add_event::<NetworkData<ClientResponseInternal<T>>>();
        self.add_system_to_stage(CoreStage::PreUpdate, register_server_message::<ClientResponseInternal::<T>, NSP>);
        self.add_system_to_stage(CoreStage::PreUpdate, create_client_response_handlers::<T, NSP>)
    }
}

fn create_client_response_handlers<T: ClientRequestMessage, NSP: NetworkServerProvider>(
    mut responses: EventReader<NetworkData<ClientResponseInternal<T::ClientResponseMessage>>>,
    mut response_map: ResMut<ResponseMap<T>>,
){
    for response in responses.iter(){
        if let Some(sender) = response_map.remove(&response.response_id){
            sender.try_send(response.response.clone());
        }
    }
}