use std::rc::Rc;
use std::cell::RefCell;
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
    name: String,
    value: String,
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

struct Text {
    text: String,
    node: Option<web_sys::Text>,
}

struct Dom<Message> {
    element: String,
    attributes: Vec<Attr>,
    events: Vec<Event<Message>>,
    children: Vec<Dom<Message>>,
    text: Option<Text>,
    node: Option<web_sys::Element>,
}

impl<Message: Clone> DomIter<Message> for Dom<Message> {
    fn dom_iter<'a>(&'a mut self) -> Box<Iterator<Item = DomItem<'a, Message>> + 'a>
    {
        use std::iter;

        // until generators are stable, this is the best we can do
        let iter = iter::once((&mut self.node, &self.element))
            .map(|(node, element)| DomItem::Element {
                element: element,
                node: match node {
                    Some(_) => Storage::Read(Box::new(move || node.take().unwrap())),
                    None => Storage::Write(Box::new(move |n| *node = Some(n))),
                },
            })
        .chain(self.attributes.iter()
            .map(|attr| DomItem::Attr {
                name: &attr.name,
                value: &attr.value
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
        .chain(self.text.iter_mut()
           .flat_map(|Text { text, node }|
               vec![
                   DomItem::Text {
                       text: text,
                       node: match node {
                           Some(_) => Storage::Read(Box::new(move || node.take().unwrap())),
                           None => Storage::Write(Box::new(move |n| *node = Some(n))),
                       },
                   },
                   // this is necessary because text nodes can have events associated with them
                   DomItem::Up,
               ]
           )
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
    use std::iter;

    let old = iter::empty();

    let mut new: Dom<Msg> = Dom {
        element: "span".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: None,
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
    use std::iter;

    let old = iter::empty();

    let mut new: Dom<Msg> = Dom {
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: Some(Text {
            text: "text".to_owned(),
            node: None,
        }),
        node: None,
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
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec![],
        text: None,
        node: Some(e("div")),
    };

    let mut new: Dom<Msg> = Dom {
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: "b".into(),
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id1".into() },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
                text: None,
                node: None,
            },
            Dom {
                element: "i".into(),
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id2".into() },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
                text: None,
                node: None,
            },
        ],
        text: None,
        node: None,
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
fn no_difference() {
    let mut old: Dom<Msg> = Dom {
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: Some(e("div")),
    };

    let mut new: Dom<Msg> = Dom {
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: None,
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
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: Some(e("div")),
    };

    let mut new: Dom<Msg> = Dom {
        element: "span".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: None,
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
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: "b".into(),
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id".into() },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: Some(closure) },
                ],
                children: vec![],
                text: None,
                node: Some(elem),
            },
            Dom {
                element: "i".into(),
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id".into() },
                ],
                events: vec![],
                children: vec![],
                text: None,
                node: Some(e("i")),
            },
        ],
        text: None,
        node: Some(e("div")),
    };

    let mut new: Dom<Msg> = Dom {
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: None,
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
        element: "span".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec![
            Dom {
                element: "b".into(),
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id".into() },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
                text: None,
                node: Some(e("b")),
            },
            Dom {
                element: "i".into(),
                attributes: vec![
                    Attr { name: "class".into(), value: "item".into() },
                    Attr { name: "id".into(), value: "id".into() },
                ],
                events: vec![
                    Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}), closure: None },
                ],
                children: vec![],
                text: None,
                node: Some(e("i")),
            },
        ],
        text: None,
        node: Some(e("span")),
    };

    let mut new: Dom<Msg> = Dom {
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: None,
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
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: Some(e("div")),
    };

    let mut new: Dom<Msg> = Dom {
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: None,
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

    assert!(new.node.is_some(), "expected node to be copied");
}

#[wasm_bindgen_test]
fn basic_patch_with_element() {
    use std::iter;
    let gen1 = iter::empty();

    let mut gen2: Dom<Msg> = Dom {
        element: "div".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: None,
    };

    let mut gen3: Dom<Msg> = Dom {
        element: "span".into(),
        attributes: vec!(),
        events: vec!(),
        children: vec!(),
        text: None,
        node: None,
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

    assert!(gen2.node.is_some(), "expected node to be created");

    // second gen remove and replace element
    let o = gen2.dom_iter();
    let n = gen3.dom_iter();
    let patch_set = diff::diff(o, n);
    patch_set.apply(parent.clone(), app.clone());

    assert!(gen3.node.is_some(), "expected node to be created");
}

#[wasm_bindgen_test]
fn basic_event_test() {
    use std::cell::RefCell;
    use std::iter;

    let gen1 = iter::empty();

    let mut gen2: Dom<Msg> = Dom {
        element: "button".into(),
        attributes: vec!(),
        events: vec![
            Event { trigger: "click".into(), handler: EventHandler::Msg(Msg {}), closure: None },
        ],
        children: vec!(),
        text: None,
        node: None,
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

    gen2.node
        .expect("expected node to be created")
        .dyn_ref::<web_sys::HtmlElement>()
        .expect("expected html element")
        .click();

    assert_eq!(app.borrow().0, 1);
}

#[wasm_bindgen_test]
fn listener_copy() {
    use std::cell::RefCell;
    use std::iter;

    let gen1 = iter::empty();

    let mut gen2: Dom<Msg> = Dom {
        element: "button".into(),
        attributes: vec!(),
        events: vec![
            Event { trigger: "click".into(), handler: EventHandler::Msg(Msg {}), closure: None },
        ],
        children: vec!(),
        text: None,
        node: None,
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

    let node = gen2.node
        .expect("expected node to be created");
    gen2.node = Some(node.clone());

    node.dyn_ref::<web_sys::HtmlElement>()
        .expect("expected html element")
        .click();

    let mut gen3: Dom<Msg> = Dom {
        element: "button".into(),
        attributes: vec!(),
        events: vec![
            Event { trigger: "click".into(), handler: EventHandler::Msg(Msg {}), closure: None },
        ],
        children: vec!(),
        text: None,
        node: None,
    };

    let o = gen2.dom_iter();
    let n = gen3.dom_iter();
    let patch_set = diff::diff(o, n);
    patch_set.apply(parent.clone(), app.clone());

    let node = gen3.node
        .expect("expected node to be created");
    gen3.node = Some(node.clone());

    node.dyn_ref::<web_sys::HtmlElement>()
        .expect("expected html element")
        .click();

    assert_eq!(app.borrow().0, 2);
}
