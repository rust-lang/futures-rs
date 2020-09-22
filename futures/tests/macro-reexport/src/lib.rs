// normal reexport
pub use futures03::{join, try_join, select, select_biased};

// reexport + rename
pub use futures03::{
    join as join2, try_join as try_join2,
    select as select2, select_biased as select_biased2,
};
