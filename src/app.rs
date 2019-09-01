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

/// A side effect producing command.
#[derive(PartialEq)]
pub struct Command<Message> {
    msg: Message,
    cmd: fn(Message, Rc<RefCell<dyn Dispatch<Message>>>),
}

impl<Message> Command<Message> {
    /// Create a new command.
    pub fn new(msg: Message, cmd: fn(Message, Rc<RefCell<dyn Dispatch<Message>>>)) -> Self {
        Command {
            msg: msg,
            cmd: cmd,
        }
    }

    /// Execute this command.
    pub fn exec(self, app: Rc<RefCell<dyn Dispatch<Message>>>) {
        (self.cmd)(self.msg, app);
    }
}

/// A list of side effect producing commands.
pub type Commands<Message> = Vec<Command<Message>>;

/// Implemented on a model, used to process a message that updates the model.
pub trait Update<Message> {
    /// Update the model using the given message.
    fn update(&mut self, msg: Message, commands: &mut Commands<Message>);
}

/// Implemented on a model, used to render (or view) the model as a virtual dom.
pub trait Render<DomTree> {
    /// Render the model as a virtual dom.
    fn render(&self) -> DomTree;
}

/// Dispatch a message from an event handler.
pub trait Dispatch<Message> {
    /// Dispatch the given message to the given app.
    fn dispatch(app: Rc<RefCell<Self>>, msg: Message) where Self: Sized;
}

/// Struct used to configure and attach an application to the DOM.
#[derive(Default)]
pub struct AppBuilder {}

impl AppBuilder {
    /// Handle popstate and hashchange events for this app.
    ///
    /// The model will need to implement the [`Route`] trait.
    ///
    /// [`Route`]: ../route/trait.Route.html
    pub fn with_routing(self) -> AppWithRouting {
        AppWithRouting(self)
    }

    /// Attach an app to the dom.
    ///
    /// The app will be attached at the given parent node and initialized with the given model.
    /// Event handlers will be registered as necessary.
    pub fn attach<Message, Model, DomTree>(self, parent: web_sys::Element, model: Model)
    -> Rc<RefCell<App<Model, DomTree>>>
    where
        Model: Update<Message> + Render<DomTree> + 'static,
        DomTree: DomIter<Message> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
    {
        // attach the app to the dom
        App::attach(parent, model)
    }
}

/// Struct used to configure and attach an application (with routing support) to the DOM.
#[derive(Default)]
pub struct AppWithRouting(AppBuilder);

impl AppWithRouting {
    /// Attach an app to the dom.
    ///
    /// The app will be attached at the given parent node and initialized with the given model.
    /// Event handlers will be registered as necessary.
    pub fn attach<Message, Model, DomTree>(self, parent: web_sys::Element, mut model: Model)
    -> Rc<RefCell<App<Model, DomTree>>>
    where
        Model: Update<Message> + Render<DomTree> + Route<Message> + 'static,
        DomTree: DomIter<Message> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
    {
        let mut commands = vec![];

        // initialize the model with the initial URL
        let url = web_sys::window()
            .expect("window")
            .document()
            .expect("document")
            .url()
            .expect("url");

        if let Some(msg) = Model::route(&url) {
            model.update(msg, &mut commands);
        }

        // attach the app to the dom
        let app_rc = self.0.attach(parent, model);

        let window = web_sys::window()
            .expect("couldn't get window handle");

        let document = window.document()
            .expect("couldn't get document handle");

        // register event handlers
        for event in ["popstate", "hashchange"].iter() {
            let app = app_rc.clone();
            let document = document.clone();
            let closure = Closure::wrap(
                Box::new(move |_event| {
                    let url = document.url()
                        .expect_throw("couldn't get document url");

                    if let Some(msg) = Model::route(&url) {
                        App::dispatch(app.clone(), msg)
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
            cmd.exec(app_rc.clone());
        }

        app_rc
    }
}

/// A wasm application consisting of a model, a virtual dom representation, and the parent element
/// where this app lives in the dom.
pub struct App<Model, DomTree> {
    dom: DomTree,
    parent: web_sys::Element,
    model: Model,
    storage: Storage,
    listeners: Vec<(String, Closure<dyn FnMut(web_sys::Event)>)>,
}

impl<Message, Model, DomTree> Dispatch<Message> for App<Model, DomTree> where
    Message: fmt::Debug + Clone + PartialEq + 'static,
    Model: Update<Message> + Render<DomTree> + 'static,
    DomTree: DomIter<Message> + 'static,
{
    fn dispatch(app_rc: Rc<RefCell<Self>>, msg: Message) {
        let mut app = app_rc.borrow_mut();
        let App {
            ref parent,
            ref mut model,
            ref mut storage,
            ref dom,
            ..
        } = *app;

        // update the model
        let mut commands = vec![];
        model.update(msg, &mut commands);

        // execute side effects
        for cmd in commands {
            cmd.exec(app_rc.clone());
        }

        // render a new dom from the updated model
        let new_dom = model.render();

        // push changes to the browser
        let old = dom.dom_iter();
        let new = new_dom.dom_iter();
        let patch_set = diff::diff(old, new, storage);
        app.storage = patch_set.apply(parent.clone(), app_rc.clone());

        app.dom = new_dom;

        // TODO: evaluate speedup or lack there of from using patch_set.is_noop() to check if we
        // actually need to apply this patch before applying the patch
    }
}

impl<Model, DomTree> App<Model, DomTree> {
    /// Attach an app to the dom.
    ///
    /// The app will be attached at the given parent node and initialized with the given model.
    /// Event handlers will be registered as necessary.
    fn attach<Message>(parent: web_sys::Element, model: Model)
    -> Rc<RefCell<App<Model, DomTree>>>
    where
        Model: Update<Message> + Render<DomTree> + 'static,
        DomTree: DomIter<Message> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
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
        app.storage = patch_set.apply(parent, app_rc.clone());

        app_rc.clone()
    }

    /// Detach the app from the dom.
    ///
    /// Any elements that were created will be destroyed and event handlers will be removed.
    pub fn detach<Message>(app_rc: Rc<RefCell<App<Model, DomTree>>>) where
        Model: Update<Message> + Render<DomTree> + 'static,
        DomTree: DomIter<Message> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
    {
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
        app.storage = patch_set.apply(parent.clone(), app_rc.clone());
    }
}
