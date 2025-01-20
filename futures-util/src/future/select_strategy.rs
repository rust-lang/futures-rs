/// When used with a select future, this will make the future biased.
/// When multiple futures are ready, the winner will be the first one
/// specified.
#[derive(Debug)]
pub struct Biased;

/// When used with a select future, this will make the future fair.
/// When multiple futures are ready, the winner will be pseudo-randomly
/// selected. This is the default behavior.
#[derive(Debug)]
pub struct Fair;

/// Reports whether the type is an instance of [`Biased`] or not.
pub trait IsBiased {
    /// Contains the answer to our question: is this biased?
    const IS_BIASED: bool;
}

impl IsBiased for Biased {
    const IS_BIASED: bool = true;
}

impl IsBiased for Fair {
    const IS_BIASED: bool = false;
}
