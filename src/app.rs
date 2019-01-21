use web_sys;
use std::rc::Rc;
use std::cell::RefCell;
use std::fmt;
use crate::diff;
use crate::dom::DomIter;

pub trait Update<Message> {
    //fn update(&mut self) -> Command
    fn update(&mut self, msg: Message);
}

pub trait Render<DomTree> {
    fn render(&self) -> DomTree;
}

pub trait Dispatch<Message> {
    fn dispatch(app: Rc<RefCell<Self>>, msg: Message);
}

pub struct App<Model, DomTree> {
    dom: DomTree,
    parent: web_sys::Element,
    model: Model,
}

impl<Message, Model, DomTree> Dispatch<Message> for App<Model, DomTree> where
    Message: fmt::Debug + Clone + PartialEq + 'static,
    Model: Update<Message> + Render<DomTree> + 'static,
    DomTree: DomIter<Message> + 'static,
{
    fn dispatch(app_rc: Rc<RefCell<Self>>, msg: Message) {
        let mut app = app_rc.borrow_mut();
        let parent = app.parent.clone();

        // update the model
        app.model.update(msg);

        // render a new dom from the updated model
        let mut new_dom = app.model.render();

        // push changes to the browser
        let old = app.dom.dom_iter();
        let new = new_dom.dom_iter();
        let patch_set = diff::diff(old, new);
        patch_set.apply(parent, app_rc.clone());

        app.dom = new_dom;
    }
}

impl<Model, DomTree> App<Model, DomTree> {
    pub fn attach<Message>(parent: web_sys::Element, model: Model) where
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
        }));

        // render the initial app
        use std::iter;

        let mut app = app_rc.borrow_mut();

        let n = app.dom.dom_iter();
        let patch_set = diff::diff(iter::empty(), n);
        patch_set.apply(parent, app_rc.clone());
    }

    pub fn detach<Message>(app_rc: Rc<RefCell<App<Model, DomTree>>>) where
        Model: Update<Message> + Render<DomTree> + 'static,
        DomTree: DomIter<Message> + 'static,
        Message: fmt::Debug + Clone + PartialEq + 'static,
    {
        use std::iter;

        let mut app = app_rc.borrow_mut();
        let parent = app.parent.clone();

        // remove the current app from the browser's dom
        let o = app.dom.dom_iter();
        let patch_set = diff::diff(o, iter::empty());
        patch_set.apply(parent, app_rc.clone());
    }
}
