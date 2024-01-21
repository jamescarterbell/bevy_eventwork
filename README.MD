> `bevy_eventwork` is a solution to the "How do I connect multiple bevy instances" problem in your [bevy](https://bevyengine.org/) games, forked from the excellent [`bevy_spicy_networking`](https://crates.io/crates/bevy_spicy_networking), with significant changes for modularity and including the removal of big dependencies like tokio and typetag.

### Contents

- [Documentation](#documentation)
  - [Quickstart](#quickstart)
- [Bevy Version Compatibility](#bevy-version-compatibility)
- [Supported Platforms](#supported-platforms)
- [Roadmap](#roadmap)
- [Crates using `bevy_eventwork`](#crates-using-bevy_eventwork)
- [Transport providers for`bevy_eventwork`](#transport-providers-for-bevy_eventwork)
- [Runtimes for`bevy_eventwork`](#transport-providers-for-bevy_eventwork)
- [Contributing](#contributing)

## Documentation

You can check out the [**online documentation**](https://docs.rs/bevy_eventwork), or build it yourself by cloning this repo and running `cargo doc -p bevy_eventwork`.

For examples, check out the [examples directory](https://github.com/jamescarterbell/bevy_eventwork/tree/master/examples).

- In `server.rs` you will find a simple chat server, that broadcasts the messages it receives from clients
- In `client.rs` you will find a simple graphical chat client, where you can connect to a server and send messages to

(Note: Since bevy does not include a text input widget, it is a very simplified demo. This should be easy to extend once the UI part of bevy
is more complete.)

### Quickstart

1. Add `bevy_eventwork`, and `serde` to your `Cargo.toml`
2. Create the messages you wish to exchange beetween a client and server, or vice-versa.
   - Implement Serialize and Deserialize from Serde on it
   - Implement `NetworkMessage`

```rust
#[derive(Serialize, Deserialize)]
struct WhisperMessage {
    recipient: UserId,
    message: String,
}

/// In this example, we'll be sending this from a client to a server,
/// BUT, any eventwork bevy instance could recieve the message as
/// long as they register to listen for it.
impl NetworkMessage for WhisperMessage {
    const NAME: &'static str = "example:WhisperMessage";
}
```

3. On the recipient side, set-up the app and register the type to be received

```rust
use bevy_eventwork::{AppNetworkMessage, tcp::TcpProvider};

fn main() {
    let mut app = App::new();

    /// Add the EventworkPlugin specifying what Transport to use and what runtime
    app.add_plugins(bevy_eventwork::EventworkPlugin::<
        TcpProvider,
        bevy::tasks::TaskPool,
    >::default());

    /// Insert your desired runtime pool into the app wrapped in EventworkRuntime
    app.insert_resource(EventworkRuntime(
        TaskPoolBuilder::new().num_threads(2).build(),
    ));

    /// Register any messages to be heard
    ///
    /// Now whenever a client sends a `WhisperMessage` the server will generate an event of
    /// `NetworkData<WhisperMessage>` which your application can then handle
    app.listen_for_message::<WhisperMessage, TcpProvider>();
}
```

4. Listen for events of that type

```rust
fn handle_incoming_whisper_messages(
    mut whisper_messages: EventReader<NetworkMessage<WhisperMessage>>,
) {
    for whisper_message in whisper_messages.iter() {
        // Handle the whisper
    }
}
```

## Request/Response

Starting with version 0.7.1, you can now automatically handle Request/Response style messaging with event work! Check the [documentation](https://docs.rs/bevy_eventwork/latest/bevy_eventwork/managers/network_request/index.html) for more info!

## Bevy Version Compatibility

Simply pick the version compatible to your bevy version:

| Bevy Eventwork | Bevy |
| :------------: | :--: |
|      0.8       | 0.12 |
|      0.7       | 0.8  |

Any version that is not compatible with the latest bevy version is in maintenance mode.
It will only receive minor bug fixes from my side, or community supplied ones.

## Supported Platforms

- **Linux**
- **Windows**
- **WASM** (With a Wasm compatible transport provider like [BEMW](https://github.com/NoahShomette/bevy_eventwork_mod_websockets))

The above three platforms are officially supported. **MacOS** should work but I do not have a Mac to test. If you have a Mac, and wish to test it out and report back, please let me know!

## Roadmap

- General code cleanup, testing, and documentation work
- Message wide event pipelines
  - Useful for mapping connection id to user provided ids
- Message type mapping
  - Currently the message type is sent as a string with each method, it would be nice if you could just send a few bytes instead.
- RPCs!

## Crates using `bevy_eventwork`

> Currently none, you can help by expanding this list. Just send a PR and add it to the table below!

| Name | Version |
| :--: | :-----: |
|  -   |    -    |

## Transport providers for `bevy_eventwork`

|                                                 Name                                                  | Version |
| :---------------------------------------------------------------------------------------------------: | :-----: |
|                                       eventwork_tcp (included)                                        |   0.8   |
| bevy_eventwork_mod_websockets ([LINK](https://github.com/NoahShomette/bevy_eventwork_mod_websockets)) |   0.1   |

## Contributing

To contribute, simply fork the repository and send a PR.

Feel free to chat me up on the bevy discord under `@SirCarter#8209` if you have any questions, suggestions, or I'm not looking into your PR fast enough and you need someone to yell at (respectfully of course).
