use std::rc::Rc;

use crate::worker::Worker;

pub struct WorkerGroup {
    pub count: u32,
    /* It is occur dynamic dispatch, but it will be called one time after fork */
    pub worker: Rc<dyn Worker>,
}

impl WorkerGroup {
    pub fn new(count: u32, worker: Rc<dyn Worker>) -> Self {
        return Self {
            count: count,
            worker: worker,
        };
    }
}
