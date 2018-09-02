use std::usize;

// Struct representation of `Inner::state`.
#[derive(Debug, Clone, Copy)]
pub(super) struct State {
    // `true` when the channel is open
    pub(super) is_open: bool,

    // Number of messages in the channel
    pub(super) num_messages: usize,
}

// The `is_open` flag is stored in the left-most bit of `Inner::state`
const OPEN_MASK: usize = usize::MAX - (usize::MAX >> 1);

// When a new channel is created, it is created in the open state with no
// pending messages.
pub(super) const INIT_STATE: usize = OPEN_MASK;

// The maximum number of messages that a channel can track is `usize::MAX >> 1`
pub(super) const MAX_CAPACITY: usize = !(OPEN_MASK);

// The maximum requested buffer size must be less than the maximum capacity of
// a channel. This is because each sender gets a guaranteed slot.
pub(super) const MAX_BUFFER: usize = MAX_CAPACITY >> 1;

pub(super) fn decode_state(num: usize) -> State {
    State {
        is_open: num & OPEN_MASK == OPEN_MASK,
        num_messages: num & MAX_CAPACITY,
    }
}

pub(super) fn encode_state(state: &State) -> usize {
    let mut num = state.num_messages;

    if state.is_open {
        num |= OPEN_MASK;
    }

    num
}
