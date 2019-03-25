use std::rc::Rc;
use std::cell::RefCell;
use std::iter;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use euca::vdom::WebItem;
use euca::vdom::Storage;
use euca::vdom::DomItem;
use euca::vdom::DomIter;
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
}

enum Node {
    Elem { name: &'static str },
    Text { text: &'static str },
}

impl Node {
    fn elem(name: &'static str) -> Self {
        Node::Elem {
            name: name,
        }
    }

    fn text(text: &'static str) -> Self {
        Node::Text {
            text: text,
        }
    }

    fn elem_with_node(name: &'static str) -> Self {
        Node::Elem {
            name: name,
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
                Node::Elem { name } => {
                    DomItem::Element {
                        element: name,
                    }
                }
                Node::Text { text } => {
                    DomItem::Text {
                        text: text,
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
            .map(|Event { trigger, handler }|
                 DomItem::Event {
                     trigger: trigger,
                     handler: match handler {
                         EventHandler::Msg(m) => euca::vdom::EventHandler::Msg(m),
                         EventHandler::Map(f) => euca::vdom::EventHandler::Fn(*f),
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

fn gen_storage<'a, Message, Iter>(iter: Iter) -> Storage where
    Message: 'a,
    Iter: Iterator<Item = DomItem<'a, Message>>,
{
    iter
        .filter(|i| {
            match i {
                DomItem::Element { .. } | DomItem::Text { .. } | DomItem::Event { .. } => true,
                DomItem::Attr { .. } | DomItem::Up => false,
            }
        })
        .map(|i| {
            match i {
                DomItem::Element { element } => WebItem::Element(e(element)),
                DomItem::Text { text } => WebItem::Text(
                    web_sys::window().expect("expected window")
                        .document().expect("expected document")
                        .create_text_node(text)
                ),
                DomItem::Event { .. } => WebItem::Closure(
                    Closure::wrap(
                        Box::new(|_|()) as Box<FnMut(web_sys::Event)>
                    )
                ),
                DomItem::Attr { .. } | DomItem::Up => {
                    unreachable!("attribute and up nodes should have been filtered out")
                },
            }
        })
        .collect()
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
                (Patch::CreateElement { element: e1 }, Patch::CreateElement { element: e2 }) => {
                    assert_eq!(e1, e2, "unexpected CreateElement");
                }
                (Patch::CopyElement { take: _ }, Patch::CopyElement { take: _ }) => {}
                (Patch::SetAttribute { name: n1, value: v1 }, Patch::SetAttribute { name: n2, value: v2 }) => {
                    assert_eq!(n1, n2, "attribute names don't match");
                    assert_eq!(v1, v2, "attribute values don't match");
                }
                (Patch::ReplaceText { take: _, text: t1 }, Patch::ReplaceText { take: _, text: t2 }) => {
                    assert_eq!(t1, t2, "unexpected ReplaceText");
                }
                (Patch::CreateText { text: t1 }, Patch::CreateText { text: t2 }) => {
                    assert_eq!(t1, t2, "unexpected CreateText");
                }
                (Patch::CopyText { take: _ }, Patch::CopyText { take: _ }) => {}
                (Patch::RemoveAttribute(a1), Patch::RemoveAttribute(a2)) => {
                    assert_eq!(a1, a2, "attribute names don't match");
                }
                (Patch::AddListener { trigger: t1, handler: h1 }, Patch::AddListener { trigger: t2, handler: h2 }) => {
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
    let mut storage = vec![];

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("span"),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
    };

    let o = old.into_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CreateElement { element: "span".into() },
            Patch::Up,
        ]
    );
}

#[test]
fn diff_add_text() {
    let old = iter::empty();
    let mut storage = vec![];

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
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CreateElement { element: "div".into() },
            Patch::CreateText { text: "text".into() },
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
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
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
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
                ],
                children: vec![],
            },
        ],
    };

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement { take: Box::new(|| e("div")) },
            Patch::CreateElement { element: "b".into() },
            Patch::SetAttribute { name: "class", value: "item" },
            Patch::SetAttribute { name: "id", value: "id1" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&Msg {}) },
            Patch::Up,
            Patch::CreateElement { element: "i".into() },
            Patch::SetAttribute { name: "class", value: "item" },
            Patch::SetAttribute { name: "id", value: "id2" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&Msg {}) },
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
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
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
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
                ],
                children: vec![],
            },
        ],
    };

    let n = new.dom_iter();
    let mut storage = vec![];
    let patch_set = diff::diff(iter::empty(), n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CreateElement { element: "div" },
            Patch::CreateElement { element: "b" },
            Patch::SetAttribute { name: "class", value: "item" },
            Patch::SetAttribute { name: "id", value: "id1" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&Msg {}) },
            Patch::Up,
            Patch::CreateElement { element: "i" },
            Patch::SetAttribute { name: "class", value: "item" },
            Patch::SetAttribute { name: "id", value: "id2" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&Msg {}) },
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
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
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
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
                ],
                children: vec![],
            },
        ],
    };

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let patch_set = diff::diff(o, iter::empty(), &mut storage);

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

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement { take: Box::new(|| e("div")) },
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

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::RemoveElement(Box::new(|| e("div"))),
            Patch::CreateElement { element: "span".into() },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_attributes() {
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("div"),
        attributes: vec![
            Attr { name: "name", value: "value" },
        ],
        events: vec!(),
        children: vec!(),
    };

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("div"),
        attributes: vec![
            Attr { name: "name", value: "new value" },
        ],
        events: vec!(),
        children: vec!(),
    };

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement { take: Box::new(|| e("div")) },
            Patch::SetAttribute { name: "name", value: "new value" },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_checked() {
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("input"),
        attributes: vec![
            Attr { name: "checked", value: "false" },
        ],
        events: vec!(),
        children: vec!(),
    };

    let mut new: Dom<Msg> = Dom {
        element: Node::elem("input"),
        attributes: vec![
            Attr { name: "checked", value: "false" },
        ],
        events: vec!(),
        children: vec!(),
    };

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement { take: Box::new(|| e("input")) },
            Patch::SetAttribute { name: "checked", value: "false" },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn old_child_nodes_with_element() {
    let mut old: Dom<Msg> = Dom {
        element: Node::elem_with_node("div"),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: Node::Elem { name: "b" },
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id".into() },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
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

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement { take: Box::new(|| e("div")) },
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
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
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
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
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

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::RemoveElement(Box::new(|| e("span"))),
            Patch::CreateElement { element: "div".into() },
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

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    let parent = e("div");
    struct App {};
    impl Dispatch<Msg> for App {
        fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
    }

    let app = Rc::new(RefCell::new(App {}));
    storage = patch_set.apply(parent.clone(), app.clone());

    match storage[0] {
        WebItem::Element(_) => {}
        _ => panic!("expected node to be created"),
    }
}

#[wasm_bindgen_test]
fn basic_patch_with_element() {
    let gen1 = iter::empty();
    let mut storage = vec![];

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
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(parent.clone(), app.clone());

    match storage[0] {
        WebItem::Element(_) => {}
        _ => panic!("expected node to be created"),
    }

    // second gen remove and replace element
    let o = gen2.dom_iter();
    let n = gen3.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(parent.clone(), app.clone());

    match storage[0] {
        WebItem::Element(_) => {}
        _ => panic!("expected node to be created"),
    }
}

#[wasm_bindgen_test]
fn basic_event_test() {
    let gen1 = iter::empty();
    let mut storage = vec![];

    let mut gen2: Dom<Msg> = Dom {
        element: Node::elem("button"),
        attributes: vec!(),
        events: vec![
            Event { trigger: "click".into(), handler: EventHandler::Map(|_| Msg {}) },
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
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(parent.clone(), app.clone());

    match storage[0] {
        WebItem::Element(ref node) => {
            node.dyn_ref::<web_sys::HtmlElement>()
                .expect("expected html element")
                .click();
        },
        _ => panic!("expected node to be created"),
    }

    assert_eq!(app.borrow().0, 1);
}

#[wasm_bindgen_test]
fn listener_copy() {
    let gen1 = iter::empty();
    let mut storage = vec![];

    let mut gen2: Dom<Msg> = Dom {
        element: Node::elem("button"),
        attributes: vec!(),
        events: vec![
            Event { trigger: "click".into(), handler: EventHandler::Msg(Msg {}) },
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
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(parent.clone(), app.clone());

    match storage[0] {
        WebItem::Element(ref node) => {
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
            Event { trigger: "click".into(), handler: EventHandler::Msg(Msg {}) },
        ],
        children: vec!(),
    };

    let o = gen2.dom_iter();
    let n = gen3.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(parent.clone(), app.clone());

    match storage[0] {
        WebItem::Element(ref node) => {
            node.dyn_ref::<web_sys::HtmlElement>()
                .expect("expected html element")
                .click();
        },
        _ => panic!("expected node to be created"),
    }

    assert_eq!(app.borrow().0, 2);
}
