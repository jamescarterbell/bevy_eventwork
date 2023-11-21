use async_net::Ipv4Addr;
use bevy::tasks::TaskPool;
use bevy::{prelude::*, tasks::TaskPoolBuilder};
use bevy_eventwork::{ConnectionId, EventworkRuntime, Network, NetworkData, NetworkEvent};
use std::net::{IpAddr, SocketAddr};

use bevy_eventwork::tcp::{NetworkSettings, TcpProvider};

mod shared;

fn main() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::log::LogPlugin::default()));

    // Before we can register the potential message types, we
    // need to add the plugin
    app.add_plugins(bevy_eventwork::EventworkPlugin::<
        TcpProvider,
        bevy::tasks::TaskPool,
    >::default());

    // Make sure you insert the EventworkRuntime resource with your chosen Runtime
    app.insert_resource(EventworkRuntime(
        TaskPoolBuilder::new().num_threads(2).build(),
    ));

    // A good way to ensure that you are not forgetting to register
    // any messages is to register them where they are defined!
    shared::server_register_network_messages(&mut app);

    app.add_systems(Startup, setup_networking);
    app.add_systems(Update, (handle_connection_events, handle_messages));

    // We have to insert the TCP [`NetworkSettings`] with our chosen settings.
    app.insert_resource(NetworkSettings::default());

    app.run();
}

// On the server side, you need to setup networking. You do not need to do so at startup, and can start listening
// at any time.
fn setup_networking(
    mut net: ResMut<Network<TcpProvider>>,
    settings: Res<NetworkSettings>,
    task_pool: Res<EventworkRuntime<TaskPool>>,
) {
    let ip_address = "127.0.0.1".parse().expect("Could not parse ip address");

    info!("Address of the server: {}", ip_address);

    let _socket_address = SocketAddr::new(ip_address, 9999);

    match net.listen(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        &task_pool.0,
        &settings,
    ) {
        Ok(_) => (),
        Err(err) => {
            error!("Could not start listening: {}", err);
            panic!();
        }
    }

    info!("Started listening for new connections!");
}

#[derive(Component)]
struct Player(ConnectionId);

fn handle_connection_events(
    mut commands: Commands,
    net: Res<Network<TcpProvider>>,
    mut network_events: EventReader<NetworkEvent>,
) {
    for event in network_events.read() {
        if let NetworkEvent::Connected(conn_id) = event {
            commands.spawn((Player(*conn_id),));

            // Broadcasting sends the message to all connected players! (Including the just connected one in this case)
            net.broadcast(shared::NewChatMessage {
                name: String::from("SERVER"),
                message: format!("New user connected; {}", conn_id),
            });
            info!("New player connected: {}", conn_id);
        }
    }
}

// Receiving a new message is as simple as listening for events of `NetworkData<T>`
fn handle_messages(
    mut new_messages: EventReader<NetworkData<shared::UserChatMessage>>,
    net: Res<Network<TcpProvider>>,
) {
    for message in new_messages.read() {
        let user = message.source();

        info!("Received message from user: {}", message.message);

        net.broadcast(shared::NewChatMessage {
            name: format!("{}", user),
            message: message.message.clone(),
        });
    }
}
