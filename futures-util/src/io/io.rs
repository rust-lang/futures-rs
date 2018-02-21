//! I/O conveniences when working with primitives in `tokio-core`
//!
//! Contains various combinators to work with I/O objects and type definitions
//! as well.
//!
//! A description of the high-level I/O combinators can be [found online] in
//! addition to a description of the [low level details].
//!
//! [found online]: https://tokio.rs/docs/getting-started/core/
//! [low level details]: https://tokio.rs/docs/going-deeper-tokio/core-low-level/

pub use io::allow_std::AllowStdIo;
pub use io::copy::{copy, Copy};
pub use io::flush::{flush, Flush};
//pub use io::lines::{lines, Lines};
pub use io::read::{read, Read};
pub use io::read_exact::{read_exact, ReadExact};
pub use io::read_to_end::{read_to_end, ReadToEnd};
//pub use io::read_until::{read_until, ReadUntil};
pub use io::shutdown::{shutdown, Shutdown};
pub use io::split::{ReadHalf, WriteHalf};
pub use io::window::Window;
pub use io::write_all::{write_all, WriteAll};
