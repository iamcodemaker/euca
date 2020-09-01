//! Abstraction of a wasm application.

use crate::app::dispatch::Dispatcher;
use crate::app::side_effect::{SideEffect, Commands};
use crate::app::detach::Detach;

use web_sys;
use wasm_bindgen::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::fmt;

/// A pending render.
pub type ScheduledRender<Command> = (Vec<Command>, i32, Closure<dyn FnMut(f64)>);

/// All of the functions one might perform on a wasm application.
pub trait Application<Message, Command> {
    /// Update the application with a message.
    fn update(&mut self, msg: Message) -> Commands<Command>;
    /// Tell the application to render itself.
    fn render(&mut self, app: &Dispatcher<Message, Command>) -> Vec<Command>;
    /// Process side effecting commands.
    fn process(&self, cmd: Command, app: &Dispatcher<Message, Command>);
    /// Get a reference to any pending rendering.
    fn get_scheduled_render(&mut self) -> &mut Option<ScheduledRender<Command>>;
    /// Store a reference to any pending rendering.
    fn set_scheduled_render(&mut self, handle: ScheduledRender<Command>);
    /// Store a listener that will be canceled when the app is detached.
    fn push_listener(&mut self, listener: (String, Closure<dyn FnMut(web_sys::Event)>));
    /// The first node of app.
    fn node(&self) -> Option<web_sys::Node>;
    /// Get all the top level nodes of node this app.
    fn nodes(&self) -> Vec<web_sys::Node>;
    /// Create the dom nodes for this app.
    fn create(&mut self, app: &Dispatcher<Message, Command>) -> Vec<web_sys::Node>;
    /// Detach the app from the dom.
    fn detach(&mut self, app: &Dispatcher<Message, Command>);
}

impl<Message, Command> Detach<Message> for Rc<RefCell<Box<dyn Application<Message, Command>>>>
where
    Message: fmt::Debug + Clone + PartialEq,
    Command: SideEffect<Message>,
{
    /// Detach the app from the dom.
    ///
    /// Any elements that were created will be destroyed and event handlers will be removed.
    fn detach(&self) {
        let mut app = self.borrow_mut();
        Application::detach(&mut **app, &self.into());
    }
}
