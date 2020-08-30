//! A self contained component in a euca app.

use std::rc::Rc;
use std::cell::RefCell;
use std::fmt;
use std::hash::Hash;
use crate::app::Dispatch;
use crate::app::Dispatcher;
use crate::app::Detach;
use crate::app::Application;
use crate::app::AppBuilder;
use crate::app::SideEffect;
use crate::app::side_effect;
use crate::app::{Update, Render};
use crate::vdom::DomIter;

/// A self containted component that can live inside another app.
pub trait Component<Message> {
    /// Dispatch a message to this component.
    fn dispatch(&self, message: Message);

    /// Detach the component from the dom.
    fn detach(&self);

    /// Get the first web_sys::Node of this component (if any)
    fn node(&self) -> Option<web_sys::Node>;

    /// Get the top level web_sys::Nodes of this component (if any)
    fn nodes(&self) -> Vec<web_sys::Node>;

    /// Get nodes waiting to attach to the parent.
    fn pending(&mut self) -> Vec<web_sys::Node>;
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
    #[must_use]
    pub fn map(mut self, f: fn(ParentMessage) -> Option<Message>) -> Self {
        self.map = f;
        self
    }

    /// A funciton to optionally map a command from the component to the parent.
    #[must_use]
    pub fn unmap(mut self, f: fn(Command) -> Option<ParentMessage>) -> Self {
        self.unmap = f;
        self
    }

    /// Create a component from the given app, and it's parent.
    #[must_use]
    pub fn create<ParentCommand, Model, DomTree, K>(self, model: Model, parent_app: Dispatcher<ParentMessage, ParentCommand>)
    -> Box<dyn Component<ParentMessage>>
    where
        ParentMessage: fmt::Debug + Clone + PartialEq + 'static,
        ParentCommand: SideEffect<ParentMessage> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
        Command: SideEffect<Message> + fmt::Debug + Clone + 'static,
        Model: Update<Message, Command> + Render<DomTree> + 'static,
        DomTree: DomIter<Message, Command, K> + 'static,
        K: Eq + Hash + 'static,
    {
        let ComponentBuilder {
            map,
            unmap,
        } = self;

        let processor = ComponentProcessor::new(parent_app, unmap);
        let (app, pending) = AppBuilder::default()
            .processor(processor)
            .create(model);

        Box::new(ComponentImpl {
            app: app,
            map: map,
            pending: pending,
        })
    }
}

struct ComponentProcessor<Message, Command, ParentMessage, ParentCommand> {
    parent: Dispatcher<ParentMessage, ParentCommand>,
    unmap: fn(Command) -> Option<ParentMessage>,
    message: std::marker::PhantomData<Message>,
}

impl<Message, Command, ParentMessage, ParentCommand>
ComponentProcessor<Message, Command, ParentMessage, ParentCommand>
{
    fn new(app: Dispatcher<ParentMessage, ParentCommand>, unmap: fn(Command) -> Option<ParentMessage>) -> Self {
        ComponentProcessor {
            parent: app,
            unmap: unmap,
            message: std::marker::PhantomData,
        }
    }
}

impl<Message, Command, ParentMessage, ParentCommand>
side_effect::Processor<Message, Command>
for ComponentProcessor<Message, Command, ParentMessage, ParentCommand>
where
    Command: SideEffect<Message> + Clone + 'static,
    ParentMessage: fmt::Debug + Clone + PartialEq + 'static,
    ParentCommand: SideEffect<ParentMessage> + 'static,
{
    fn process(&self, cmd: Command, app: &Dispatcher<Message, Command>) {
        cmd.clone().process(app);
        if let Some(cmd) = (self.unmap)(cmd) {
            Dispatch::dispatch(&self.parent, cmd);
        }
    }
}

/// A wasm application consisting of a model, a virtual dom representation, and the parent element
/// where this app lives in the dom.
struct ComponentImpl<Message, Command, ParentMessage> {
    app: Rc<RefCell<Box<dyn Application<Message, Command>>>>,
    map: fn(ParentMessage) -> Option<Message>,
    pending: Vec<web_sys::Node>,
}

impl<Message, Command, ParentMessage> Component<ParentMessage>
for ComponentImpl<Message, Command, ParentMessage>
where
    Message: fmt::Debug + Clone + PartialEq + 'static,
    Command: SideEffect<Message> + 'static,
    ParentMessage: fmt::Debug + Clone + PartialEq + 'static,
{
    fn dispatch(&self, msg: ParentMessage) {
        if let Some(msg) = (self.map)(msg) {
            Dispatch::dispatch(&self.app, msg);
        }
    }

    fn detach(&self) {
        Detach::detach(&self.app);
    }

    fn node(&self) -> Option<web_sys::Node> {
        Application::node(&**self.app.borrow())
    }

    fn nodes(&self) -> Vec<web_sys::Node> {
        Application::nodes(&**self.app.borrow())
    }

    fn pending(&mut self) -> Vec<web_sys::Node> {
        let mut pending = vec![];
        std::mem::swap(&mut pending, &mut self.pending);
        pending
    }
}
