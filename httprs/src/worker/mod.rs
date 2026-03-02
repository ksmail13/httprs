use std::any::Any;

mod error;
pub mod group;
pub mod helper;
pub mod manager;

pub trait Worker {
    type Context;

    fn init(&self) -> Self::Context;
    fn run(&self, context: &mut Self::Context);
    fn cleanup(&self, context: &mut Self::Context);
}

/**
 * Wrapper for Worker trait to use dynamic dispatch
 * Hide Worker::Context from outside
 */
pub trait AnyWorker {
    fn init(&self) -> Box<dyn Any>;
    fn run(&self, context: &mut Box<dyn Any>);
    fn cleanup(&self, context: &mut Box<dyn Any>);
}

impl<T: Worker> AnyWorker for T
where
    T::Context: 'static,
{
    fn init(&self) -> Box<dyn Any> {
        Box::new(self.init())
    }

    fn run(&self, context: &mut Box<dyn Any>) {
        self.run(context.downcast_mut().unwrap())
    }

    fn cleanup(&self, context: &mut Box<dyn Any>) {
        self.cleanup(context.downcast_mut().unwrap())
    }
}
