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

pub use crate::app::detach::Detach;
pub use crate::app::model::{Update, Render};

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

/// A shared app handle.
///
/// Since events need to be dispatched from event handlers in the browser, they will need a way to
/// relay messages back to the app.
pub type Dispatcher<Message> =  Rc<RefCell<dyn Dispatch<Message>>>;

/// Processor for side-effecting commands.
pub trait SideEffect<Message> {
    /// Process a side-effecting command.
    fn process(self, dispatcher: Dispatcher<Message>);
}

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
    -> Rc<RefCell<App<Model, DomTree, Command>>>
    where
        Model: Update<Message, Command> + Render<DomTree> + 'static,
        DomTree: DomIter<Message> + 'static,
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
                            App::dispatch(Rc::clone(&app), msg)
                        }
                    }) as Box<dyn FnMut(web_sys::Event)>
                );

                window
                    .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
                    .expect("failed to add event listener");

                app_rc.borrow_mut().listeners.push((event.to_string(), closure));
            }

            // execute side effects
            for cmd in commands {
                cmd.process(Rc::clone(&app_rc) as Dispatcher<Message>);
            }
        }

        app_rc
    }
}

/// A wasm application consisting of a model, a virtual dom representation, and the parent element
/// where this app lives in the dom.
pub struct App<Model, DomTree, Command> {
    dom: DomTree,
    parent: web_sys::Element,
    model: Model,
    storage: Storage,
    listeners: Vec<(String, Closure<dyn FnMut(web_sys::Event)>)>,
    animation_frame_handle: Option<(i32, Closure<dyn FnMut(f64)>)>,
    command: std::marker::PhantomData<Command>,
}

impl<Message, Command, Model, DomTree> PartialDispatch<Message, Command> for App<Model, DomTree, Command>
where
    Model: Update<Message, Command> + Render<DomTree> + 'static,
    Command: SideEffect<Message> + 'static,
    Message: fmt::Debug + Clone + PartialEq + 'static,
    DomTree: DomIter<Message> + 'static,
{
    fn update(app_rc: Rc<RefCell<Self>>, msg: Message) -> Vec<Command> {
        let mut app = app_rc.borrow_mut();

        // update the model
        let mut commands = vec![];
        app.model.update(msg, &mut commands);

        // request an animation frame for rendering if we don't already have a request out
        if app.animation_frame_handle.is_none() {
            let app_rc = Rc::clone(&app_rc);

            let window = web_sys::window()
                .expect_throw("couldn't get window handle");

            let closure = Closure::wrap(
                Box::new(move |_| {
                    let mut app = app_rc.borrow_mut();
                    let App {
                        ref parent,
                        ref mut model,
                        ref mut storage,
                        ref dom,
                        ..
                    } = *app;

                    // render a new dom from the updated model
                    let new_dom = model.render();

                    // push changes to the browser
                    let old = dom.dom_iter();
                    let new = new_dom.dom_iter();
                    let patch_set = diff::diff(old, new, storage);
                    app.storage = patch_set.apply(parent.clone(), Rc::clone(&app_rc));

                    app.dom = new_dom;
                    app.animation_frame_handle = None;

                }) as Box<dyn FnMut(f64)>
            );

            let handle = window.request_animation_frame(closure.as_ref().unchecked_ref())
                .expect_throw("error with requestion_animation_frame");

            app.animation_frame_handle = Some((handle, closure));
        }

        commands

        // TODO: evaluate speedup or lack there of from using patch_set.is_noop() to check if we
        // actually need to apply this patch before applying the patch
    }
}

impl<Message, Command, Model, DomTree> Dispatch<Message> for App<Model, DomTree, Command>
where
    Model: Update<Message, Command> + Render<DomTree> + 'static,
    Command: SideEffect<Message> + 'static,
    Message: fmt::Debug + Clone + PartialEq + 'static,
    DomTree: DomIter<Message> + 'static,
{
    fn dispatch(app_rc: Rc<RefCell<Self>>, msg: Message) {
        let commands = PartialDispatch::update(Rc::clone(&app_rc), msg);

        // execute side effects
        for cmd in commands {
            cmd.process(Rc::clone(&app_rc) as Dispatcher<Message>);
        }
    }
}

impl<Model, DomTree, Command> App<Model, DomTree, Command> {
    /// Attach an app to the dom.
    ///
    /// The app will be attached at the given parent node and initialized with the given model.
    /// Event handlers will be registered as necessary.
    fn attach<Message>(parent: web_sys::Element, model: Model, )
    -> Rc<RefCell<App<Model, DomTree, Command>>>
    where
        Model: Update<Message, Command> + Render<DomTree> + 'static,
        DomTree: DomIter<Message> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
        Command: SideEffect<Message> + 'static,
    {
        // render our initial model
        let dom = model.render();

        // we use a RefCell here because we need the dispatch callback to be able to mutate our
        // App. This should be safe because the browser should only ever dispatch events from a
        // single thread.
        let app_rc: Rc<RefCell<_>> = Rc::new(RefCell::new(App {
            dom: dom,
            parent: parent.clone(),
            model: model,
            storage: vec![],
            listeners: vec![],
            animation_frame_handle: None,
            command: std::marker::PhantomData,
        }));

        // render the initial app
        use std::iter;

        let mut app = app_rc.borrow_mut();
        let App {
            ref mut storage,
            ref dom,
            ..
        } = *app;

        let n = dom.dom_iter();
        let patch_set = diff::diff(iter::empty(), n, storage);
        app.storage = patch_set.apply(parent, Rc::clone(&app_rc));

        Rc::clone(&app_rc)
    }
}

impl<Model, DomTree, Message, Command> Detach<Message> for App<Model, DomTree, Command>
where
    Model: Update<Message, Command> + Render<DomTree> + 'static,
    DomTree: DomIter<Message> + 'static,
    Message: fmt::Debug + Clone + PartialEq + 'static,
    Command: SideEffect<Message> + 'static,
{
    /// Detach the app from the dom.
    ///
    /// Any elements that were created will be destroyed and event handlers will be removed.
    fn detach(app_rc: Rc<RefCell<Self>>) {
        use std::iter;

        let mut app = app_rc.borrow_mut();
        let App {
            ref parent,
            ref mut storage,
            ref dom,
            ref mut listeners,
            ..
        } = *app;

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
        app.storage = patch_set.apply(parent.clone(), Rc::clone(&app_rc));
    }
}
