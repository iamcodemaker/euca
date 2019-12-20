//! A wasm app in the structure of [The Elm Architecture].
//!
//! The app is represented by a model which is the state of the app, a function that accepts user
//! defined messages and updates the model, and a function which renders the model into a virtual
//! dom representation.
//!
//! Because the update and render portions of the app are completely separated, it is trivial to
//! test these in isolation.
//!
//! [The Elm Architecture]: https://guide.elm-lang.org/architecture/

pub mod detach;
pub mod model;
pub mod dispatch;

pub use crate::app::detach::Detach;
pub use crate::app::model::{Update, Render};
pub use crate::app::dispatch::{PartialDispatch, Dispatch, Dispatcher};
pub use crate::app::dispatch::SideEffect;

use web_sys;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::rc::Rc;
use std::cell::RefCell;
use std::fmt;
use crate::diff;
use crate::vdom::DomIter;
use crate::vdom::Storage;
use crate::route::Route;
use crate::generic_helpers;

/// Struct used to configure and attach an application to the DOM.
pub struct AppBuilder<Message, Router: Route<Message>> {
    router: Option<Rc<Router>>,
    message: std::marker::PhantomData<Message>,
}

impl<Message> Default for AppBuilder<Message, generic_helpers::Router<Message>> {
    fn default() -> Self {
        AppBuilder {
            router: None,
            message: std::marker::PhantomData,
        }
    }
}

impl<Message, Router: Route<Message> + 'static> AppBuilder<Message, Router> {
    /// Handle popstate and hashchange events for this app.
    ///
    /// The router will need to implement the [`Route`] trait.
    ///
    /// [`Route`]: ../route/trait.Route.html
    pub fn router<R: Route<Message>>(self, router: R) -> AppBuilder<Message, R> {
        let AppBuilder {
            message,
            ..
        } = self;

        AppBuilder {
            message: message,
            router: Some(Rc::new(router)),
        }
    }

    /// Attach an app to the dom.
    ///
    /// The app will be attached at the given parent node and initialized with the given model.
    /// Event handlers will be registered as necessary.
    pub fn attach<Model, Command, DomTree>(self, parent: web_sys::Element, mut model: Model)
    -> Rc<RefCell<Box<dyn Application<Message, Command>>>>
    where
        Model: Update<Message, Command> + Render<DomTree> + 'static,
        DomTree: DomIter<Message, Command> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
        Command: SideEffect<Message> + 'static,
    {
        let mut commands = vec![];

        if let Some(ref router) = self.router {
            // initialize the model with the initial URL
            let url = web_sys::window()
                .expect("window")
                .document()
                .expect("document")
                .url()
                .expect("url");

            if let Some(msg) = router.route(&url) {
                model.update(msg, &mut commands);
            }
        }

        // attach the app to the dom
        let app_rc = App::attach(parent, model);

        if let Some(ref router) = self.router {
            let window = web_sys::window()
                .expect("couldn't get window handle");

            let document = window.document()
                .expect("couldn't get document handle");

            // register event handlers
            for event in ["popstate", "hashchange"].iter() {
                let app = Rc::clone(&app_rc);
                let document = document.clone();
                let router = router.clone();
                let closure = Closure::wrap(
                    Box::new(move |_event| {
                        let url = document.url()
                            .expect_throw("couldn't get document url");

                        if let Some(msg) = router.route(&url) {
                            Dispatch::dispatch(&app, msg);
                        }
                    }) as Box<dyn FnMut(web_sys::Event)>
                );

                window
                    .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
                    .expect("failed to add event listener");

                app_rc.borrow_mut().push_listener((event.to_string(), closure));
            }

            // execute side effects
            let dispatcher = Dispatcher::from(&app_rc);
            for cmd in commands {
                cmd.process(&dispatcher);
            }
        }

        app_rc
    }
}

/// All of the functions one might perform on a wasm application.
pub trait Application<Message, Command> {
    /// Update the application with a message.
    fn update(&mut self, msg: Message) -> Vec<Command>;
    /// Tell the application to render itself.
    fn render(&mut self, app: &Dispatcher<Message, Command>);
    /// Get a reference to any pending rendering.
    fn get_scheduled_render(&self) -> &Option<(i32, Closure<dyn FnMut(f64)>)>;
    /// Store a reference to any pending rendering.
    fn set_scheduled_render(&mut self, handle: (i32, Closure<dyn FnMut(f64)>));
    /// Store a listener that will be canceled when the app is detached.
    fn push_listener(&mut self, listener: (String, Closure<dyn FnMut(web_sys::Event)>));
    /// Attach the initial app to the dom.
    fn attach(&mut self, app: &Dispatcher<Message, Command>);
    /// Detach the app from the dom.
    fn detach(&mut self, app: &Dispatcher<Message, Command>);
}

impl<Model, DomTree, Message, Command> Application<Message, Command>
for App<Model, DomTree, Message, Command>
where
    Model: Update<Message, Command> + Render<DomTree> + 'static,
    Command: SideEffect<Message> + 'static,
    Message: fmt::Debug + Clone + PartialEq + 'static,
    DomTree: DomIter<Message, Command> + 'static,
{
    fn update(&mut self, msg: Message) -> Vec<Command> {
        // update the model
        let mut commands = vec![];
        self.model.update(msg, &mut commands);
        commands
    }

    fn get_scheduled_render(&self) -> &Option<(i32, Closure<dyn FnMut(f64)>)> {
        &self.animation_frame_handle
    }

    fn set_scheduled_render(&mut self, handle: (i32, Closure<dyn FnMut(f64)>)) {
        self.animation_frame_handle = Some(handle)
    }

    fn render(&mut self, app_rc: &Dispatcher<Message, Command>) {
        let App {
            ref parent,
            ref mut model,
            ref mut storage,
            ref dom,
            ..
        } = *self;

        // render a new dom from the updated model
        let new_dom = model.render();

        // push changes to the browser
        let old = dom.dom_iter();
        let new = new_dom.dom_iter();
        let patch_set = diff::diff(old, new, storage);
        self.storage = patch_set.apply(parent, app_rc);

        self.dom = new_dom;
        self.animation_frame_handle = None;

        // TODO: evaluate speedup or lack there of from using patch_set.is_noop() to check if we
        // actually need to apply this patch before applying the patch
    }

    fn push_listener(&mut self, listener: (String, Closure<dyn FnMut(web_sys::Event)>)) {
        self.listeners.push(listener);
    }

    fn detach(&mut self, app: &Dispatcher<Message, Command>) {
        use std::iter;

        let App {
            ref parent,
            ref mut storage,
            ref dom,
            ref mut listeners,
            ..
        } = *self;

        // remove listeners
        let window = web_sys::window()
            .expect("couldn't get window handle");

        for (event, listener) in listeners.drain(..) {
            window
                .remove_event_listener_with_callback(&event, listener.as_ref().unchecked_ref())
                .expect("failed to remove event listener");
        }

        // remove the current app from the browser's dom by diffing it with an empty virtual dom.
        let o = dom.dom_iter();
        let patch_set = diff::diff(o, iter::empty(), storage);
        self.storage = patch_set.apply(parent, app);
    }

    fn attach(&mut self, app: &Dispatcher<Message, Command>) {
        // render the initial app
        use std::iter;

        let App {
            ref parent,
            ref mut storage,
            ref dom,
            ..
        } = *self;

        let n = dom.dom_iter();
        let patch_set = diff::diff(iter::empty(), n, storage);

        self.storage = patch_set.apply(parent, app);
    }
}

/// A wasm application consisting of a model, a virtual dom representation, and the parent element
/// where this app lives in the dom.
struct App<Model, DomTree, Message, Command> {
    dom: DomTree,
    parent: web_sys::Element,
    model: Model,
    storage: Storage<Message>,
    listeners: Vec<(String, Closure<dyn FnMut(web_sys::Event)>)>,
    animation_frame_handle: Option<(i32, Closure<dyn FnMut(f64)>)>,
    command: std::marker::PhantomData<Command>,
}

impl<Message, Command> PartialDispatch<Message, Command> for Rc<RefCell<Box<dyn Application<Message, Command>>>>
where
    Command: SideEffect<Message> + 'static,
    Message: fmt::Debug + Clone + PartialEq + 'static,
{
    fn update(&self, msg: Message) -> Vec<Command> {
        // update the model
        let mut app = self.borrow_mut();
        let commands = Application::update(&mut **app, msg);

        // request an animation frame for rendering if we don't already have a request out
        if Application::get_scheduled_render(&**app).is_none() {
            let app_rc = Rc::clone(self);

            let window = web_sys::window()
                .expect_throw("couldn't get window handle");

            let closure = Closure::wrap(
                Box::new(move |_| {
                    let mut app = app_rc.borrow_mut();
                    Application::render(&mut **app, &Dispatcher::from(&app_rc));
                }) as Box<dyn FnMut(f64)>
            );

            let handle = window.request_animation_frame(closure.as_ref().unchecked_ref())
                .expect_throw("error with requestion_animation_frame");

            Application::set_scheduled_render(&mut **app, (handle, closure));
        }

        commands

        // TODO: evaluate speedup or lack there of from using patch_set.is_noop() to check if we
        // actually need to apply this patch before applying the patch
    }
}

impl<Model, DomTree, Message, Command> App<Model, DomTree, Message, Command> {
    /// Attach an app to the dom.
    ///
    /// The app will be attached at the given parent node and initialized with the given model.
    /// Event handlers will be registered as necessary.
    fn attach(parent: web_sys::Element, model: Model, )
    -> Rc<RefCell<Box<dyn Application<Message, Command>>>>
    where
        Model: Update<Message, Command> + Render<DomTree> + 'static,
        DomTree: DomIter<Message, Command> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
        Command: SideEffect<Message> + 'static,
    {
        // render our initial model
        let dom = model.render();
        let app = App {
            dom: dom,
            parent: parent.clone(),
            model: model,
            storage: vec![],
            listeners: vec![],
            animation_frame_handle: None,
            command: std::marker::PhantomData,
        };

        // we use a RefCell here because we need the dispatch callback to be able to mutate our
        // App. This should be safe because the browser should only ever dispatch events from a
        // single thread.
        let app_rc = Rc::new(RefCell::new(Box::new(app) as Box<dyn Application<Message, Command>>));

        // attach the initial app
        Application::attach(&mut **app_rc.borrow_mut(), &Dispatcher::from(&app_rc));

        app_rc
    }
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
