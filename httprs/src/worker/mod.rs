mod error;
pub mod group;
pub mod helper;
pub mod manager;

pub trait Worker {
    fn init(&self);
    fn run(&self);
    fn cleanup(&self);
}
