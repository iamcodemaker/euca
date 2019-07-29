use std::rc::Rc;
use std::cell::RefCell;
use std::iter;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use euca::vdom::WebItem;
use euca::vdom::Storage;
use euca::vdom::DomItem;
use euca::vdom::DomIter;
use euca::dom::Dom;
use euca::dom::DomVec;
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

fn t(text: &str) -> web_sys::Text {
    web_sys::window().expect("expected window")
        .document().expect("expected document")
        .create_text_node(text)
}

fn gen_storage<'a, Message, Iter>(iter: Iter) -> Storage where
    Message: 'a,
    Iter: Iterator<Item = DomItem<'a, Message>>,
{
    iter
        .filter(|i| {
            match i {
                DomItem::Element(_) | DomItem::Text(_) | DomItem::Event { .. } => true,
                DomItem::Attr { .. } | DomItem::Up => false,
            }
        })
        .map(|i| {
            match i {
                DomItem::Element(element) => WebItem::Element(e(element)),
                DomItem::Text(text) => WebItem::Text(
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

        for (i, (l, r)) in $patch_set.into_iter().zip(cmp).enumerate() {
            match (l, r) {
                (Patch::CreateElement { element: e1 }, Patch::CreateElement { element: e2 }) => {
                    assert_eq!(e1, e2, "[{}] unexpected CreateElement", i);
                }
                (Patch::CopyElement(_), Patch::CopyElement(_)) => {}
                (Patch::SetAttribute { name: n1, value: v1 }, Patch::SetAttribute { name: n2, value: v2 }) => {
                    assert_eq!(n1, n2, "[{}] attribute names don't match", i);
                    assert_eq!(v1, v2, "[{}] attribute values don't match", i);
                }
                (Patch::ReplaceText { take: _, text: t1 }, Patch::ReplaceText { take: _, text: t2 }) => {
                    assert_eq!(t1, t2, "[{}] unexpected ReplaceText", i);
                }
                (Patch::CreateText { text: t1 }, Patch::CreateText { text: t2 }) => {
                    assert_eq!(t1, t2, "[{}] unexpected CreateText", i);
                }
                (Patch::CopyText(_), Patch::CopyText(_)) => {}
                (Patch::RemoveAttribute(a1), Patch::RemoveAttribute(a2)) => {
                    assert_eq!(a1, a2, "[{}] attribute names don't match", i);
                }
                (Patch::AddListener { trigger: t1, handler: h1 }, Patch::AddListener { trigger: t2, handler: h2 }) => {
                    assert_eq!(t1, t2, "[{}] trigger names don't match", i);
                    assert_eq!(h1, h2, "[{}] handlers don't match", i);
                }
                (Patch::RemoveListener { trigger: t1, take: _ }, Patch::RemoveListener { trigger: t2, take: _ }) => {
                    assert_eq!(t1, t2, "[{}] trigger names don't match", i);
                }
                (Patch::CopyListener(_), Patch::CopyListener(_)) => {}
                (Patch::RemoveElement(_), Patch::RemoveElement(_)) => {}
                (Patch::RemoveText(_), Patch::RemoveText(_)) => {}
                (Patch::Up, Patch::Up) => {}
                (item1, item2) => panic!("[{}] patch items don't match\n  left: {:?}\n right: {:?}", i, item1, item2),
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

    let new = Dom::elem("span");

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

    let new = Dom::elem("div")
        .push(Dom::text("text"))
    ;

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
    let old = Dom::elem("div");
    let new = Dom::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id1")
            .event("onclick", Msg {})
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id2")
            .event("onclick", Msg {})
        )
    ;

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
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
    let new = Dom::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id1")
            .event("onclick", Msg {})
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id2")
            .event("onclick", Msg {})
        )
    ;

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
    let old = Dom::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id1")
            .event("onclick", Msg {})
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id2")
            .event("onclick", Msg {})
        )
    ;

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
fn to_empty_vec() {
    let old: DomVec<_> = vec![
        Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id1")
            .event("onclick", Msg {}),
        Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id2")
            .event("onclick", Msg {}),
    ].into();

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let patch_set = diff::diff(o, iter::empty(), &mut storage);

    compare!(
        patch_set,
        [
            Patch::RemoveElement(Box::new(|| e("b"))),
            Patch::RemoveElement(Box::new(|| e("i"))),
        ]
    );
}

#[wasm_bindgen_test]
fn no_difference() {
    let old = Dom::elem("div");
    let new = Dom::elem("div");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn basic_diff_with_element() {
    let old = Dom::elem("div");
    let new = Dom::elem("span");

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
    let old = Dom::elem("div").attr("name", "value");
    let new = Dom::elem("div").attr("name", "new value");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::SetAttribute { name: "name", value: "new value" },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_checked() {
    let old = Dom::elem("input").attr("checked", "false");
    let new = Dom::elem("input").attr("checked", "false");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("input"))),
            Patch::SetAttribute { name: "checked", value: "false" },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn old_child_nodes_with_element() {
    let old = Dom::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", Msg {})
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", Msg {})
        )
    ;

    let new = Dom::elem("div");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::RemoveElement(Box::new(|| e("b"))),
            Patch::RemoveElement(Box::new(|| e("i"))),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn old_child_nodes_with_element_and_child() {
    let old = Dom::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", Msg {})
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", Msg {})
        )
    ;

    let new = Dom::elem("div")
        .push(Dom::elem("i"))
    ;

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::RemoveElement(Box::new(|| e("b"))),
            Patch::CreateElement { element: "i".into() },
            Patch::Up,
            Patch::RemoveElement(Box::new(|| e("i"))),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn assorted_child_nodes() {
    let old = Dom::elem("div")
        .push(Dom::elem("h1")
            .attr("id", "id")
            .event("onclick", Msg {})
            .push(Dom::text("h1"))
        )
        .push(Dom::elem("p")
            .attr("class", "item")
            .push(Dom::text("paragraph1"))
        )
        .push(Dom::elem("p")
            .attr("class", "item")
            .push(Dom::text("paragraph2"))
        )
        .push(Dom::elem("p")
            .attr("class", "item")
            .attr("style", "")
            .push(Dom::text("paragraph3"))
        )
    ;

    let new = Dom::elem("div")
        .push(Dom::elem("h1")
            .attr("id", "id")
            .event("onclick", Msg {})
            .push(Dom::text("header"))
        )
        .push(Dom::elem("p")
            .attr("class", "item")
            .push(Dom::elem("b").push("bold"))
            .push(Dom::text("paragraph1"))
        )
        .push(Dom::elem("button")
            .event("click", Msg {})
            .push(Dom::text("submit"))
        )
    ;

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::CopyElement(Box::new(|| e("h1"))),
            Patch::CopyListener(Box::new(|| Closure::wrap(Box::new(|_|{}) as Box<FnMut(web_sys::Event)>))),
            Patch::ReplaceText { take: Box::new(|| t("h1")), text: "header" },
            Patch::Up,
            Patch::Up,
            Patch::CopyElement(Box::new(|| e("p"))),
            Patch::RemoveText(Box::new(|| t("paragraph1"))),
            Patch::CreateElement { element: "b".into() },
            Patch::CreateText { text: "bold" },
            Patch::Up,
            Patch::Up,
            Patch::CreateText { text: "paragraph1" },
            Patch::Up,
            Patch::Up,
            Patch::RemoveElement(Box::new(|| e("p"))),
            Patch::CreateElement { element: "button".into() },
            Patch::AddListener { trigger: "click", handler: euca::vdom::EventHandler::Msg(&Msg {}) },
            Patch::CreateText { text: "submit" },
            Patch::Up,
            Patch::Up,
            Patch::RemoveElement(Box::new(|| e("p"))),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_old_child_nodes_with_new_element() {
    let old = Dom::elem("span")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", Msg {})
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", Msg {})
        )
    ;

    let new = Dom::elem("div");

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
    let old = Dom::elem("div");
    let new = Dom::elem("div");

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
    let gen2 = Dom::elem("div");
    let gen3 = Dom::elem("div");

    let parent = e("div");
    struct App {};
    impl Dispatch<Msg> for App {
        fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
    }

    let app = Rc::new(RefCell::new(App {}));
    let mut storage = vec![];

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
    let gen2 = Dom::elem("button").event("click", Msg {});

    let parent = e("div");
    struct App(i32);
    impl Dispatch<Msg> for App {
        fn dispatch(app: Rc<RefCell<Self>>, _: Msg) {
            let mut app = app.borrow_mut();
            app.0 += 1;
        }
    }

    let app = Rc::new(RefCell::new(App(0)));
    let mut storage = vec![];

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
    let gen2 = Dom::elem("button").event("click", Msg {});

    let parent = e("div");
    struct App(i32);
    impl Dispatch<Msg> for App {
        fn dispatch(app: Rc<RefCell<Self>>, _: Msg) {
            let mut app = app.borrow_mut();
            app.0 += 1;
        }
    }

    let app = Rc::new(RefCell::new(App(0)));
    let mut storage = vec![];

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

    let gen3 = Dom::elem("button").event("click", Msg {});

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

#[wasm_bindgen_test]
fn replace_element_with_text() {
    let old = Dom::elem("div");
    let new = Dom::text("div");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::RemoveElement(Box::new(|| e("div"))),
            Patch::CreateText { text: "div".into() },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn replace_text_with_element() {
    let old = Dom::text("div");
    let new = Dom::elem("div");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::RemoveText(Box::new(|| t("div"))),
            Patch::CreateElement { element: "div".into() },
            Patch::Up,
        ]
    );
}
