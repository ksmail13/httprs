use crate::worker::{AnyWorker, WorkerGroup, helper::ProcessManager, manager::WorkerManager};

pub mod acceptor;
pub mod tcp;

pub struct ServerWorkerInfo<'a> {
    pub worker_count: u32,
    pub worker: &'a (dyn AnyWorker + 'a),
}

pub struct ServerArgs<'a> {
    pub worker_infos: Vec<ServerWorkerInfo<'a>>,
    pub timeout_ms: u64,
}

pub struct Server<'a> {
    config: ServerArgs<'a>,
}

impl<'a> Server<'a> {
    pub fn new(config: ServerArgs<'a>) -> Self {
        Self { config }
    }

    pub fn open_server(&mut self) {
        let config = &self.config;

        let group: Vec<WorkerGroup> = config
            .worker_infos
            .iter()
            .map(|i| WorkerGroup::new(i.worker_count, i.worker))
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
