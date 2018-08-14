use futures_core::task::SpawnErrorKind;

/// The result of a failed spawn
#[derive(Debug)]
pub struct SpawnError {
    /// The kind of error
    pub kind: SpawnErrorKind,
}
