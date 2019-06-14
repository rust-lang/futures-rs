//! Additional combinators for testing async IO.

mod interleave_pending;

pub mod read;
pub use read::AsyncReadTestExt;

pub mod write;
pub use write::AsyncWriteTestExt;
