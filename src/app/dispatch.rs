//! Dispatch messages via a shared app handle.

use web_sys;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use std::rc::Rc;
use std::cell::RefCell;
use std::fmt;
use crate::app::Application;
use crate::app::side_effect::{SideEffect, Commands};

/// A shared app handle.
///
/// Since events need to be dispatched from event handlers in the browser, they need a way to relay
/// messages back to the app.
pub struct Dispatcher<Message, Command> {
    app: Rc<RefCell<Box<dyn Application<Message, Command>>>>,
    pending: Rc<RefCell<Vec<Message>>>,
}

impl<Message, Command> Clone for Dispatcher<Message, Command> {
    fn clone(&self) -> Self {
        Dispatcher {
            app: Rc::clone(&self.app),
            pending: Rc::clone(&self.pending),
        }
    }
}

impl<Message, Command> From<Rc<RefCell<Box<dyn Application<Message, Command>>>>> for Dispatcher<Message, Command> {
    fn from(app: Rc<RefCell<Box<dyn Application<Message, Command>>>>) -> Self {
        Dispatcher {
            app: app,
            pending: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl<Message, Command> From<&Rc<RefCell<Box<dyn Application<Message, Command>>>>> for Dispatcher<Message, Command> {
    fn from(app: &Rc<RefCell<Box<dyn Application<Message, Command>>>>) -> Self {
        Dispatcher {
            app: Rc::clone(app),
            pending: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl<Message, Command> Dispatcher<Message, Command>
where
    Command: SideEffect<Message> + 'static,
    Message: fmt::Debug + Clone + PartialEq + 'static,
{
    /// Dispatch a message to the associated app.
    pub fn dispatch(&self, msg: Message) {
        // queue the message
        self.pending.borrow_mut().push(msg);

        // try to borrow the app
        let mut app = match self.app.try_borrow_mut() {
            Ok(app) => app,
            // already borrowed, the current borrower will process the queue
            Err(_) => return,
        };

        // now process queued messages
        loop {
            // grab the first pending message (if any)
            let msg = match self.pending.borrow_mut().pop() {
                Some(msg) => msg,
                None => break,
            };

            let commands = Application::update(&mut **app, msg);

            let Commands {
                immediate,
                post_render,
            } = commands;

            // request an animation frame for rendering if we don't already have a request out
            if let Some((ref mut cmds, _, _)) = Application::get_scheduled_render(&mut **app) {
                cmds.extend(post_render);
            }
            else {
                let dispatcher = self.clone();

                let window = web_sys::window()
                    .expect_throw("couldn't get window handle");

                let closure = Closure::wrap(
                    Box::new(move |_| {
                        let mut app = dispatcher.app.borrow_mut();
                        let commands = Application::render(&mut **app, &dispatcher);
                        for cmd in commands {
                            Application::process(&**app, cmd, &dispatcher);
                        }
                    }) as Box<dyn FnMut(f64)>
                );

                let handle = window.request_animation_frame(closure.as_ref().unchecked_ref())
                    .expect_throw("error with requestion_animation_frame");

                Application::set_scheduled_render(&mut **app, (post_render, handle, closure));
            }

            // execute side effects
            for cmd in immediate {
                Application::process(&**app, cmd, &self);
            }
        }
    }
}
