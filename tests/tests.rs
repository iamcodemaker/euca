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
use euca::component::Component;
use euca::diff;

use euca::test::{ App, Msg, Cmd };

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

#[derive(Default)]
struct FakeComponent { }

impl FakeComponent {
    fn new() -> Box<Self> {
        Box::new(FakeComponent { })
    }

    fn create(_: euca::app::dispatch::Dispatcher<Msg, Cmd>)
    -> Box<dyn Component<Msg>>
    {
        Self::new()
    }
}

impl<Message> Component<Message> for FakeComponent {
    fn dispatch(&self, _: Message) { }
    fn detach(&self) { }
    fn node(&self) -> Option<web_sys::Node> { None }
    fn pending(&mut self) -> Vec<web_sys::Node> { vec![] }
}

fn gen_storage<'a, Message, Command, Iter>(iter: Iter) -> Storage<Message> where
    Message: 'a,
    Iter: Iterator<Item = DomItem<'a, Message, Command>>,
{
    iter
        // filter items that do not have storage
        .filter(|i| {
            match i {
                DomItem::Element(_) | DomItem::Text(_) | DomItem::Event { .. }
                | DomItem::Component { .. } => true,
                DomItem::Attr { .. } | DomItem::UnsafeInnerHtml(_)
                | DomItem::Up => false,
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
                        Box::new(|_|()) as Box<dyn FnMut(web_sys::Event)>
                    )
                ),
                DomItem::Component { .. } => WebItem::Component(FakeComponent::new()),
                DomItem::Attr { .. } | DomItem::Up | DomItem::UnsafeInnerHtml(_) => {
                    unreachable!("attribute, inner html, and up nodes should have been filtered out")
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
        let cmp: PatchSet<Msg, Cmd> = vec!($($x),*).into();

        let dump = format!("patch_set: {:#?}\nexpected: {:#?}",  $patch_set, cmp);

        assert_eq!($patch_set.len(), cmp.len(), "lengths don't match\n{}", dump);

        for (i, (l, r)) in $patch_set.into_iter().zip(cmp).enumerate() {
            match (l, r) {
                (Patch::CreateElement { element: e1 }, Patch::CreateElement { element: e2 }) => {
                    assert_eq!(e1, e2, "[{}] unexpected CreateElement\n{}", i, dump);
                }
                (Patch::CopyElement(_), Patch::CopyElement(_)) => {}
                (Patch::SetAttribute { name: n1, value: v1 }, Patch::SetAttribute { name: n2, value: v2 }) => {
                    assert_eq!(n1, n2, "[{}] attribute names don't match\n{}", i, dump);
                    assert_eq!(v1, v2, "[{}] attribute values don't match\n{}", i, dump);
                }
                (Patch::ReplaceText { take: _, text: t1 }, Patch::ReplaceText { take: _, text: t2 }) => {
                    assert_eq!(t1, t2, "[{}] unexpected ReplaceText\n{}", i, dump);
                }
                (Patch::CreateText { text: t1 }, Patch::CreateText { text: t2 }) => {
                    assert_eq!(t1, t2, "[{}] unexpected CreateText\n{}", i, dump);
                }
                (Patch::CopyText(_), Patch::CopyText(_)) => {}
                (Patch::RemoveAttribute(a1), Patch::RemoveAttribute(a2)) => {
                    assert_eq!(a1, a2, "[{}] attribute names don't match\n{}", i, dump);
                }
                (Patch::AddListener { trigger: t1, handler: h1 }, Patch::AddListener { trigger: t2, handler: h2 }) => {
                    assert_eq!(t1, t2, "[{}] trigger names don't match\n{}", i, dump);
                    assert_eq!(h1, h2, "[{}] handlers don't match\n{}", i, dump);
                }
                (Patch::RemoveListener { trigger: t1, take: _ }, Patch::RemoveListener { trigger: t2, take: _ }) => {
                    assert_eq!(t1, t2, "[{}] trigger names don't match\n{}", i, dump);
                }
                (Patch::CopyListener(_), Patch::CopyListener(_)) => {}
                (Patch::RemoveElement(_), Patch::RemoveElement(_)) => {}
                (Patch::RemoveText(_), Patch::RemoveText(_)) => {}
                (Patch::SetInnerHtml(h1), Patch::SetInnerHtml(h2)) => {
                    assert_eq!(h1, h2, "[{}] unexpected innerHtml\n{}", i, dump);
                }
                (Patch::UnsetInnerHtml, Patch::UnsetInnerHtml) => {}
                (Patch::CreateComponent { msg: m1, create: f1 }, Patch::CreateComponent { msg: m2, create: f2 }) => {
                    assert_eq!(m1, m2, "[{}] component messages don't match\n{}", i, dump);
                    assert_eq!(f1, f2, "[{}] component create functions don't match\n{}", i, dump);
                }
                (Patch::RemoveComponent(_), Patch::RemoveComponent(_)) => {}
                (Patch::CopyComponent(_), Patch::CopyComponent(_)) => {}
                (Patch::Up, Patch::Up) => {}
                (item1, item2) => panic!("[{}] patch items don't match\n  left: {:?}\n right: {:?}\n{}", i, item1, item2, dump),
            }
        }
    };
}

#[test]
fn basic_diff() {
    let old = iter::empty();
    let mut storage = vec![];

    let new = Dom::<_, Cmd>::elem("span");

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

    let new = Dom::<_, Cmd>::elem("div")
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
    let old = Dom::<_, Cmd>::elem("div");
    let new = Dom::<_, Cmd>::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id1")
            .event("onclick", ())
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id2")
            .event("onclick", ())
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
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&()) },
            Patch::Up,
            Patch::CreateElement { element: "i".into() },
            Patch::SetAttribute { name: "class", value: "item" },
            Patch::SetAttribute { name: "id", value: "id2" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&()) },
            Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn from_empty() {
    let new = Dom::<_, Cmd>::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id1")
            .event("onclick", ())
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id2")
            .event("onclick", ())
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
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&()) },
            Patch::Up,
            Patch::CreateElement { element: "i" },
            Patch::SetAttribute { name: "class", value: "item" },
            Patch::SetAttribute { name: "id", value: "id2" },
            Patch::AddListener { trigger: "onclick", handler: euca::vdom::EventHandler::Msg(&()) },
            Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn to_empty() {
    let old = Dom::<_, Cmd>::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id1")
            .event("onclick", ())
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id2")
            .event("onclick", ())
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
    let old: DomVec<_, Cmd> = vec![
        Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id1")
            .event("onclick", ()),
        Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id2")
            .event("onclick", ()),
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
    let old = Dom::<_, Cmd>::elem("div");
    let new = Dom::<_, Cmd>::elem("div");

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
    let old = Dom::<_, Cmd>::elem("div");
    let new = Dom::<_, Cmd>::elem("span");

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
    let old = Dom::<_, Cmd>::elem("div").attr("name", "value");
    let new = Dom::<_, Cmd>::elem("div").attr("name", "new value");

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
    let old = Dom::<_, Cmd>::elem("input").attr("checked", "false");
    let new = Dom::<_, Cmd>::elem("input").attr("checked", "false");

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
    let old = Dom::<_, Cmd>::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", ())
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", ())
        )
    ;

    let new = Dom::<_, Cmd>::elem("div");

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
    let old = Dom::<_, Cmd>::elem("div")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", ())
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", ())
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
    let old = Dom::<_, Cmd>::elem("div")
        .push(Dom::elem("h1")
            .attr("id", "id")
            .event("onclick", ())
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
            .event("onclick", ())
            .push(Dom::text("header"))
        )
        .push(Dom::elem("p")
            .attr("class", "item")
            .push(Dom::elem("b").push("bold"))
            .push(Dom::text("paragraph1"))
        )
        .push(Dom::elem("button")
            .event("click", ())
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
            Patch::CopyListener(Box::new(|| Closure::wrap(Box::new(|_|{}) as Box<dyn FnMut(web_sys::Event)>))),
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
            Patch::AddListener { trigger: "click", handler: euca::vdom::EventHandler::Msg(&()) },
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
    let old = Dom::<_, Cmd>::elem("span")
        .push(Dom::elem("b")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", ())
        )
        .push(Dom::elem("i")
            .attr("class", "item")
            .attr("id", "id")
            .event("onclick", ())
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
    let old = Dom::<_, Cmd>::elem("div");
    let new = Dom::elem("div");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    let parent = e("div");
    let app = App::dispatcher();
    storage = patch_set.apply(&parent, &app);

    match storage[0] {
        WebItem::Element(_) => {}
        _ => panic!("expected node to be created"),
    }
}

#[wasm_bindgen_test]
fn basic_patch_with_element() {
    let gen1 = iter::empty();
    let gen2 = Dom::<_, Cmd>::elem("div");
    let gen3 = Dom::elem("div");

    let parent = e("div");
    let app = App::dispatcher();
    let mut storage = vec![];

    // first gen create element
    let o = gen1.into_iter();
    let n = gen2.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(&parent, &app);

    match storage[0] {
        WebItem::Element(_) => {}
        _ => panic!("expected node to be created"),
    }

    // second gen remove and replace element
    let o = gen2.dom_iter();
    let n = gen3.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(&parent, &app);

    match storage[0] {
        WebItem::Element(_) => {}
        _ => panic!("expected node to be created"),
    }
}

#[wasm_bindgen_test]
fn basic_event_test() {
    let gen1 = iter::empty();
    let gen2 = Dom::elem("button").event("click", ());

    let parent = e("div");
    let messages = Rc::new(RefCell::new(vec![]));
    let app = App::dispatcher_with_vec(Rc::clone(&messages));
    let mut storage = vec![];

    let o = gen1.into_iter();
    let n = gen2.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(&parent, &app);

    match storage[0] {
        WebItem::Element(ref node) => {
            node.dyn_ref::<web_sys::HtmlElement>()
                .expect("expected html element")
                .click();
        },
        _ => panic!("expected node to be created"),
    }

    assert_eq!(messages.borrow().len(), 1);
}

#[wasm_bindgen_test]
fn listener_copy() {
    let gen1 = iter::empty();
    let gen2 = Dom::elem("button").event("click", ());

    let parent = e("div");
    let messages = Rc::new(RefCell::new(vec![]));
    let app = App::dispatcher_with_vec(Rc::clone(&messages));
    let mut storage = vec![];

    let o = gen1.into_iter();
    let n = gen2.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(&parent, &app);

    match storage[0] {
        WebItem::Element(ref node) => {
            node.dyn_ref::<web_sys::HtmlElement>()
                .expect("expected html element")
                .click();
        },
        _ => panic!("expected node to be created"),
    }

    let gen3 = Dom::elem("button").event("click", ());

    let o = gen2.dom_iter();
    let n = gen3.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);
    storage = patch_set.apply(&parent, &app);

    match storage[0] {
        WebItem::Element(ref node) => {
            node.dyn_ref::<web_sys::HtmlElement>()
                .expect("expected html element")
                .click();
        },
        _ => panic!("expected node to be created"),
    }

    assert_eq!(messages.borrow().len(), 2);
}

#[wasm_bindgen_test]
fn replace_element_with_text() {
    let old = Dom::<_, Cmd>::elem("div");
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
    let old = Dom::<_, Cmd>::text("div");
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

#[wasm_bindgen_test]
fn inner_html_noop() {
    let old;
    let new;
    unsafe {
        old = Dom::<_, Cmd>::elem("div")
            .inner_html("html");
        new = Dom::elem("div")
            .inner_html("html");
    }

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
fn inner_html_add() {
    let old = Dom::<_, Cmd>::elem("div");
    let new;
    unsafe {
        new = Dom::elem("div")
            .inner_html("html");
    }

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::SetInnerHtml("html"),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn inner_html_change() {
    let old;
    let new;
    unsafe {
        old = Dom::<_, Cmd>::elem("div")
            .inner_html("toml");
        new = Dom::elem("div")
            .inner_html("html");
    }

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::SetInnerHtml("html"),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn inner_html_remove() {
    let old;
    unsafe {
        old = Dom::<_, Cmd>::elem("div")
            .inner_html("html");
    }
    let new = Dom::elem("div");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::UnsetInnerHtml,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn inner_html_replace() {
    let old;
    unsafe {
        old = Dom::<_, Cmd>::elem("div")
            .inner_html("html");
    }
    let new = Dom::elem("div")
        .push(Dom::text("html"));

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::UnsetInnerHtml,
            Patch::CreateText { text: "html" },
            Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn inner_html_replace_with_children() {
    let old;
    let new;
    unsafe {
        old = Dom::<_, Cmd>::elem("div")
            .inner_html("html");
        new = Dom::elem("div")
            .push(Dom::elem("div")
                .inner_html("html")
            )
        ;
    }

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::UnsetInnerHtml,
            Patch::CreateElement { element: "div" },
            Patch::SetInnerHtml("html"),
            Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn inner_html_replace_children() {
    let old;
    let new;
    unsafe {
        old = Dom::<_, Cmd>::elem("div")
            .push(Dom::elem("div")
                .inner_html("html")
            )
        ;
        new = Dom::elem("div")
            .inner_html("html");
    }

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
            Patch::RemoveElement(Box::new(|| e("div"))),
            Patch::SetInnerHtml("html"),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn inner_html_remove_parent_node() {
    let gen1: DomVec;
    let gen2: DomVec;
    unsafe {
        gen1 = vec![
            Dom::elem("div")
                .push(Dom::elem("p").push("test2")),
            Dom::elem("div")
                .inner_html("<div><p>test5</p></div>"),
        ].into();
        gen2 = vec![
            Dom::elem("div")
                .push(Dom::elem("p").push("test3")),
        ].into();
    }

    let parent = e("div");
    let app = App::dispatcher();
    let mut storage = vec![];

    let n = gen1.dom_iter();
    let patch_set = diff::diff(iter::empty(), n, &mut storage);
    storage = patch_set.apply(&parent, &app);

    let o = gen1.dom_iter();
    let n = gen2.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);
    console_log::init().unwrap_throw();
    log::info!("{:?}", patch_set);
    let _ = patch_set.apply(&parent, &app);

    assert_eq!(
        parent.children()
            .item(0)
            .expect_throw("expected outer child node")
            .children()
            .item(0)
            .expect_throw("expected inner child node")
            .node_name(),
        "P",
        "wrong node in DOM"
    );

    assert!(
        parent.children()
            .item(1)
            .is_none(),
        "unexpected second child node, should only be one child node"
    );
}

#[test]
fn diff_empty_create_component() {
    let old = iter::empty();
    let mut storage = vec![];

    let new = Dom::component((), FakeComponent::create);

    let o = old.into_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CreateComponent { msg: (), create: FakeComponent::create },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_basic_component() {

    let old = Dom::elem("div");
    let new = Dom::elem("div")
        .push(Dom::component((), FakeComponent::create));

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
              Patch::CreateComponent { msg: (), create: FakeComponent::create },
              Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_add_nested_component() {
    let old = Dom::elem("div")
        .push(Dom::elem("div")
            .push(Dom::elem("div"))
            .push(Dom::elem("div"))
            .push(Dom::elem("div"))
        )
        .push(Dom::elem("div"));
    let new = Dom::elem("div")
        .push(Dom::elem("div")
            .push(Dom::elem("div"))
            .push(Dom::elem("div"))
            .push(Dom::elem("div"))
            .push(Dom::component((), FakeComponent::create))
        )
        .push(Dom::elem("div"));

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
              Patch::CopyElement(Box::new(|| e("div"))),
                Patch::CopyElement(Box::new(|| e("div"))),
                Patch::Up,
                Patch::CopyElement(Box::new(|| e("div"))),
                Patch::Up,
                Patch::CopyElement(Box::new(|| e("div"))),
                Patch::Up,
                Patch::CreateComponent { msg: (), create: FakeComponent::create },
                Patch::Up,
              Patch::Up,
              Patch::CopyElement(Box::new(|| e("div"))),
              Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_copy_nested_component() {
    let old = Dom::elem("div")
        .push(Dom::elem("div")
            .push(Dom::elem("div"))
            .push(Dom::component((), FakeComponent::create))
            .push(Dom::elem("div"))
        )
        .push(Dom::elem("div"));
    let new = Dom::elem("div")
        .push(Dom::elem("div")
            .push(Dom::elem("div"))
            .push(Dom::component((), FakeComponent::create))
            .push(Dom::elem("div"))
        )
        .push(Dom::elem("div"));

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
              Patch::CopyElement(Box::new(|| e("div"))),
                Patch::CopyElement(Box::new(|| e("div"))),
                Patch::Up,
                Patch::CopyComponent(Box::new(|| FakeComponent::new())),
                Patch::Up,
                Patch::CopyElement(Box::new(|| e("div"))),
                Patch::Up,
              Patch::Up,
              Patch::CopyElement(Box::new(|| e("div"))),
              Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_remove_nested_component() {
    let old = Dom::elem("div")
        .push(Dom::elem("div")
            .push(Dom::elem("div"))
            .push(Dom::component((), FakeComponent::create))
            .push(Dom::elem("div"))
        )
        .push(Dom::elem("div"));
    let new = Dom::elem("div")
        .push(Dom::elem("div")
            .push(Dom::elem("div"))
            .push(Dom::elem("div"))
        )
        .push(Dom::elem("div"));

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(Box::new(|| e("div"))),
              Patch::CopyElement(Box::new(|| e("div"))),
                Patch::CopyElement(Box::new(|| e("div"))),
                Patch::Up,
                Patch::RemoveComponent(Box::new(|| FakeComponent::new())),
                Patch::CreateElement { element: "div" },
                Patch::Up,
                Patch::RemoveElement(Box::new(|| e("div"))),
              Patch::Up,
              Patch::CopyElement(Box::new(|| e("div"))),
              Patch::Up,
            Patch::Up,
        ]
    );
}
