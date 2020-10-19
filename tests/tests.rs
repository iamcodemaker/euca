use std::rc::Rc;
use std::cell::RefCell;
use std::iter;
use std::fmt;
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

fn leaked_e<Message>(name: &str) -> &mut WebItem<Message> {
    Box::leak(Box::new(WebItem::Element(e(name))))
}

fn t(text: &str) -> web_sys::Text {
    web_sys::window().expect("expected window")
        .document().expect("expected document")
        .create_text_node(text)
}

fn leaked_t<Message>(text: &str) -> &mut WebItem<Message> {
    Box::leak(Box::new(WebItem::Text(t(text))))
}

fn leaked_closure<Message>() -> &'static mut WebItem<Message> {
    Box::leak(Box::new(WebItem::Closure(Closure::wrap(Box::new(|_|{}) as Box<dyn FnMut(web_sys::Event)>))))
}

#[derive(Default)]
struct FakeComponent { }

impl FakeComponent {
    fn new() -> Box<Self> {
        Box::new(FakeComponent { })
    }

    fn leaked<Message>() -> &'static mut WebItem<Message> {
        Box::leak(Box::new(WebItem::Component(Self::new())))
    }

    fn create(_: euca::app::dispatch::Dispatcher<Msg, Cmd>)
    -> Box<dyn Component<Msg>>
    {
        Self::new()
    }

    fn create2(_: euca::app::dispatch::Dispatcher<Msg, Cmd>)
    -> Box<dyn Component<Msg>>
    {
        Self::new()
    }
}

impl<Message> Component<Message> for FakeComponent {
    fn dispatch(&self, _: Message) { }
    fn detach(&self) { }
    fn node(&self) -> Option<web_sys::Node> { None }
    fn nodes(&self) -> Vec<web_sys::Node> { vec![] }
    fn pending(&mut self) -> Vec<web_sys::Node> { vec![] }
}

fn gen_storage<'a, Message, Command, Key, Iter>(iter: Iter) -> Storage<Message> where
    Message: 'a,
    Key: 'a,
    Iter: Iterator<Item = DomItem<'a, Message, Command, Key>>,
{
    iter
        // filter items that do not have storage
        .filter(|i| {
            match i {
                DomItem::Element { .. } | DomItem::Text(_) | DomItem::Event { .. }
                | DomItem::Component { .. } | DomItem::Up => true,
                DomItem::Key(_) | DomItem::Attr { .. } | DomItem::UnsafeInnerHtml(_)
                => false,
            }
        })
        .map(|i| {
            match i {
                DomItem::Element { name: element, .. } => WebItem::Element(e(element)),
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
                DomItem::Up => WebItem::Up,
                DomItem::Component { .. } => WebItem::Component(FakeComponent::new()),
                DomItem::Attr { .. } | DomItem::Key(_)
                | DomItem::UnsafeInnerHtml(_) => {
                    unreachable!("attribute, inner html, and up nodes should have been filtered out")
                },
            }
        })
        .collect()
}

fn compare_patch_vecs<K: fmt::Debug + Eq + ?Sized>(left: &Vec<Patch<Msg, Cmd, &K>>, right: &Vec<Patch<Msg, Cmd, &K>>, dump: &str) {
    assert_eq!(left.len(), right.len(), "lengths don't match\n{}", dump);

    for (i, (l, r)) in left.iter().zip(right).enumerate() {
        match (l, r) {
            (Patch::ReferenceKey(k1), Patch::ReferenceKey(k2)) => {
                assert_eq!(k1, k2, "[{}] ReferenceKey keys don't match\n{}", i, dump);
            }
            (Patch::CreateElement { element: e1 }, Patch::CreateElement { element: e2 }) => {
                assert_eq!(e1, e2, "[{}] unexpected CreateElement\n{}", i, dump);
            }
            (Patch::CopyElement(WebItem::Element(e1)), Patch::CopyElement(WebItem::Element(e2))) => {
                assert_eq!(e1.tag_name(), e2.tag_name(), "[{}] WebItems don't match for CopyElement\n{}", i, dump);
            }
            (Patch::MoveElement(WebItem::Element(e1)), Patch::MoveElement(WebItem::Element(e2))) => {
                assert_eq!(e1.tag_name(), e2.tag_name(), "[{}] WebItems don't match for MoveElement\n{}", i, dump);
            }
            (Patch::SetAttribute { name: n1, value: v1 }, Patch::SetAttribute { name: n2, value: v2 }) => {
                assert_eq!(n1, n2, "[{}] attribute names don't match\n{}", i, dump);
                assert_eq!(v1, v2, "[{}] attribute values don't match\n{}", i, dump);
            }
            (Patch::ReplaceText { take: WebItem::Text(wt1), text: t1 }, Patch::ReplaceText { take: WebItem::Text(wt2), text: t2 }) => {
                assert_eq!(t1, t2, "[{}] unexpected ReplaceText\n{}", i, dump);
                assert_eq!(wt1.data(), wt2.data(), "[{}] WebItems don't match for ReplaceText\n{}", i, dump);
            }
            (Patch::CreateText { text: t1 }, Patch::CreateText { text: t2 }) => {
                assert_eq!(t1, t2, "[{}] unexpected CreateText\n{}", i, dump);
            }
            (Patch::CopyText(WebItem::Text(wt1)), Patch::CopyText(WebItem::Text(wt2))) => {
                assert_eq!(wt1.data(), wt2.data(), "[{}] WebItems don't match for CopyText\n{}", i, dump);
            }
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
            (Patch::RemoveElement(WebItem::Element(e1)), Patch::RemoveElement(WebItem::Element(e2))) => {
                assert_eq!(e1.tag_name(), e2.tag_name(), "[{}] unexpected RemoveElement\n{}", i, dump);
            }
            (Patch::RemoveText(WebItem::Text(wt1)), Patch::RemoveText(WebItem::Text(wt2))) => {
                assert_eq!(wt1.data(), wt2.data(), "[{}] WebItems don't match for RemoveText\n{}", i, dump);
            }
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
}

macro_rules! compare {
    ( $patch_set:ident, [ $( $x:expr ,)* ] ) => {
        compare!($patch_set, [ $($x),* ]);
    };
    ( $patch_set:ident, [ $( $x:expr),* ] ) => {
        let cmp: PatchSet<Msg, Cmd, &_> = vec!($($x),*).into();
        compare!($patch_set, cmp)
    };
    ( $patch_set:ident, [ $( $x:expr ,)* ], $( $k:expr => [ $( $v:expr ,)* ], )* ) => {
        compare!($patch_set, [ $($x),* ], $( $k => [ $($v),* ] ),*)
    };
    ( $patch_set:ident, [ $( $x:expr),* ], $( $k:expr => [ $( $v:expr),* ] ),* ) => {
        let mut cmp: PatchSet<Msg, Cmd, &_> = vec!($($x),*).into();
        $(
            cmp.keyed.insert($k, vec!($($v),*));
        )*

        compare!($patch_set, cmp)
    };
    ( $patch_set:ident, $cmp_set:ident ) => {

        let dump = format!("patch_set: {:#?}\nexpected: {:#?}",  $patch_set, $cmp_set);

        compare_patch_vecs(&$patch_set.patches, &$cmp_set.patches, &dump);

        let mut patch_set = $patch_set;
        let mut cmp_set = $cmp_set;
        for (key, cmp) in cmp_set.keyed.drain() {
            if let Some(patches) = patch_set.keyed.remove(key) {
                compare_patch_vecs(&patches, &cmp, &dump);
            }
            else {
                panic!("failed to find expected key '{:?}' in patch set\n{}", key, dump);
            }
        }

        if !patch_set.keyed.is_empty() {
            panic!("unexpected keys in patch set\n{}", dump);
        }
    };
}

#[test]
fn basic_diff() {
    let old = iter::empty();
    let mut storage = vec![];

    let new = Dom::<_, _, &()>::elem("span");

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

    let new = Dom::<_, _, &()>::elem("div")
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
    let old = Dom::<_, _, &()>::elem("div");
    let new = Dom::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
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
    let new = Dom::<_, _, &()>::elem("div")
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
    let old = Dom::<_, _, &()>::elem("div")
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
            Patch::RemoveElement(leaked_e("div")),
        ]
    );
}

#[wasm_bindgen_test]
fn to_empty_vec() {
    let old: DomVec<_, _, &()> = vec![
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
            Patch::RemoveElement(leaked_e("b")),
            Patch::RemoveElement(leaked_e("i")),
        ]
    );
}

#[wasm_bindgen_test]
fn no_difference() {
    let old = Dom::<_, _, &()>::elem("div");
    let new = Dom::elem("div");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn basic_diff_with_element() {
    let old = Dom::<_, _, &()>::elem("div");
    let new = Dom::elem("span");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::RemoveElement(leaked_e("div")),
            Patch::CreateElement { element: "span".into() },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_attributes() {
    let old = Dom::<_, _, &()>::elem("div").attr("name", "value");
    let new = Dom::elem("div").attr("name", "new value");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
            Patch::SetAttribute { name: "name", value: "new value" },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_checked() {
    let old = Dom::<_, _, &()>::elem("input").attr("checked", "false");
    let new = Dom::elem("input").attr("checked", "false");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("input")),
            Patch::SetAttribute { name: "checked", value: "false" },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn old_child_nodes_with_element() {
    let old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
            Patch::RemoveElement(leaked_e("b")),
            Patch::RemoveElement(leaked_e("i")),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn old_child_nodes_with_element_and_child() {
    let old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
            Patch::RemoveElement(leaked_e("b")),
            Patch::CreateElement { element: "i".into() },
            Patch::Up,
            Patch::RemoveElement(leaked_e("i")),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn assorted_child_nodes() {
    let old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
            Patch::CopyElement(leaked_e("h1")),
            Patch::CopyListener(leaked_closure()),
            Patch::ReplaceText { take: leaked_t("h1"), text: "header" },
            Patch::Up,
            Patch::Up,
            Patch::CopyElement(leaked_e("p")),
            Patch::RemoveText(leaked_t("paragraph1")),
            Patch::CreateElement { element: "b".into() },
            Patch::CreateText { text: "bold" },
            Patch::Up,
            Patch::Up,
            Patch::CreateText { text: "paragraph1" },
            Patch::Up,
            Patch::Up,
            Patch::RemoveElement(leaked_e("p")),
            Patch::CreateElement { element: "button".into() },
            Patch::AddListener { trigger: "click", handler: euca::vdom::EventHandler::Msg(&()) },
            Patch::CreateText { text: "submit" },
            Patch::Up,
            Patch::Up,
            Patch::RemoveElement(leaked_e("p")),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_old_child_nodes_with_new_element() {
    let old = Dom::<_, _, &()>::elem("span")
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
            Patch::RemoveElement(leaked_e("span")),
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
    let gen2 = Dom::<_, _, &()>::elem("button").event("click", ());

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
    let gen2 = Dom::<_, _, &()>::elem("button").event("click", ());

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
    let old = Dom::<_, _, &()>::elem("div");
    let new = Dom::text("div");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::RemoveElement(leaked_e("div")),
            Patch::CreateText { text: "div".into() },
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn replace_text_with_element() {
    let old = Dom::<_, _, &()>::text("div");
    let new = Dom::elem("div");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::RemoveText(leaked_t("div")),
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
        old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn inner_html_add() {
    let old = Dom::<_, _, &()>::elem("div");
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
            Patch::CopyElement(leaked_e("div")),
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
        old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
            Patch::SetInnerHtml("html"),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn inner_html_remove() {
    let old;
    unsafe {
        old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
            Patch::UnsetInnerHtml,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn inner_html_replace() {
    let old;
    unsafe {
        old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
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
        old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
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
        old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
            Patch::RemoveElement(leaked_e("div")),
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

    let new = Dom::<_, _, &()>::component((), FakeComponent::create);

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

    let old = Dom::<_, _, &()>::elem("div");
    let new = Dom::elem("div")
        .push(Dom::component((), FakeComponent::create));

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
              Patch::CreateComponent { msg: (), create: FakeComponent::create },
              Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_two_components() {

    let old = Dom::<_, _, &()>::elem("div")
        .push(Dom::component((), FakeComponent::create));
    let new = Dom::elem("div")
        .push(Dom::component((), FakeComponent::create2));

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
              Patch::RemoveComponent(FakeComponent::leaked()),
              Patch::CreateComponent { msg: (), create: FakeComponent::create2 },
              Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_add_nested_component() {
    let old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
              Patch::CopyElement(leaked_e("div")),
                Patch::CopyElement(leaked_e("div")),
                Patch::Up,
                Patch::CopyElement(leaked_e("div")),
                Patch::Up,
                Patch::CopyElement(leaked_e("div")),
                Patch::Up,
                Patch::CreateComponent { msg: (), create: FakeComponent::create },
                Patch::Up,
              Patch::Up,
              Patch::CopyElement(leaked_e("div")),
              Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_copy_nested_component() {
    let old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
              Patch::CopyElement(leaked_e("div")),
                Patch::CopyElement(leaked_e("div")),
                Patch::Up,
                Patch::CopyComponent(FakeComponent::leaked()),
                Patch::Up,
                Patch::CopyElement(leaked_e("div")),
                Patch::Up,
              Patch::Up,
              Patch::CopyElement(leaked_e("div")),
              Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_remove_nested_component() {
    let old = Dom::<_, _, &()>::elem("div")
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
            Patch::CopyElement(leaked_e("div")),
              Patch::CopyElement(leaked_e("div")),
                Patch::CopyElement(leaked_e("div")),
                Patch::Up,
                Patch::RemoveComponent(FakeComponent::leaked()),
                Patch::CreateElement { element: "div" },
                Patch::Up,
                Patch::RemoveElement(leaked_e("div")),
              Patch::Up,
              Patch::CopyElement(leaked_e("div")),
              Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_keyed_element_equal() {
    let old = Dom::elem("div")
        .push(Dom::elem("div")
            .key("yup")
        );
    let new = Dom::elem("div")
        .push(Dom::elem("div")
            .key("yup")
        );

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
              Patch::MoveElement(leaked_e("div")),
              Patch::Up,
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_keyed_element_not_equal() {
    let old = Dom::elem("div")
        .push(Dom::elem("div")
            .key("yup")
        );
    let new = Dom::elem("div")
        .push(Dom::elem("div")
            .key("nope")
        );

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
              Patch::ReferenceKey(&"nope"),
            Patch::Up,
            Patch::RemoveElement(leaked_e("div")),
        ],
        &"nope" => [
            Patch::CreateElement { element: "div" },
            Patch::Up,
        ],
    );
}

#[wasm_bindgen_test]
fn diff_keyed_element_move_key() {
    let old = Dom::elem("div")
        .push(Dom::elem("div"))
        .push(Dom::elem("div")
            .key("yup")
        );
    let new = Dom::elem("div")
        .push(Dom::elem("div")
            .key("yup")
        )
        .push(Dom::elem("div"));

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
              Patch::RemoveElement(leaked_e("div")),
              Patch::ReferenceKey(&"yup"),
              Patch::CreateElement { element: "div" },
              Patch::Up,
            Patch::Up,
        ],
        &"yup" => [
            Patch::MoveElement(leaked_e("div")),
            Patch::Up,
        ],
    );
}

#[wasm_bindgen_test]
fn diff_keyed_from_empty() {
    let new = Dom::elem("div")
        .push(Dom::elem("div")
            .key("yup")
        );

    let o = iter::empty();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, iter::empty());

    compare!(
        patch_set,
        [
            Patch::CreateElement { element: "div" },
              Patch::ReferenceKey(&"yup"),
            Patch::Up,
        ],
        &"yup" => [
            Patch::CreateElement { element: "div" },
            Patch::Up,
        ],
    );
}

#[wasm_bindgen_test]
fn diff_keyed_to_empty() {
    let old = Dom::elem("div")
        .push(Dom::elem("div")
            .key("yup")
        );

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = iter::empty();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::RemoveElement(leaked_e("div")),
            Patch::RemoveElement(leaked_e("div")),
        ]
    );
}

#[wasm_bindgen_test]
fn diff_nested_keyed_element_swap() {
    let old = Dom::elem("div")
        .push(Dom::elem("div")
            .key("yup")
            .push(Dom::elem("span")
                .key("yup yup")
            )
        );
    let new = Dom::elem("div")
        .push(Dom::elem("span")
            .key("yup yup")
            .push(Dom::elem("div")
                .key("yup")
            )
        );

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
              Patch::ReferenceKey(&"yup yup"),
            Patch::Up,
        ],
        &"yup yup" => [
            Patch::MoveElement(leaked_e("span")),
              Patch::ReferenceKey(&"yup"),
            Patch::Up,
        ],
        &"yup" => [
            Patch::MoveElement(leaked_e("div")),
            Patch::Up,
        ],
    );
}

#[wasm_bindgen_test]
fn diff_duplicate_key() {
    let old = Dom::elem("div")
        .push(Dom::elem("div")
            .key("yup")
        )
        .push(Dom::elem("span")
            .key("yup")
        );
    let new = Dom::elem("div")
        .push(Dom::elem("div"))
        .push(Dom::elem("div")
            .key("yup")
        )
        .push(Dom::elem("span")
            .key("yup")
        );

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
              Patch::CreateElement { element: "div" },
              Patch::Up,
              Patch::RemoveElement(leaked_e("span")),
              Patch::ReferenceKey(&"yup"),
              Patch::CreateElement { element: "span" },
              Patch::Up,
            Patch::Up,
        ],
        &"yup" => [
            Patch::MoveElement(leaked_e("div")),
            Patch::Up,
        ],
    );
}

#[wasm_bindgen_test]
fn diff_remove_attr() {
    let old = Dom::<_, _, &()>::elem("div")
        .attr("name", "value")
        .push("text");
    let new = Dom::elem("div")
        .push("text");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
              Patch::RemoveAttribute("name"),
              Patch::CreateText { text: "text".into() },
              Patch::Up,
              Patch::RemoveText(leaked_t("text")),
            Patch::Up,
        ]
    );
}

#[wasm_bindgen_test]
fn diff_remove_event_handler() {
    let old = Dom::<_, _, &()>::elem("div")
        .event("onclick", ())
        .push("text");
    let new = Dom::elem("div")
        .push("text");

    let mut storage = gen_storage(old.dom_iter());
    let o = old.dom_iter();
    let n = new.dom_iter();
    let patch_set = diff::diff(o, n, &mut storage);

    compare!(
        patch_set,
        [
            Patch::CopyElement(leaked_e("div")),
              Patch::RemoveListener { trigger: "onclick", take: leaked_closure() },
              Patch::CreateText { text: "text".into() },
              Patch::Up,
              Patch::RemoveText(leaked_t("text")),
            Patch::Up,
        ]
    );
}
