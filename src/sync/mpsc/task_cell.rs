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
    // Slot is empty.
    Empty = 1,
    // Slot contains `Some(task)` which needs to be notified on next `notify`.
    TaskWritten = 2,
    // Slot is locked by `park` call.
    LockedByPark = 3,
    // Slot is locked by `notify` call.
    LockedByNotify = 4,
    // Slot is locked by `notify` call, but when it's done, it must notify again.
    LockedByNotifyNotifyAgain = 5,
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
    // Must not park in two slots.
    let s0_park = s0 == SlotState::LockedByPark || s0 == SlotState::TaskWritten;
    let s1_park = s1 == SlotState::LockedByPark || s1 == SlotState::TaskWritten;
    debug_assert!(!(s0_park && s1_park),
        "{:?} {:?}", s0, s1);

    // Notifier must not lock two slots, so at least one slot
    // is always available to park.
    let s0_lock = s0 == SlotState::LockedByNotify || s0 == SlotState::LockedByNotifyNotifyAgain;
    let s1_lock = s1 == SlotState::LockedByNotify || s1 == SlotState::LockedByNotifyNotifyAgain;
    debug_assert!(!(s0_lock && s1_lock),
        "{:?} {:?}", s0, s1);
}

#[inline(always)]
fn decode_state(v: usize) -> (SlotState, SlotState) {
    // slot 0 state is written in bits 0..8
    // slot 1 state is written in bits 8..16
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

#[inline(always)]
fn update_state(state: usize, slot: usize, new_value: SlotState) -> usize {
    let (s0, s1) = decode_state(state);
    if slot == 0 {
        encode_state(new_value, s1)
    } else {
        encode_state(s0, new_value)
    }
}


pub struct TaskCell {
    // Encoded pair `(SlotState, SlotState)`.
    state: AtomicUsize,
    // Two slots.
    // At least one slot is always available for park.
    tasks: [UnsafeCell<Option<Task>>; 2],
}

impl fmt::Debug for TaskCell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskCell")
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
        let mut state = self.state.load(Ordering::SeqCst);
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
                _ => {
                    // At least one slot must be available for `park`,
                    // that's invariant of `TaskCell`.
                    unreachable!()
                },
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
                    // so current task may skip notify.
                    return;
                }
                (SlotState::LockedByNotifyNotifyAgain, _) |
                (_, SlotState::LockedByNotifyNotifyAgain) => {
                    // Thread which started notifying will notify again.
                    return;
                }
                (SlotState::LockedByNotify, sb) |
                (sb, SlotState::LockedByNotify) => {
                    // Another task is doing notify now.
                    // Current thread needs to notify again (because another task
                    // maybe parked after previous notify started) a task which is probably
                    // written in another slot. However, one slot must be always available
                    // for park function, so current thread must not lock another slot,
                    // so instead of notifying task in another thread,
                    // current thread tells previous notifier to notify again.

                    // Slot current thread is working with.
                    let slot_0 = decode_state(state).0 == SlotState::LockedByNotify;
                    let slot = if slot_0 { 0 } else { 1 };

                    let notify = encode_state_flip(
                        SlotState::LockedByNotifyNotifyAgain, sb, slot == 1);

                    match self.state.compare_exchange(
                        state, notify, Ordering::SeqCst, Ordering::SeqCst)
                    {
                        Ok(_) => return,
                        Err(state) => state,
                    }
                }
                (SlotState::TaskWritten, sb) |
                (sb, SlotState::TaskWritten) => {
                    // Default notify operation: actual notify happens here.

                    // Slot current thread is working with.
                    let slot_0 = decode_state(state).0 == SlotState::TaskWritten;
                    let slot = if slot_0 { 0 } else { 1 };

                    // Lock the slot.
                    let locked = encode_state_flip(SlotState::LockedByNotify, sb, slot == 1);
                    match self.state.compare_exchange(
                        state, locked, Ordering::SeqCst, Ordering::SeqCst)
                    {
                        Ok(_) => {
                            unsafe { (*self.tasks[slot].get()).take() }.unwrap().notify();

                            let mut notify_again = false;

                            // Now unlock the slot.
                            let mut state = locked;
                            loop {
                                let unlocked = update_state(state, slot, SlotState::Empty);
                                state = match self.state.compare_exchange(
                                    state, unlocked, Ordering::SeqCst, Ordering::SeqCst)
                                {
                                    Ok(_) => {
                                        state = unlocked;
                                        break;
                                    },
                                    Err(state) => {
                                        // CAS could fail for two reasons:
                                        // 1. If another thread changed another slot
                                        // 2. If another thread called `notify`

                                        // If another thread called `notify`,
                                        // current thread needs to do the notify work again.
                                        if decode_state_flip(state, slot == 1).0
                                            == SlotState::LockedByNotifyNotifyAgain
                                        {
                                            notify_again = true;
                                        }
                                        state
                                    }
                                }
                            }

                            if !notify_again {
                                return;
                            }

                            state
                        }
                        Err(state) => state,
                    }
                }
            };
        }
    }
}
