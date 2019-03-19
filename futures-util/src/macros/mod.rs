#[macro_use]
mod poll;

macro_rules! cfg_target_has_atomic {
    ($($item:item)*) => {$(
        #[cfg_attr(
            feature = "cfg-target-has-atomic",
            cfg(all(target_has_atomic = "cas", target_has_atomic = "ptr"))
        )]
        $item
    )*};
}
