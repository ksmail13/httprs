mod error;
pub mod group;
pub mod helper;
pub mod manager;

pub trait Worker {
    fn init(&mut self);
    fn run(&self);
    fn cleanup(&mut self);
}
