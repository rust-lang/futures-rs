use super::{Map, Flatten};

/// Future for the [`then`](super::FutureExt::then) method.
pub type Then<Fut1, F> = Flatten<Map<Fut1, F>>;
