use std::rc::Rc;
use std::cell::RefCell;
use std::iter;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use euca::vdom::DomItem;
use euca::vdom::DomIter;
use euca::vdom::Storage;
use euca::patch::Patch;
use euca::patch::PatchSet;
use euca::app::Dispatch;
use euca::diff;

use wasm_bindgen_test::*;
use wasm_bindgen_test::wasm_bindgen_test_configure;
wasm_bindgen_test_configure!(run_in_browser);

fn e(name: &str) -> web_sys::Element {
    web_sys::window().expect("expected window")
        .document().expect("expected document")
        .create_element(name).expect("expected element")
}

fn c(elem: &web_sys::Element, trigger: &str) -> Closure<FnMut(web_sys::Event)> {
    let closure = Closure::wrap(
        Box::new(|_|()) as Box<FnMut(web_sys::Event)>
    );
    (elem.as_ref() as &web_sys::EventTarget)
        .add_event_listener_with_callback(trigger, closure.as_ref().unchecked_ref())
        .expect("failed to add event listener");
    closure
}

fn element_with_closure(name: &str, trigger: &str) -> (web_sys::Element, Closure<FnMut(web_sys::Event)>) {
    let elem = e(name);
    let closure = c(&elem, trigger);
    (elem, closure)
}

#[derive(PartialEq)]
struct Attr {
    name: &'static str,
    value: &'static str,
}

#[derive(PartialEq)]
enum EventHandler<Message> {
    Msg(Message),
    Map(fn(web_sys::Event) -> Message),
}

struct Event<Message> {
    trigger: String,
    handler: EventHandler<Message>,
    closure: Option<Closure<FnMut(web_sys::Event)>>,
}

enum Node {
    Elem { name: &'static str, node: Option<web_sys::Element> },
    Text { text: &'static str, node: Option<web_sys::Text> },
}

impl Node {
    fn elem(name: &'static str) -> Self {
        Node::Elem {
            name: name,
            node: None,
        }
    }

    fn text(text: &'static str) -> Self {
        Node::Text {
            text: text,
            node: None,
        }
    }

    fn elem_with_node(name: &'static str) -> Self {
        Node::Elem {
            name: name,
            node: Some(e(name)),
        }
    }
}

struct Dom<Message> {
    element: Node,
    attributes: Vec<Attr>,
    events: Vec<Event<Message>>,
    children: Vec<Dom<Message>>,
}

impl<Message: Clone> DomIter<Message> for Dom<Message> {
    fn dom_iter<'a>(&'a mut self) -> Box<Iterator<Item = DomItem<'a, Message>> + 'a>
    {
        // until generators are stable, this is the best we can do
        let iter = iter::once(&mut self.element)
            .map(|node| match node {
                Node::Elem { name, ref mut node } => {
                    DomItem::Element {
                        element: name,
                        node: match node {
                            Some(_) => Storage::Read(Box::new(move || node.take().unwrap())),
                            None => Storage::Write(Box::new(move |n| *node = Some(n))),
                        },
                    }
                }
                Node::Text { text, ref mut node } => {
                    DomItem::Text {
                        text: text,
                        node: match node {
                            Some(_) => Storage::Read(Box::new(move || node.take().unwrap())),
                            None => Storage::Write(Box::new(move |n| *node = Some(n))),
                        },
                    }
                }
            })
        .chain(self.attributes.iter()
            .map(|attr| DomItem::Attr {
                name: attr.name,
                value: attr.value
            })
        )
        .chain(self.events.iter_mut()
            .map(|Event { trigger, handler, closure }|
                 DomItem::Event {
                     trigger: trigger,
                     handler: match handler {
                         EventHandler::Msg(m) => euca::vdom::EventHandler::Msg(m),
                         EventHandler::Map(f) => euca::vdom::EventHandler::Fn(*f),
                     },
                     closure: match closure {
                         Some(_) => Storage::Read(Box::new(move || closure.take().unwrap())),
                         None => Storage::Write(Box::new(move |c| *closure = Some(c))),
                     },
                 }
             )
        )
        .chain(self.children.iter_mut()
           .flat_map(|c| c.dom_iter())
        )
        .chain(iter::once(DomItem::Up));

        Box::new(iter)
    }
}

macro_rules! compare {
    ( $patch_set:ident, [ $( $x:expr ,)* ] ) => {
        compare!($patch_set, [ $($x),* ]);
    };
    ( $patch_set:ident, [ $( $x:expr),* ] ) => {
        let cmp: PatchSet<Msg> = vec!($($x),*).into();

        assert_eq!($patch_set.len(), cmp.len(), "lengths don't match\n  left: {:?}\n right: {:?}", $patch_set, cmp);

        for (l, r) in $patch_set.into_iter().zip(cmp) {
            match (l, r) {
                (Patch::CreateElement { store: _, element: e1 }, Patch::CreateElement { store: _, element: e2 }) => {
                    assert_eq!(e1, e2, "unexpected CreateElement");
                }
                (Patch::CopyElement { store: _, take: _ }, Patch::CopyElement { store: _, take: _ }) => {}
                (Patch::AddAttribute { name: n1, value: v1 }, Patch::AddAttribute { name: n2, value: v2 }) => {
                    assert_eq!(n1, n2, "attribute names don't match");
                    assert_eq!(v1, v2, "attribute values don't match");
                }
                (Patch::ReplaceText { store: _, take: _, text: t1 }, Patch::ReplaceText { store: _, take: _, text: t2 }) => {
                    assert_eq!(t1, t2, "unexpected ReplaceText");
                }
                (Patch::CreateText { store: _, text: t1 }, Patch::CreateText { store: _, text: t2 }) => {
                    assert_eq!(t1, t2, "unexpected CreateText");
                }
                (Patch::CopyText { store: _, take: _ }, Patch::CopyText { store: _, take: _ }) => {}
                (Patch::RemoveAttribute(a1), Patch::RemoveAttribute(a2)) => {
                    assert_eq!(a1, a2, "attribute names don't match");
                }
                (Patch::AddListener { trigger: t1, handler: h1, store: _ }, Patch::AddListener { trigger: t2, handler: h2, store: _ }) => {
                    assert_eq!(t1, t2, "trigger names don't match");
                    assert_eq!(h1, h2, "handlers don't match");
                }
                (Patch::RemoveListener { trigger: t1, take: _ }, Patch::RemoveListener { trigger: t2, take: _ }) => {
                    assert_eq!(t1, t2, "trigger names don't match");
                }
                (Patch::RemoveElement(_), Patch::RemoveElement(_)) => {}
                (Patch::RemoveText(_), Patch::RemoveText(_)) => {}
                (Patch::Up, Patch::Up) => {}
                (i1, i2) => panic!("patch items don't match: {:?} {:?}", i1, i2),
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq)]
struct Msg {}

#[test]
fn basic_diff() {
    let old = iter::empty();

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("span"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let o = old.into_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n);

    compare!(
        patch_set,
        [
            Patch::CreateElement { store: Box::new(|_|()), element: "span".into() },
            Patch::Up,
        ]
    );
}

#[test]
fn diff_add_text() {
    let old = iter::empty();

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: Node::text("text"),
                attributes: vec![],
                events: vec![],
                children: vec![],
            },
        ],
    };

    let o = old.into_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n);

    compare!(
        patch_set,
        [
            Patch::CreateElement { store: Box::new(|_|()), element: "div".into() },
            Patch::CreateText { store: Box::new(|_|()), text: "text".into() },
            Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn new_child_nodes() {
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec![],
    };

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: Node::elem("b"),
                attributes: vec![
                    Attr { name: "class", value: "item" },
                    Attr { name: "id", value: "id1" },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
            },
            Dom {
                element: Node::elem("i"),
                attributes: vec![
                    Attr { name: "class", value: "item" },
                    Attr { name: "id", value: "id2" },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
            },
        ],
    };

    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n);

    compare!(
        patch_set,
        [
            Patch::CopyElement { store: Box::new(|_|()), take: Box::new(|| e("div")) },
            Patch::CreateElement { store: Box::new(|_|()), element: "b".into() },
            Patch::AddAttribute { name: "class", value: "item" },
            Patch::AddAttribute { name: "id", value: "id1" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&Msg {}), store: Box::new(|_|()) },
            Patch::Up,
            Patch::CreateElement { store: Box::new(|_|()), element: "i".into() },
            Patch::AddAttribute { name: "class", value: "item" },
            Patch::AddAttribute { name: "id", value: "id2" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&Msg {}), store: Box::new(|_|()) },
            Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn from_empty() {
    let mut new: Dom<Msg> = Dom {
        element: Node::elem("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: Node::elem("b"),
                attributes: vec![
                    Attr { name: "class", value: "item" },
                    Attr { name: "id", value: "id1" },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
            },
            Dom {
                element: Node::elem("i"),
                attributes: vec![
                    Attr { name: "class", value: "item" },
                    Attr { name: "id", value: "id2" },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
            },
        ],
    };

    let n = new.dom_iter();
    let patch_set = diff::diff(iter::empty(), n);

    compare!(
        patch_set,
        [
            Patch::CreateElement { store: Box::new(|_|()), element: "div" },
            Patch::CreateElement { store: Box::new(|_|()), element: "b" },
            Patch::AddAttribute { name: "class", value: "item" },
            Patch::AddAttribute { name: "id", value: "id1" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&Msg {}), store: Box::new(|_|()) },
            Patch::Up,
            Patch::CreateElement { store: Box::new(|_|()), element: "i" },
            Patch::AddAttribute { name: "class", value: "item" },
            Patch::AddAttribute { name: "id", value: "id2" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&Msg {}), store: Box::new(|_|()) },
            Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn to_empty() {
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: Node::elem_with_node("b"),
                attributes: vec![
                    Attr { name: "class", value: "item" },
                    Attr { name: "id", value: "id1" },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
            },
            Dom {
                element: Node::elem_with_node("i"),
                attributes: vec![
                    Attr { name: "class", value: "item" },
                    Attr { name: "id", value: "id2" },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
            },
        ],
    };

    let o = old.dom_iter();
    let patch_set = diff::diff(o, iter::empty());

    compare!(
        patch_set,
        [
            Patch::RemoveElement(Box::new(|| e("div"))),
        ]
    );
}

#[wasm_bindgen_test]
fn no_difference() {
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n);

    compare!(
        patch_set,
        [
            Patch::CopyElement { store: Box::new(|_|()), take: Box::new(|| e("div")) },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn basic_diff_with_element() {
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("span"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n);

    compare!(
        patch_set,
        [
            Patch::RemoveElement(Box::new(|| e("div"))),
            Patch::CreateElement { store: Box::new(|_|()), element: "span".into() },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn old_child_nodes_with_element() {
    let (elem, closure) = element_with_closure("b", "onclick");
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: Node::Elem { name: "b", node: Some(elem) },
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id".into() },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: Some(closure) },
                ],
                children: vec![],
            },
            Dom {
                element: Node::elem_with_node("i"),
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id".into() },
                ],
                events: vec![],
                children: vec![],
            },
        ],
    };

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n);

    compare!(
        patch_set,
        [
            Patch::CopyElement { store: Box::new(|_|()), take: Box::new(|| e("div")) },
            Patch::RemoveElement(Box::new(|| e("b"))),
            Patch::RemoveElement(Box::new(|| e("i"))),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_old_child_nodes_with_element() {
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("span"),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: Node::elem_with_node("b"),
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id".into() },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
            },
            Dom {
                element: Node::elem_with_node("i"),
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id".into() },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
            },
        ],
    };

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n);

    compare!(
        patch_set,
        [
            Patch::RemoveElement(Box::new(|| e("span"))),
            Patch::CreateElement { store: Box::new(|_|()), element: "div".into() },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn null_patch_with_element() {
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n);

    let parent = e("div");
    struct App {};
    impl Dispatch<Msg> for App {
        fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
    }

    let app = Rc::new(RefCell::new(App {}));
    patch_set.apply(parent.clone(), app.clone());

    match new.element {
        Node::Elem { name: _, node: Some(_) } => {},
        _ => panic!("expected node to be copied"),
    }
}

#[wasm_bindgen_test]
fn basic_patch_with_element() {
    let gen1 = iter::empty();

    let mut gen2: Dom<Msg> = Dom {
        element: Node::elem("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let mut gen3: Dom<Msg> = Dom {
        element: Node::elem("span"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let parent = e("div");
    struct App {};
    impl Dispatch<Msg> for App {
        fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
    }

    let app = Rc::new(RefCell::new(App {}));

    // first gen create element
    let o = gen1.into_iter();
    let n = gen2.dom_iter();
    let patch_set = diff::diff(o, n);
    patch_set.apply(parent.clone(), app.clone());

    match gen2.element {
        Node::Elem { name: _, node: Some(_) } => {},
        _ => panic!("expected node to be created"),
    }

    // second gen remove and replace element
    let o = gen2.dom_iter();
    let n = gen3.dom_iter();
    let patch_set = diff::diff(o, n);
    patch_set.apply(parent.clone(), app.clone());

    match gen3.element {
        Node::Elem { name: _, node: Some(_) } => {},
        _ => panic!("expected node to be created"),
    };
}

#[wasm_bindgen_test]
fn basic_event_test() {
    let gen1 = iter::empty();

    let mut gen2: Dom<Msg> = Dom {
        element: Node::elem("button"),
        attributes: vec!(),
        events: vec![
            Event { trigger: "click".into(), handler: EventHandler::Map(|_| Msg {}), closure: None },
        ],
        children: vec!(),
    };

    let parent = e("div");
    struct App(i32);
    impl Dispatch<Msg> for App {
        fn dispatch(app: Rc<RefCell<Self>>, _: Msg) {
            let mut app = app.borrow_mut();
            app.0 += 1;
        }
    }

    let app = Rc::new(RefCell::new(App(0)));

    let o = gen1.into_iter();
    let n = gen2.dom_iter();
    let patch_set = diff::diff(o, n);
    patch_set.apply(parent.clone(), app.clone());

    match gen2.element {
        Node::Elem { name: _, node: Some(node) } => {
            node.dyn_ref::<web_sys::HtmlElement>()
                .expect("expected html element")
                .click();
        },
        _ => panic!("expected node to be created"),
    };

    assert_eq!(app.borrow().0, 1);
}

#[wasm_bindgen_test]
fn listener_copy() {
    let gen1 = iter::empty();

    let mut gen2: Dom<Msg> = Dom {
        element: Node::elem("button"),
        attributes: vec!(),
        events: vec![
            Event { trigger: "click".into(), handler: EventHandler::Msg(Msg {}), closure: None },
        ],
        children: vec!(),
    };

    let parent = e("div");
    struct App(i32);
    impl Dispatch<Msg> for App {
        fn dispatch(app: Rc<RefCell<Self>>, _: Msg) {
            let mut app = app.borrow_mut();
            app.0 += 1;
        }
    }

    let app = Rc::new(RefCell::new(App(0)));

    let o = gen1.into_iter();
    let n = gen2.dom_iter();
    let patch_set = diff::diff(o, n);
    patch_set.apply(parent.clone(), app.clone());

    match gen2.element {
        Node::Elem { name: _, node: Some(ref node) } => {
            node.dyn_ref::<web_sys::HtmlElement>()
                .expect("expected html element")
                .click();
        },
        _ => panic!("expected node to be created"),
    }

    let mut gen3: Dom<Msg> = Dom {
        element: Node::elem("button"),
        attributes: vec!(),
        events: vec![
            Event { trigger: "click".into(), handler: EventHandler::Msg(Msg {}), closure: None },
        ],
        children: vec!(),
    };

    let o = gen2.dom_iter();
    let n = gen3.dom_iter();
    let patch_set = diff::diff(o, n);
    patch_set.apply(parent.clone(), app.clone());

    match gen3.element {
        Node::Elem { name: _, node: Some(node) } => {
            node.dyn_ref::<web_sys::HtmlElement>()
                .expect("expected html element")
                .click();
        },
        _ => panic!("expected node to be created"),
    }

    assert_eq!(app.borrow().0, 2);
}
