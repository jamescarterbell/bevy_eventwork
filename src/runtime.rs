mod bevy_runtime;

use std::future::Future;

use bevy::prelude::{Deref, DerefMut, Resource};

/// A Resource that provides access to the runtime to internal Eventwork systems.
///
/// This *must* be inserted into the app for the Eventwork plugin to work
#[derive(Resource, DerefMut, Deref)]
pub struct EventworkRuntime<RT: Runtime + Send + Sync>(pub RT);

/// A runtime abstraction allowing you to use any runtime for spicy
pub trait Runtime: Send + Sync + 'static {
    /// Associated handle
    type JoinHandle: JoinHandle;

    /// Create a long running background task.
    fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) -> Self::JoinHandle;
}

/// A runtime abstraction allowing you to use any runtime with spicy
pub trait JoinHandle: 'static + Send + Sync {
    /// Stop the task.
    fn abort(&mut self);
}
