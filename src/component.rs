//! A self contained component in a euca app.

use std::rc::Rc;
use std::cell::RefCell;
use std::fmt;
use crate::app::PartialDispatch;
use crate::app::Dispatch;
use crate::app::Dispatcher;
use crate::app::Detach;
use crate::app::Application;
use crate::app::SideEffect;

/// A self containted component that can live inside another app.
pub trait Component<Message> {
    /// Dispatch a message to this component.
    fn update(&self, message: Message);

    /// Detach the component from the dom.
    fn detach(&self);
}

/// A builder for constructing a self contained component app that lives inside of another app.
pub struct ComponentBuilder<Message, Command, ParentMessage> {
    map: fn(ParentMessage) -> Option<Message>,
    unmap: fn(Command) -> Option<ParentMessage>,
}

impl<Message, Command, ParentMessage> Default for ComponentBuilder<Message, Command, ParentMessage>
{
    fn default() -> Self {
        ComponentBuilder {
            map: |_| None,
            unmap: |_| None,
        }
    }
}

impl<Message, Command, ParentMessage> ComponentBuilder<Message, Command, ParentMessage> {
    /// A function to optionally map a message from the parent to the component.
    pub fn map(mut self, f: fn(ParentMessage) -> Option<Message>) -> Self {
        self.map = f;
        self
    }

    /// A funciton to optionally map a command from the component to the parent.
    pub fn unmap(mut self, f: fn(Command) -> Option<ParentMessage>) -> Self {
        self.unmap = f;
        self
    }

    /// Create a component from the given app, and it's parent.
    pub fn build<ParentCommand>(self, app: Rc<RefCell<Box<dyn Application<Message, Command>>>>, parent: Dispatcher<ParentMessage, ParentCommand>)
    -> Box<dyn Component<ParentMessage>>
    where
        ParentMessage: fmt::Debug + Clone + PartialEq + 'static,
        ParentCommand: SideEffect<ParentMessage> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
        Command: SideEffect<Message> + 'static,
    {
        let ComponentBuilder {
            map,
            unmap,
        } = self;

        Box::new(ComponentImpl {
            app: app,
            parent: parent,
            map: map,
            unmap: unmap,
        })
    }
}

/// A wasm application consisting of a model, a virtual dom representation, and the parent element
/// where this app lives in the dom.
struct ComponentImpl<Message, Command, ParentMessage, ParentCommand> {
    app: Rc<RefCell<Box<dyn Application<Message, Command>>>>,
    parent: Dispatcher<ParentMessage, ParentCommand>,
    map: fn(ParentMessage) -> Option<Message>,
    unmap: fn(Command) -> Option<ParentMessage>,
}

impl<Message, Command, ParentMessage, ParentCommand> Component<ParentMessage>
for ComponentImpl<Message, Command, ParentMessage, ParentCommand>
where
    Message: fmt::Debug + Clone + PartialEq + 'static,
    Command: SideEffect<Message> + 'static,
    ParentMessage: fmt::Debug + Clone + PartialEq + 'static,
    ParentCommand: SideEffect<ParentMessage> + 'static,
{
    fn update(&self, msg: ParentMessage) {
        if let Some(msg) = (self.map)(msg) {
            let commands = PartialDispatch::update(&self.app, msg);
            for cmd in commands {
                // XXX execute command?
                if let Some(cmd) = (self.unmap)(cmd) {
                    Dispatch::dispatch(&self.parent, cmd);
                }
            }
        }
    }

    fn detach(&self) {
        Detach::detach(&self.app);
    }
}
