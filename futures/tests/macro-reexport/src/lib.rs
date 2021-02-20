// normal reexport
pub use futures04::{join, try_join, select, select_biased};

// reexport + rename
pub use futures04::{
    join as join2, try_join as try_join2,
    select as select2, select_biased as select_biased2,
};
