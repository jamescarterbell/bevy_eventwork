use std::{fmt::Debug, marker::PhantomData, sync::atomic::AtomicU64};

use async_channel::{Receiver, Sender};
use bevy::{
    ecs::system::SystemParam,
    prelude::{debug, App, Event, EventReader, EventWriter, PreUpdate, Res, ResMut, Resource},
};
use dashmap::DashMap;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{error::NetworkError, ConnectionId, NetworkData, NetworkMessage, NetworkPacket};

use super::{network::register_message, Network, NetworkProvider};

#[derive(SystemParam, Debug)]
/// A wrapper around [`Network`] that allows for the sending of [`RequestMessage`]'s.
pub struct Requester<'w, 's, T: RequestMessage, NP: NetworkProvider> {
    server: Res<'w, Network<NP>>,
    response_map: Res<'w, ResponseMap<T>>,
    #[system_param(ignore)]
    marker: PhantomData<&'s usize>,
}

impl<'w, 's, T: RequestMessage, NP: NetworkProvider> Requester<'w, 's, T, NP> {
    /// Sends a request and returns an object that will eventually return the response
    pub fn send_request(
        &self,
        client_id: ConnectionId,
        request: T,
    ) -> Result<Response<T::ResponseMessage>, NetworkError> {
        let (id, response) = self.response_map.get_responder();
        self.server
            .send_message(client_id, RequestInternal { id, request })?;
        Ok(response)
    }
}

/// The eventual response of a remote request.
#[derive(Debug)]
pub struct Response<T> {
    rx: Receiver<T>,
}

impl<T> Response<T> {
    /// Try to recieve the response, then drop the underlying machinery for handling the request.
    /// On err, we simply return the object to be checked again later.
    pub fn try_recv(self) -> Result<T, Response<T>> {
        if let Ok(res) = self.rx.try_recv() {
            Ok(res)
        } else {
            Err(self)
        }
    }
}

#[derive(Debug, Resource)]
/// Technically an internal type, public for use in system pram
pub struct ResponseMap<T: RequestMessage> {
    count: AtomicU64,
    map: DashMap<u64, Sender<T::ResponseMessage>>,
}

impl<T: RequestMessage> Default for ResponseMap<T> {
    fn default() -> Self {
        Self {
            count: Default::default(),
            map: DashMap::new(),
        }
    }
}

impl<T: RequestMessage> ResponseMap<T> {
    fn get_responder(&self) -> (u64, Response<T::ResponseMessage>) {
        let id = self
            .count
            .fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        let (tx, rx) = async_channel::bounded(1);
        self.map.insert(id, tx);
        (id, Response { rx })
    }

    fn remove(&self, id: &u64) -> Option<Sender<T::ResponseMessage>> {
        self.map.remove(id).map(|inner| inner.1)
    }
}

/// Marks a type as a request type.
pub trait RequestMessage:
    Clone + Serialize + DeserializeOwned + Send + Sync + Debug + 'static
{
    /// The response type for the request.
    type ResponseMessage: Clone + Serialize + DeserializeOwned + Send + Sync + Debug + 'static;

    /// The label used for the request type, same rules as [`ServerMessage`] in terms of naming.
    const REQUEST_NAME: &'static str;
    /// The label used for the request type, same rules as [`ClientMessage`] in terms of naming.
    const RESPONSE_NAME: &'static str;
}

#[derive(Serialize, Deserialize)]
struct RequestInternal<T> {
    id: u64,
    request: T,
}

impl<T: RequestMessage> NetworkMessage for RequestInternal<T> {
    const NAME: &'static str = T::REQUEST_NAME;
}

/// A wrapper around a request that automatically handles writing
/// the response to eventwork for network transmission.
#[derive(Debug, Event)]
pub struct Request<T: RequestMessage> {
    request: T,
    request_id: u64,
    response_tx: Sender<NetworkPacket>,
}

impl<T: RequestMessage> Request<T> {
    /// Read the underlying request
    #[inline(always)]
    pub fn get_request(&self) -> &T {
        &self.request
    }

    /// Consume the request and automatically send the response back to the client.
    pub fn respond(self, response: T::ResponseMessage) -> Result<(), NetworkError> {
        let packet = NetworkPacket {
            kind: String::from(T::RESPONSE_NAME),
            data: bincode::serialize(&ResponseInternal {
                response_id: self.request_id,
                response,
            })
            .map_err(|_| NetworkError::Serialization)?,
        };

        self.response_tx
            .try_send(packet)
            .map_err(|_| NetworkError::SendError)
    }
}

/// A utility trait on [`App`] to easily register [`RequestMessage`]s for servers to recieve
pub trait AppNetworkRequestMessage {
    /// Register a server request message type to listen for on the server
    fn listen_for_request_message<T: RequestMessage, NP: NetworkProvider>(&mut self) -> &mut Self;
}

impl AppNetworkRequestMessage for App {
    fn listen_for_request_message<T: RequestMessage, NP: NetworkProvider>(&mut self) -> &mut Self {
        let server = self.world.get_resource::<Network<NP>>().expect("Could not find `Network`. Be sure to include the `ServerPlugin` before listening for server messages.");

        debug!(
            "Registered a new ServerMessage: {}",
            RequestInternal::<T>::NAME
        );

        assert!(
            !server
                .recv_message_map
                .contains_key(RequestInternal::<T>::NAME),
            "Duplicate registration of ServerMessage: {}",
            RequestInternal::<T>::NAME
        );
        server
            .recv_message_map
            .insert(RequestInternal::<T>::NAME, Vec::new());
        self.add_event::<NetworkData<RequestInternal<T>>>();
        self.add_event::<Request<T>>();
        self.add_systems(
            PreUpdate,
            (
                create_request_handlers::<T, NP>,
                register_message::<RequestInternal<T>, NP>,
            ),
        )
    }
}

fn create_request_handlers<T: RequestMessage, NP: NetworkProvider>(
    mut requests: EventReader<NetworkData<RequestInternal<T>>>,
    mut requests_wrapped: EventWriter<Request<T>>,
    network: Res<Network<NP>>,
) {
    for request in requests.read() {
        if let Some(connection) = &network.established_connections.get(request.source()) {
            requests_wrapped.send(Request {
                request: request.request.clone(),
                request_id: request.id,
                response_tx: connection.send_message.clone(),
            });
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ResponseInternal<T> {
    response_id: u64,
    response: T,
}

impl<T: RequestMessage> NetworkMessage for ResponseInternal<T> {
    const NAME: &'static str = T::RESPONSE_NAME;
}

/// A utility trait on [`App`] to easily register [`RequestMessage::ResponseMessage`]s for clients to recieve
pub trait AppNetworkResponseMessage {
    /// Register a server request message type to listen for on the server
    fn listen_for_response_message<T: RequestMessage, NP: NetworkProvider>(&mut self) -> &mut Self;
}

impl AppNetworkResponseMessage for App {
    fn listen_for_response_message<T: RequestMessage, NP: NetworkProvider>(&mut self) -> &mut Self {
        self.insert_resource(ResponseMap::<T>::default());
        let client = self.world.get_resource::<Network<NP>>().expect("Could not find `Network`. Be sure to include the `ServerPlugin` before listening for server messages.");

        debug!(
            "Registered a new ServerMessage: {}",
            ResponseInternal::<T>::NAME
        );

        assert!(
            !client
                .recv_message_map
                .contains_key(ResponseInternal::<T>::NAME),
            "Duplicate registration of ServerMessage: {}",
            ResponseInternal::<T>::NAME
        );
        client
            .recv_message_map
            .insert(ResponseInternal::<T>::NAME, Vec::new());
        self.add_event::<NetworkData<ResponseInternal<T>>>();
        self.add_systems(
            PreUpdate,
            (
                register_message::<ResponseInternal<T>, NP>,
                create_client_response_handlers::<T>,
            ),
        )
    }
}

fn create_client_response_handlers<T: RequestMessage>(
    mut responses: EventReader<NetworkData<ResponseInternal<T::ResponseMessage>>>,
    response_map: ResMut<ResponseMap<T>>,
) {
    for response in responses.read() {
        if let Some(sender) = response_map.remove(&response.response_id) {
            sender
                .try_send(response.response.clone())
                .expect("Internal channel closed!");
        }
    }
}
