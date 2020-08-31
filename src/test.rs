//! Test utilties.

use crate::app::Application;
use crate::app::ScheduledRender;
use crate::app::Dispatcher;
use crate::app::Commands;
use crate::app::Update;

use wasm_bindgen::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;

/// Test message.
pub type Msg = ();
/// Test command.
pub type Cmd = ();
/// Test key.
pub type Key = ();

/// Test app.
pub struct App {
    messages: Rc<RefCell<Vec<Msg>>>,
    render: Option<ScheduledRender<Cmd>>,
}

impl App {
    /// Get a dispatcher for this test application.
    pub fn dispatcher() -> Dispatcher<Msg, Cmd> {
        Dispatcher::from(Rc::new(RefCell::new(Box::new(
            App {
                messages: Rc::new(RefCell::new(vec![])),
                render: None,
            }
        ) as Box<dyn Application<Msg, Cmd>>)))
    }

    /// Get a dispatcher that tracks messages dispatched to and pushes them to the given vec.
    pub fn dispatcher_with_vec(messages: Rc<RefCell<Vec<Msg>>>) -> Dispatcher<Msg, Cmd> {
        Dispatcher::from(Rc::new(RefCell::new(Box::new(
            App {
                messages: messages,
                render: None,
            }
        ) as Box<dyn Application<Msg, Cmd>>)))
    }
}

impl Application<Msg, Cmd> for App {
    fn update(&mut self, msg: Msg) -> Commands<Cmd> {
        self.messages.borrow_mut().push(msg);
        Commands::default()
    }
    fn render(&mut self, _app: &Dispatcher<Msg, Cmd>) -> Vec<Cmd> { vec![] }
    fn process(&self, _cmd: Cmd, _app: &Dispatcher<Msg, Cmd>) { }
    fn get_scheduled_render(&mut self) -> &mut Option<ScheduledRender<Cmd>> {
        &mut self.render
    }
    fn set_scheduled_render(&mut self, handle: ScheduledRender<Cmd>) {
        self.render = Some(handle);
    }
    fn push_listener(&mut self, _listener: (String, Closure<dyn FnMut(web_sys::Event)>)) { }
    fn node(&self) -> Option<web_sys::Node> { None }
    fn nodes(&self) -> Vec<web_sys::Node> { vec![] }
    fn create(&mut self, _app: &Dispatcher<Msg, Cmd>) -> Vec<web_sys::Node> { vec![] }
    fn detach(&mut self, _app: &Dispatcher<Msg, Cmd>) { }
}

/// Some helpers to make testing a model easier.
pub trait Model<Message, Command> {
    /// Update a model with the given message.
    ///
    /// This function is a helper function designed to make testing models simpler. Normally during
    /// an update to a model, the `Commands` structure must be passed in as an argument. This
    /// function automatically does that and returns the resulting `Commands` structure. It's only
    /// useful for unit testing.
    fn test_update(&mut self, msg: Message) -> Commands<Command>;
}

impl<Message, Command, M: Update<Message, Command>> Model<Message, Command> for M {
    fn test_update(&mut self, msg: Message) -> Commands<Command> {
        let mut cmds = Commands::default();
        Update::update(self, msg, &mut cmds);
        cmds
    }
}
