use futures_core::task::Waker;

// Sent to the consumer to wake up blocked producers
#[derive(Debug)]
pub(super) struct SenderTask {
    pub(super) task: Option<Waker>,
    pub(super) is_parked: bool,
}

impl SenderTask {
    pub(super) fn new() -> Self {
        SenderTask {
            task: None,
            is_parked: false,
        }
    }

    pub(super) fn notify(&mut self) {
        self.is_parked = false;

        if let Some(task) = self.task.take() {
            task.wake();
        }
    }
}

#[derive(Debug)]
pub(super) struct ReceiverTask {
    pub(super) unparked: bool,
    pub(super) task: Option<Waker>,
}
