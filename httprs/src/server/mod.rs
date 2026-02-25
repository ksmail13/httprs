use std::{collections::HashMap, net::TcpListener, rc::Rc};

use nix::sys::{
    socket::{
        setsockopt,
        sockopt::{ReceiveTimeout, ReuseAddr, ReusePort},
    },
    time::TimeVal,
};

use crate::{
    process::Process,
    worker::{group::WorkerGroup, helper::ProcessManager, manager::WorkerManager},
};

mod worker;

pub struct WorkerInfo {
    pub host: String,
    pub port: u16,
    pub worker: u32,
    pub process: Rc<dyn Process>,
}

pub struct ServerArgs {
    pub worker_infos: Vec<WorkerInfo>,
    pub timeout_ms: u64,
}

pub struct Server {
    config: ServerArgs,
}

impl Server {
    pub fn new(config: ServerArgs) -> Self {
        return Self { config };
    }

    pub fn open_server(&mut self) {
        let config = &self.config;
        let listeners = Rc::new(
            config
                .worker_infos
                .iter()
                .flat_map(|i| {
                    let ip = format!("{}:{}", i.host, i.port);
                    let listener = TcpListener::bind(ip.clone()).unwrap();

                    if let Err(e) = setsockopt(&listener, ReuseAddr, &true) {
                        log::error!(target: "TcpWorker::run", "set ReuseAddr failed: [{e}]");
                        return Err(e);
                    }

                    if let Err(e) = setsockopt(&listener, ReusePort, &true) {
                        log::error!(target: "TcpWorker::run", "set ReusePort failed: [{e}]");
                        return Err(e);
                    }

                    // accept 과정에서 시그널 받으면 취소되도록 설정
                    if let Err(e) = setsockopt(&listener, ReceiveTimeout, &TimeVal::new(2, 0)) {
                        log::error!(target: "TcpWorker::run", "[] set ReceiveTimeout failed: [{e}]");
                        return Err(e);
                    }

                    return Ok((ip, listener));
                })
                .fold(HashMap::new(), |m, pair| {
                    let mut new_m = HashMap::from(m);
                    new_m.insert(pair.0, pair.1);
                    return new_m;
                }),
        );

        let group: Vec<WorkerGroup> = config
            .worker_infos
            .iter()
            .map(|i| {
                return WorkerGroup::new(
                    i.worker,
                    Rc::new(worker::TcpWorker {
                        timeout_ms: config.timeout_ms,
                        listeners: listeners.clone(),
                        host: format!("{}:{}", i.host, i.port),
                        tcp_process: i.process.clone(),
                    }),
                );
            })
            .collect();

        let manager = WorkerManager::new(group, ProcessManager {});
        let mut group_list = manager.start();

        manager.run(&mut group_list);
    }
}
