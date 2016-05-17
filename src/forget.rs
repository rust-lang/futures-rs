use std::sync::Arc;

use Future;
use slot::Slot;

pub fn forget<T: Future>(mut t: T) {
    let slot = Arc::new(Slot::new(None));
    let slot2 = slot.clone();
    t.schedule(move |data| {
        drop((data, slot2));
    });
    slot.try_produce(t).ok().unwrap();
}
