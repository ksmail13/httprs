use std::rc::Rc;

use crate::worker::AnyWorker;

pub struct WorkerGroup {
    pub count: u32,
    /* It is occur dynamic dispatch, but it will be called one time after fork */
    pub worker: Rc<dyn AnyWorker>,
}

impl WorkerGroup {
    pub fn new(count: u32, worker: Rc<dyn AnyWorker>) -> Self {
        return Self {
            count: count,
            worker: worker,
        };
    }
}
