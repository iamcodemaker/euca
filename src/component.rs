//! A self contained component in a euca app.

use std::rc::Rc;
use std::cell::RefCell;
use crate::app::PartialDispatch;
use crate::app::Dispatch;
use crate::app::Detach;

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
    pub fn map(&mut self, f: fn(ParentMessage) -> Option<Message>) -> &mut Self {
        self.map = f;
        self
    }

    /// A funciton to optionally map a command from the component to the parent.
    pub fn unmap(&mut self, f: fn(Command) -> Option<ParentMessage>) -> &mut Self {
        self.unmap = f;
        self
    }

    /// Create a component from the given app, and it's parent.
    pub fn build<App, Parent>(self, app: Rc<RefCell<App>>, parent: Rc<RefCell<Parent>>)
    -> Box<dyn Component<ParentMessage>>
    where
        ParentMessage: 'static,
        Message: 'static,
        Command: 'static,
        Rc<RefCell<App>>: PartialDispatch<Message, Command> + Detach<Message> + 'static,
        Rc<RefCell<Parent>>: Dispatch<ParentMessage> + 'static,
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
struct ComponentImpl<Message, Command, ParentMessage, App, Parent> {
    app: Rc<RefCell<App>>,
    parent: Rc<RefCell<Parent>>,
    map: fn(ParentMessage) -> Option<Message>,
    unmap: fn(Command) -> Option<ParentMessage>,
}

impl<Message, Command, ParentMessage, App, Parent> Component<ParentMessage> for ComponentImpl<Message, Command, ParentMessage, App, Parent>
where
    Rc<RefCell<App>>: PartialDispatch<Message, Command> + Detach<Message>,
    Rc<RefCell<Parent>>: Dispatch<ParentMessage>,
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
