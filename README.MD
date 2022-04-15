> `bevy_eventwork` is a solution to the "How do I connect multiple clients to a single server" problem in your [bevy](https://bevyengine.org/) games, forked from the excellent [`bevy_spicy_networking`](https://crates.io/crates/bevy_spicy_networking), with significant changes for modularity and including the removal of big dependencies like tokio and typetag.

<hr>

Simply add either the server/client plugins, your chosen transport provider (a simple tcp provider is provided), and your runtime, then register the types of messages you plan on receiving and listen for them as events!


<hr>

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


Documentation
-------------

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
    - Implement `ServerMessage` when it is sent to the server from a client.
    - Implement `ClientMessage` when it is sent to a client from the server.

```rust
#[derive(Serialize, Deserialize)]
struct WhisperMessage {
    recipient: UserId,
    message: String,
}

// In this case, its a client sending a message to a server
impl ServerMessage for WhisperMessage {
    const NAME: &'static str = "example:WhisperMessage"; // This name needs to be unique!
    // Otherwise, the recipient will mix up the messages
}
```

3. On the recipient side, register the type to be received

```rust
use bevy_eventwork::AppNetworkServerMessage;

let appbuilder: &mut AppBuilder = /* Get an AppBuilder, which is returned by bevy from App::build() */;


// Now whenever a client sends a `WhisperMessage` the server will generate an event of
// `NetworkData<WhisperMessage>` which your application can then handle
appbuilder.listen_for_server_message::<WhisperMessage>();
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


Bevy Version Compatibility
--------------------------

Simply pick the version compatible to your bevy version:

| Bevy Eventwork | Bevy  |
| :------------: | :---: |
|       > 0.6      |  > 0.6  |

Any version that is not compatible with the latest bevy version is in maintenance mode.
It will only receive minor bug fixes from my side, or community supplied ones.

Supported Platforms
-------------------

Currently only **Linux** and **Windows** are officially supported, I don't see why **MacOS** wouldn't be, but I do not have a Mac to test. If you have a Mac, and wish to test it out and report back, please let me know!

Roadmap
-------

* Message wide event pipelines
    * Useful for mapping connection id to user provided ids
* Message type mapping (
    * Currently the message type is sent as a string with each method, it would be nice if you could just send a few bytes instead.


Crates using `bevy_eventwork`
-----------------------------

> Currently none, you can help by expanding this list. Just send a PR and add it to the table below!

| Name | Version |
|:----:|:-------:|
|  -   |    -    |

Transport providers for `bevy_eventwork`
----------------------------------------


| Name            | Version |
|:---------------:|:-------:|
|  eventwork_tcp (included)  |   0.6   |

Contributing
------------

To contribute, simply fork the repository and send a PR.

 Feel free to chat me up on the bevy discord under `@SirCarter#8209` if you have any questions, suggestions, or I'm not looking into your PR fast enough and you need someone to yell at (respectfully of course).
