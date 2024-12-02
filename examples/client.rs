#![allow(clippy::type_complexity)]

use async_net::{Ipv4Addr, SocketAddr};
use bevy::{
    color::palettes,
    prelude::*,
    state::commands,
    tasks::{TaskPool, TaskPoolBuilder},
};
use bevy_eventwork::{ConnectionId, EventworkRuntime, Network, NetworkData, NetworkEvent};
use std::net::IpAddr;

use bevy_eventwork::tcp::{NetworkSettings, TcpProvider};

mod shared;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins);

    // You need to add the `ClientPlugin` first before you can register
    // `ClientMessage`s
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
    shared::client_register_network_messages(&mut app);

    app.add_systems(Startup, setup_ui);

    app.add_systems(
        Update,
        (
            handle_connect_button,
            handle_message_button,
            handle_incoming_messages,
            handle_network_events,
        ),
    );

    // We have to insert the TCP [`NetworkSettings`] with our chosen settings.
    app.insert_resource(NetworkSettings::default());

    app.init_resource::<GlobalChatSettings>();

    app.add_systems(PostUpdate, handle_chat_area);

    app.run();
}

///////////////////////////////////////////////////////////////
////////////// Incoming Message Handler ///////////////////////
///////////////////////////////////////////////////////////////

fn handle_incoming_messages(
    mut messages: Query<&mut GameChatMessages>,
    mut new_messages: EventReader<NetworkData<shared::NewChatMessage>>,
) {
    let mut messages = messages.get_single_mut().unwrap();

    for new_message in new_messages.read() {
        messages.add(UserMessage::new(&new_message.name, &new_message.message));
    }
}

fn handle_network_events(
    mut new_network_events: EventReader<NetworkEvent>,
    connect_query: Query<&Children, With<ConnectButton>>,
    mut text_query: Query<&mut Text>,
    mut messages: Query<&mut GameChatMessages>,
) {
    let connect_children = connect_query.get_single().unwrap();
    let mut text = text_query.get_mut(connect_children[0]).unwrap();
    let mut messages = messages.get_single_mut().unwrap();

    for event in new_network_events.read() {
        info!("Received event");
        match event {
            NetworkEvent::Connected(_) => {
                messages.add(SystemMessage::new(
                    "Succesfully connected to server!".to_string(),
                ));
                text.0 = String::from("Disconnect");
            }

            NetworkEvent::Disconnected(_) => {
                messages.add(SystemMessage::new("Disconnected from server!".to_string()));
                text.0 = String::from("Connect to server");
            }
            NetworkEvent::Error(err) => {
                messages.add(UserMessage::new(String::from("SYSTEM"), err.to_string()));
            }
        }
    }
}

///////////////////////////////////////////////////////////////
////////////// Data Definitions ///////////////////////////////
///////////////////////////////////////////////////////////////

#[derive(Resource)]
struct GlobalChatSettings {
    chat_style: (TextFont, TextColor),
    author_style: (TextFont, TextColor),
}

impl FromWorld for GlobalChatSettings {
    fn from_world(_world: &mut World) -> Self {
        GlobalChatSettings {
            chat_style: (
                TextFont::from_font_size(20.0),
                TextColor::from(Color::BLACK),
            ),
            author_style: (
                TextFont::from_font_size(20.0),
                TextColor::from(palettes::css::RED),
            ),
        }
    }
}

enum ChatMessage {
    SystemMessage(SystemMessage),
    UserMessage(UserMessage),
}

impl ChatMessage {
    fn get_author(&self) -> String {
        match self {
            ChatMessage::SystemMessage(_) => "SYSTEM".to_string(),
            ChatMessage::UserMessage(UserMessage { user, .. }) => user.clone(),
        }
    }

    fn get_text(&self) -> String {
        match self {
            ChatMessage::SystemMessage(SystemMessage(msg)) => msg.clone(),
            ChatMessage::UserMessage(UserMessage { message, .. }) => message.clone(),
        }
    }
}

impl From<SystemMessage> for ChatMessage {
    fn from(other: SystemMessage) -> ChatMessage {
        ChatMessage::SystemMessage(other)
    }
}

impl From<UserMessage> for ChatMessage {
    fn from(other: UserMessage) -> ChatMessage {
        ChatMessage::UserMessage(other)
    }
}

struct SystemMessage(String);

impl SystemMessage {
    fn new<T: Into<String>>(msg: T) -> SystemMessage {
        Self(msg.into())
    }
}

#[derive(Component)]
struct UserMessage {
    user: String,
    message: String,
}

impl UserMessage {
    fn new<U: Into<String>, M: Into<String>>(user: U, message: M) -> Self {
        UserMessage {
            user: user.into(),
            message: message.into(),
        }
    }
}

#[derive(Component)]
struct ChatMessages<T> {
    messages: Vec<T>,
}

impl<T> ChatMessages<T> {
    fn new() -> Self {
        ChatMessages { messages: vec![] }
    }

    fn add<K: Into<T>>(&mut self, msg: K) {
        let msg = msg.into();
        self.messages.push(msg);
    }
}

type GameChatMessages = ChatMessages<ChatMessage>;

///////////////////////////////////////////////////////////////
////////////// UI Definitions/Handlers ////////////////////////
///////////////////////////////////////////////////////////////

#[derive(Component)]
struct ConnectButton;

fn handle_connect_button(
    net: ResMut<Network<TcpProvider>>,
    settings: Res<NetworkSettings>,
    interaction_query: Query<
        (&Interaction, &Children),
        (Changed<Interaction>, With<ConnectButton>),
    >,
    mut text_query: Query<&mut Text>,
    mut messages: Query<&mut GameChatMessages>,
    task_pool: Res<EventworkRuntime<TaskPool>>,
) {
    let mut messages = if let Ok(messages) = messages.get_single_mut() {
        messages
    } else {
        return;
    };

    for (interaction, children) in interaction_query.iter() {
        let mut text = text_query.get_mut(children[0]).unwrap();
        if let Interaction::Pressed = interaction {
            if net.has_connections() {
                net.disconnect(ConnectionId { id: 0 })
                    .expect("Couldn't disconnect from server!");
            } else {
                text.0 = String::from("Connecting...");
                messages.add(SystemMessage::new("Connecting to server..."));

                net.connect(
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                    &task_pool.0,
                    &settings,
                );
            }
        }
    }
}

#[derive(Component)]
struct MessageButton;

fn handle_message_button(
    net: Res<Network<TcpProvider>>,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<MessageButton>)>,
    mut messages: Query<&mut GameChatMessages>,
) {
    let mut messages = if let Ok(messages) = messages.get_single_mut() {
        messages
    } else {
        return;
    };

    for interaction in interaction_query.iter() {
        if let Interaction::Pressed = interaction {
            match net.send_message(
                ConnectionId { id: 0 },
                shared::UserChatMessage {
                    message: String::from("Hello there!"),
                },
            ) {
                Ok(()) => (),
                Err(err) => messages.add(SystemMessage::new(format!(
                    "Could not send message: {}",
                    err
                ))),
            }
        }
    }
}

#[derive(Component)]
struct ChatArea;

fn handle_chat_area(
    chat_settings: Res<GlobalChatSettings>,
    messages: Query<&GameChatMessages, Changed<GameChatMessages>>,
    mut chat_text_query: Query<(Entity, &mut Text), With<ChatArea>>,
    mut read_messages_index: Local<usize>,
    mut commands: Commands,
) {
    let messages = if let Ok(messages) = messages.get_single() {
        messages
    } else {
        return;
    };
    let (text_entity, mut text) = chat_text_query.get_single_mut().unwrap();

    for message_index in *read_messages_index..messages.messages.len() {
        let message = &messages.messages[message_index];
        let new_message = commands
            .spawn((
                Text::new(format!("{}:", message.get_author())),
                chat_settings.author_style.clone(),
            ))
            .with_child((
                TextSpan::new(format!("{}\n", message.get_text())),
                chat_settings.chat_style.clone(),
            ))
            .id();
        commands.entity(text_entity).add_children(&[new_message]);
    }

    *read_messages_index = messages.messages.len();
}

fn setup_ui(mut commands: Commands, _materials: ResMut<Assets<ColorMaterial>>) {
    commands.spawn(Camera2d);

    commands.spawn((GameChatMessages::new(),));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::ColumnReverse,
                ..default()
            },
            Into::<BackgroundColor>::into(Color::NONE),
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(90.0),
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn((
                            Text::default(),
                            Node {
                                flex_direction: FlexDirection::Column,
                                ..default()
                            },
                        ))
                        .insert(ChatArea);
                });
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(10.0),
                        ..default()
                    },
                    Into::<BackgroundColor>::into(palettes::css::GRAY),
                ))
                .with_children(|parent_button_bar| {
                    parent_button_bar
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(50.0),
                                height: Val::Percent(100.0),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                        ))
                        .insert(MessageButton)
                        .with_children(|button| {
                            button.spawn((
                                Text::new("Send Message!"),
                                TextFont::from_font_size(40.0),
                                TextColor::from(Color::BLACK),
                                TextLayout::new_with_justify(JustifyText::Center),
                            ));
                        });

                    parent_button_bar
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(50.0),
                                height: Val::Percent(100.0),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                        ))
                        .insert(ConnectButton)
                        .with_children(|button| {
                            button.spawn((
                                Text::new("Connect to server"),
                                TextFont::from_font_size(40.0),
                                TextColor::from(Color::BLACK),
                                TextLayout::new_with_justify(JustifyText::Center),
                            ));
                        });
                });
        });
}
