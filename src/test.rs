//! Test utilties.

use crate::app::Application;
use crate::app::Dispatcher;
use crate::app::SideEffect;
use crate::app::Commands;

use wasm_bindgen::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;

/// Test message.
#[derive(Clone, Debug, PartialEq)]
pub struct Msg {}
/// Test command.
#[derive(Debug)]
pub struct Cmd {}

impl SideEffect<Msg> for Cmd {
    fn process(self, _dispatcher: &Dispatcher<Msg, Self>) { }
}

/// Test app.
pub struct App {
    messages: Rc<RefCell<Vec<Msg>>>,
    render: Option<(i32, Closure<dyn FnMut(f64)>)>,
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
    fn render(&mut self, _app: &Dispatcher<Msg, Cmd>) { }
    fn process(&self, _cmd: Cmd, _app: &Dispatcher<Msg, Cmd>) { }
    fn get_scheduled_render(&self) -> &Option<(i32, Closure<dyn FnMut(f64)>)> {
        &self.render
    }
    fn set_scheduled_render(&mut self, handle: (i32, Closure<dyn FnMut(f64)>)) {
        self.render = Some(handle);
    }
    fn push_listener(&mut self, _listener: (String, Closure<dyn FnMut(web_sys::Event)>)) { }
    fn attach(&mut self, _app: &Dispatcher<Msg, Cmd>) { }
    fn detach(&mut self, _app: &Dispatcher<Msg, Cmd>) { }
}
