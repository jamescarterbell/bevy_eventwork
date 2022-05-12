use serde::{de::DeserializeOwned, Serialize};

/// Any type that should be sent over the wire has to implement [`NetworkMessage`].
///
/// ## Example
/// ```rust
/// use bevy_eventwork::NetworkMessage;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct PlayerInformation {
///     health: usize,
///     position: (u32, u32, u32)
/// }
///
/// impl NetworkMessage for PlayerInformation {
///     const NAME: &'static str = "PlayerInfo";
/// }
/// ```

/// Marks a type as an eventwork message
pub trait NetworkMessage: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// A unique name to identify your message, this needs to be unique __across all included crates__
    ///
    /// A good combination is crate name + struct name.
    const NAME: &'static str;
}
