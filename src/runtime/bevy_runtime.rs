use crate::Runtime;

use super::JoinHandle;

impl Runtime for bevy::tasks::TaskPool {
    type JoinHandle = Option<bevy::tasks::Task<()>>;

    fn spawn(
        &self,
        task: impl std::future::Future<Output = ()> + Send + 'static,
    ) -> Self::JoinHandle {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Some(self.spawn(task))
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.spawn(task);
            return None;
        }
    }

    fn spawn_local(
        &self,
        task: impl futures_lite::Future<Output = ()> + 'static,
    ) -> Self::JoinHandle {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Some(self.spawn_local(task))
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.spawn_local(task);
            return None;
        }
    }
}

impl JoinHandle for Option<bevy::tasks::Task<()>> {
    fn abort(&mut self) {
        self.take();
    }
}
