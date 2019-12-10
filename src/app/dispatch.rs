//! Dispatch messages via a shared app handle.

use std::rc::Rc;
use std::cell::RefCell;

/// A shared app handle.
///
/// Since events need to be dispatched from event handlers in the browser, they need a way to relay
/// messages back to the app.
pub type Dispatcher<Message> =  Rc<RefCell<dyn Dispatch<Message>>>;

/// Dispatch a message from an event handler.
pub trait Dispatch<Message> {
    /// Dispatch the given message to the given app.
    fn dispatch(app: Rc<RefCell<Self>>, msg: Message) where Self: Sized;
}

/// Partially dispatch a message, returning any resulting Commands instead of executing them.
pub trait PartialDispatch<Message, Command> {
    /// Dispatch a message to the app but don't execute commands.
    fn update(app: Rc<RefCell<Self>>, msg: Message) -> Vec<Command> where Self: Sized;
}
