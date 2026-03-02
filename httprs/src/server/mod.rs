use std::rc::Rc;

use crate::worker::{
    group::WorkerGroup, helper::ProcessManager, manager::WorkerManager, AnyWorker,
};

pub mod tcp;

pub struct ServerWorkerInfo {
    pub worker_count: u32,
    pub worker: Rc<dyn AnyWorker>,
}

pub struct ServerArgs {
    pub worker_infos: Vec<ServerWorkerInfo>,
    pub timeout_ms: u64,
}

pub struct Server {
    config: ServerArgs,
}

impl Server {
    pub fn new(config: ServerArgs) -> Self {
        Self { config }
    }

    pub fn open_server(&mut self) {
        let config = &self.config;

        let group: Vec<WorkerGroup> = config
            .worker_infos
            .iter()
            .map(|i| WorkerGroup::new(i.worker_count, i.worker.clone()))
            .collect();

        let manager = WorkerManager::new(group, ProcessManager {});
        let mut group_list = manager.start();

        manager.run(&mut group_list);
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    ParseFail(String),
    IoFail(String),
}
