//! Dispatch messages via a shared app handle.

use std::rc::Rc;
use std::cell::RefCell;
use std::fmt;
use crate::app::{Application, SideEffect};

/// A shared app handle.
///
/// Since events need to be dispatched from event handlers in the browser, they need a way to relay
/// messages back to the app.
pub struct Dispatcher<Message, Command> {
    app: Rc<RefCell<Box<dyn Application<Message, Command>>>>,
}

//impl<Message, Command> Dispatcher<Message, Command> { }

impl<Message, Command> Clone for Dispatcher<Message, Command> {
    fn clone(&self) -> Self {
        Dispatcher {
            app: Rc::clone(&self.app),
        }
    }
}

impl<Message, Command> From<Rc<RefCell<Box<dyn Application<Message, Command>>>>> for Dispatcher<Message, Command> {
    fn from(app: Rc<RefCell<Box<dyn Application<Message, Command>>>>) -> Self {
        Dispatcher {
            app: app,
        }
    }
}

impl<Message, Command> From<&Rc<RefCell<Box<dyn Application<Message, Command>>>>> for Dispatcher<Message, Command> {
    fn from(app: &Rc<RefCell<Box<dyn Application<Message, Command>>>>) -> Self {
        Dispatcher {
            app: Rc::clone(app),
        }
    }
}

/// Dispatch a message from an event handler.
pub trait Dispatch<Message> {
    /// Dispatch the given message to the given app.
    fn dispatch(&self, msg: Message);
}

impl<Message, Command> Dispatch<Message> for Dispatcher<Message, Command>
where
    Command: SideEffect<Message> + 'static,
    Message: fmt::Debug + Clone + PartialEq + 'static,
{
    fn dispatch(&self, msg: Message) {
        Dispatch::dispatch(&self.app, msg);
    }
}
