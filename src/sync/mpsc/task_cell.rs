use std::fmt;
use std::mem;
use std::cell::UnsafeCell;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use task;
use task::Task;


#[repr(usize)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SlotState {
    Empty = 1,
    // Task is written in slot.
    TaskWritten = 2,
    // Slot is locked by park call.
    LockedByPark = 3,
    // Slot is locked by notify call.
    LockedByNotify = 4,
}

impl SlotState {
    #[inline(always)]
    fn encode(&self) -> usize {
        *self as usize
    }

    #[inline(always)]
    fn decode(v: usize) -> SlotState {
        unsafe { mem::transmute(v) }
    }
}

#[inline(always)]
fn validate_state(s0: SlotState, s1: SlotState) {
    let s0_park = s0 == SlotState::LockedByPark || s0 == SlotState::TaskWritten;
    let s1_park = s1 == SlotState::LockedByPark || s1 == SlotState::TaskWritten;
    debug_assert!(!(s0_park && s1_park),
        "{:?} {:?}", s0, s1);

    debug_assert!(s0 != SlotState::LockedByNotify || s1 != SlotState::LockedByNotify,
        "{:?} {:?}", s0, s1);
}

#[inline(always)]
fn decode_state(v: usize) -> (SlotState, SlotState) {
    let s0 = SlotState::decode(v & 0xff);
    let s1 = SlotState::decode(v >> 8);
    (s0, s1)
}

#[inline(always)]
fn decode_state_flip(v: usize, flip: bool) -> (SlotState, SlotState) {
    let (s0, s1) = decode_state(v);
    if !flip { (s0, s1) } else { (s1, s0) }
}

#[inline(always)]
fn encode_state(s0: SlotState, s1: SlotState) -> usize {
    validate_state(s0, s1);
    s0.encode() | (s1.encode() << 8)
}

#[inline(always)]
fn encode_state_flip(s0: SlotState, s1: SlotState, flip: bool) -> usize {
    if !flip {
        encode_state(s0, s1)
    } else {
        encode_state(s1, s0)
    }
}


pub struct TaskCell {
    // Pair of `(SlotState, SlotState)`.
    state: AtomicUsize,
    // Two slots.
    // At least one slot is always available for park.
    tasks: [UnsafeCell<Option<Task>>; 2],
}

impl fmt::Debug for TaskCell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskNotify")
            .field("state", &decode_state(self.state.load(Ordering::Relaxed)))
            .finish()
    }
}

impl TaskCell {
    pub fn new() -> TaskCell {
        TaskCell {
            state: AtomicUsize::new(encode_state(SlotState::Empty, SlotState::Empty)),
            tasks: [UnsafeCell::new(None), UnsafeCell::new(None)],
        }
    }

    #[inline(always)]
    fn store_state_for_slot(&self, index: usize, slot_state: SlotState) -> usize {
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            let (_sa, sb) = decode_state_flip(state, index == 1);
            let new_state = encode_state_flip(slot_state, sb, index == 1);
            state = match self.state.compare_exchange(
                state, new_state, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return new_state,
                Err(state) => state,
            };
        }
    }

    /// Store current task.
    ///
    /// Panics if called concurrently.
    pub fn store_current(&self) {
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            let (slot, sb) = match decode_state(state) {
                (SlotState::LockedByPark, _) |
                (_, SlotState::LockedByPark) => {
                    panic!("`park` must not be called concurrently");
                }
                // If task is written to some slot,
                // that slot must be overwritten in `park`.
                (SlotState::TaskWritten, sb) => (0, sb),
                (sb, SlotState::TaskWritten) => (1, sb),
                // Otherwise write to empty slot.
                (SlotState::Empty, sb) => (0, sb),
                (sb, SlotState::Empty) => (1, sb),
                _ => unreachable!(),
            };

            debug_assert!(sb != SlotState::TaskWritten && sb != SlotState::LockedByPark,
                "{:?}", decode_state_flip(state, slot == 1));

            let locked = encode_state_flip(SlotState::LockedByPark, sb, slot == 1);

            state = match self.state.compare_exchange(
                state, locked, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => {
                    unsafe { *self.tasks[slot].get() = Some(task::current()); }
                    // Release lock.
                    self.store_state_for_slot(slot, SlotState::TaskWritten);
                    return;
                }
                Err(state) => state,
            };
        }
    }

    /// Wake up waiting task if any.
    pub fn notify(&self) {
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            state = match decode_state(state) {
                (SlotState::Empty, SlotState::Empty) => {
                    // Nobody is waiting.
                    return;
                },
                (SlotState::LockedByPark, _) |
                (_, SlotState::LockedByPark) => {
                    // Parking task *must* check state after `park`,
                    // so current task may skip notify (and it can't).
                    return;
                }
                (SlotState::LockedByNotify, _) |
                (_, SlotState::LockedByNotify) => {
                    // Another task is notifying now, it will check for task again
                    // after it releases the lock.
                    return;
                }
                (SlotState::TaskWritten, sb) |
                (sb, SlotState::TaskWritten) => {
                    let slot_0 = decode_state(state).0 == SlotState::TaskWritten;
                    let slot = if slot_0 { 0 } else { 1 };

                    let locked = encode_state_flip(SlotState::LockedByNotify, sb, slot == 1);
                    match self.state.compare_exchange(
                        state, locked, Ordering::SeqCst, Ordering::SeqCst)
                    {
                        Ok(_) => {
                            unsafe { (*self.tasks[slot].get()).take() }.unwrap().notify();
                            // Release lock.
                            let state = self.store_state_for_slot(slot, SlotState::Empty);

                            // It is important to continue here instead of return.
                            // Another task could park in next slot during this notify,
                            // and notify followed by that park will be ignored
                            // because it could not acquire lock.
                            // So this notify should continue notifying.

                            state
                        }
                        Err(state) => state,
                    }
                }
            };
        }
    }
}
