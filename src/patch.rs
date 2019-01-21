use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use crate::dom::EventHandler;
use crate::app::Dispatch;

pub enum Patch<'a, Message> {
    RemoveElement(Box<FnMut() -> web_sys::Element + 'a>),
    CreateElement { store: Box<FnMut(web_sys::Element) + 'a>, element: &'a str },
    CopyElement { store: Box<FnMut(web_sys::Element) + 'a>, take: Box<FnMut() -> web_sys::Element + 'a> },
    RemoveText(Box<FnMut() -> web_sys::Text + 'a>),
    ReplaceText { store: Box<FnMut(web_sys::Text) + 'a>, take: Box<FnMut() -> web_sys::Text + 'a>, text: &'a str },
    CreateText { store: Box<FnMut(web_sys::Text) + 'a>, text: &'a str },
    CopyText { store: Box<FnMut(web_sys::Text) + 'a>, take: Box<FnMut() -> web_sys::Text + 'a> },
    AddAttribute { name: &'a str, value: &'a str },
    RemoveAttribute(&'a str),
    AddListener { trigger: &'a str, handler: EventHandler<'a, Message>, store: Box<FnMut(Closure<FnMut(web_sys::Event)>) + 'a> },
    CopyListener { store: Box<FnMut(Closure<FnMut(web_sys::Event)>) + 'a>, take: Box<FnMut() -> Closure<FnMut(web_sys::Event)> + 'a> },
    RemoveListener { trigger: &'a str, take: Box<FnMut() -> Closure<FnMut(web_sys::Event)> + 'a> },
    Up,
}

impl<'a, Message> fmt::Debug for Patch<'a, Message> where
    Message: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Patch::RemoveElement(_) => write!(f, "RemoveElement(_)"),
            Patch::CreateElement { store: _, element: s } => write!(f, "CreateElement {{ store: _, element: {:?} }}", s),
            Patch::CopyElement { store: _, take: _ } => write!(f, "CopyElement {{ store: _, take: _ }}"),
            Patch::RemoveText(_) => write!(f, "RemoveText(_)"),
            Patch::ReplaceText { store: _, take: _, text: t }  => write!(f, "ReplaceText {{ store: _, take: _, text: {:?} }}", t),
            Patch::CreateText { store: _, text: t } => write!(f, "CreateText {{ store: _, text: {:?} }}", t),
            Patch::CopyText { store: _, take: _ } => write!(f, "CopyText {{ store: _, take: _ }}"),
            Patch::AddAttribute { name: n, value: v } => write!(f, "AddAttribute {{ name: {:?}, value: {:?} }}", n, v),
            Patch::RemoveAttribute(s) => write!(f, "RemoveAttribute({:?})", s),
            Patch::AddListener { trigger: t, handler: h, store: _ } => write!(f, "AddListener {{ trigger: {:?}, handler: {:?}, store: _ }}", t, h),
            Patch::CopyListener { store: _, take: _ } => write!(f, "CopyListener {{ store: _, take: _ }}"),
            Patch::RemoveListener { trigger: t, take: _ } => write!(f, "RemoveListener {{ trigger: {:?}), take: _ }}", t),
            Patch::Up => write!(f, "Up"),
        }
    }
}

#[derive(Default, Debug)]
pub struct PatchSet<'a, Message>(pub Vec<Patch<'a, Message>>);

impl<'a, Message> PatchSet<'a, Message> {
    pub fn new() -> Self {
        return PatchSet(Vec::new());
    }

    pub fn push(&mut self, patch: Patch<'a, Message>) {
        self.0.push(patch)
    }

    pub fn len(&self) -> usize {
        return self.0.len()
    }

    pub fn is_noop(&self) -> bool {
        use Patch::*;

        self.0.iter().all(|p| match p {
            // these patches just copy stuff into the new virtual dom tree, thus if we just keep
            // the old dom tree, the end result is the same
            CopyElement { .. } | CopyListener { .. }
            | CopyText { .. } | Up
            => true,
            // these patches change the dom
            RemoveElement(_) | CreateElement { .. }
            | RemoveListener { .. } | AddListener { .. }
            | RemoveAttribute(_) | AddAttribute { .. }
            | RemoveText(_) | CreateText { .. } | ReplaceText { .. }
            => false,
        })
    }

    pub fn apply<D>(self, parent: web_sys::Element, app: Rc<RefCell<D>>) where
        Message: 'static + Clone,
        EventHandler<'a, Message>: Clone,
        D: Dispatch<Message> + 'static,
    {
        let mut node_stack: Vec<web_sys::Node> = vec![parent.unchecked_into()];

        let document = web_sys::window().expect("expected window")
            .document().expect("expected document");

        for p in self.0.into_iter() {
            match p {
                Patch::RemoveElement(mut take) => {
                    node_stack.last()
                        .expect("no previous node")
                        .remove_child(&take())
                        .expect("failed to remove child node");
                }
                Patch::CreateElement { mut store, element } => {
                    let node = document.create_element(&element).expect("failed to create element");
                    store(node.clone());
                    node_stack.last()
                        .expect("no previous node")
                        .append_child(&node)
                        .expect("failed to append child node");
                    node_stack.push(node.into());
                }
                Patch::CopyElement { mut store, mut take } => {
                    let node = take();
                    store(node.clone());
                    node_stack.push(node.into());
                }
                Patch::RemoveText(mut take) => {
                    node_stack.last()
                        .expect("no previous node")
                        .remove_child(&take())
                        .expect("failed to remove child node");
                }
                Patch::ReplaceText { mut store, mut take, text } => {
                    let node = take();
                    node.set_data(&text);
                    store(node.clone());
                    node_stack.push(node.into());
                }
                Patch::CreateText { mut store, text } => {
                    let node = document.create_text_node(&text);
                    store(node.clone());
                    node_stack.last()
                        .expect("no previous node")
                        .append_child(&node)
                        .expect("failed to append child node");
                    node_stack.push(node.into());
                }
                Patch::CopyText { mut store, mut take } => {
                    let node = take();
                    store(node.clone());
                    node_stack.push(node.into());
                }
                Patch::AddAttribute { name, value } => {
                    node_stack.last()
                        .expect("no previous node")
                        .dyn_ref::<web_sys::Element>()
                        .expect("attributes can only be added to elements")
                        .set_attribute(name, value)
                        .expect("failed to set attribute");
                }
                Patch::RemoveAttribute(name) => {
                    node_stack.last()
                        .expect("no previous node")
                        .dyn_ref::<web_sys::Element>()
                        .expect("attributes can only be removed from elements")
                        .remove_attribute(name)
                        .expect("failed to remove attribute");
                }
                Patch::AddListener { trigger, handler, mut store } => {
                    let app = app.clone();
                    let closure = match handler {
                        EventHandler::Msg(msg) => {
                            let msg = msg.clone();
                            Closure::wrap(
                                Box::new(move |_| {
                                    D::dispatch(app.clone(), msg.clone())
                                }) as Box<FnMut(web_sys::Event)>
                            )
                        }
                        EventHandler::Fn(fun) => {
                            Closure::wrap(
                                Box::new(move |event| {
                                    D::dispatch(app.clone(), fun(event))
                                }) as Box<FnMut(web_sys::Event)>
                            )
                        }
                    };
                    let node = node_stack.last().expect("no previous node");
                    (node.as_ref() as &web_sys::EventTarget)
                        .add_event_listener_with_callback(&trigger, closure.as_ref().unchecked_ref())
                        .expect("failed to add event listener");
                    store(closure);
                }
                Patch::CopyListener { mut store, mut take } => {
                    store(take());
                }
                Patch::RemoveListener { trigger, mut take } => {
                    let node = node_stack.last().expect("no previous node");
                    (node.as_ref() as &web_sys::EventTarget)
                        .remove_event_listener_with_callback(&trigger, take().as_ref().unchecked_ref())
                        .expect("failed to remove event listener");
                }
                Patch::Up => {
                    node_stack.pop();
                }
            }
        }
    }
}

impl<'a, Message> From<Vec<Patch<'a, Message>>> for PatchSet<'a, Message> {
    fn from(v: Vec<Patch<'a, Message>>) -> Self {
        PatchSet(v)
    }
}

impl<'a, Message> IntoIterator for PatchSet<'a, Message> {
    type Item = Patch<'a, Message>;
    type IntoIter = ::std::vec::IntoIter<Patch<'a, Message>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use wasm_bindgen_test::*;
    use wasm_bindgen_test::wasm_bindgen_test_configure;
    wasm_bindgen_test_configure!(run_in_browser);

    enum Msg {}

    #[test]
    fn empty_patch_set_is_noop() {
        let patch_set: PatchSet<Msg> = vec![
        ].into();

        assert!(patch_set.is_noop());
    }

    #[wasm_bindgen_test]
    fn noop_patch_set_is_noop() {
        let patch_set: PatchSet<Msg> = vec![
            Patch::CopyElement {
                store: Box::new(|_|()),
                take: Box::new(|| {
                    web_sys::window().expect("expected window")
                        .document().expect("expected document")
                        .create_element("test").expect("expected element")
                }),
            },
            Patch::Up,
        ].into();

        assert!(patch_set.is_noop());
    }

    #[test]
    fn not_noop() {
        let patch_set: PatchSet<Msg> = vec![
            Patch::CreateElement {
                store: Box::new(|_|()),
                element: "",
            },
        ].into();

        assert!(!patch_set.is_noop());
    }
}
