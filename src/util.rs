use {Future, Task, Poll};

pub enum Collapsed<T: Future> {
    Start(T),
    Tail(Box<Future<Item=T::Item, Error=T::Error>>),
}

impl<T: Future> Collapsed<T> {
    pub fn poll(&mut self, task: &mut Task) -> Poll<T::Item, T::Error> {
        match *self {
            Collapsed::Start(ref mut a) => a.poll(task),
            Collapsed::Tail(ref mut a) => a.poll(task),
        }
    }

    pub fn schedule(&mut self, task: &mut Task) {
        match *self {
            Collapsed::Start(ref mut a) => a.schedule(task),
            Collapsed::Tail(ref mut a) => a.schedule(task),
        }
    }

    pub fn collapse(&mut self) {
        let a = match *self {
            Collapsed::Start(ref mut a) => {
                match a.tailcall() {
                    Some(a) => a,
                    None => return,
                }
            }
            Collapsed::Tail(ref mut a) => {
                if let Some(b) = a.tailcall() {
                    *a = b;
                }
                return
            }
        };
        *self = Collapsed::Tail(a);
    }
}
