#[deny(missing_docs)]

use web_sys;
use std::fmt;
use std::cmp;

struct Dom<Message> {
    m: Message,
}

trait Update {
    //fn update(&mut self) -> Command
    fn update(&mut self);
}

trait Render<Message> {
    fn render(&self) -> Dom<Message>;
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum EventHandler<'a, Message> {
    Msg(&'a Message),
    Fn(fn(web_sys::Event) -> Message),
}

/// Items representing all of the data in the DOM tree.
///
/// This is the struct emitted from the `Iterator` passed to our `diff` function. The items emitted
/// should always be in the same order, given the same input. Each entry in the enum represents
/// some aspect of a DOM node. The idea here is the sequence of items will be the same sequence of
/// things seen if we were to walk the DOM tree depth first going through all nodes and their
/// various attributes and events.
enum DomItem<'a, Message> {
    /// A node in the tree.
    Node { node: Option<web_sys::Element>, element: &'a str, store: Box<FnMut(web_sys::Element) + 'a> },
    /// An attribute of the last node we saw.
    Attr { name: &'a str, value: &'a str },
    /// An event handler from the last node we saw.
    Event { trigger: &'a str, handler: EventHandler<'a, Message>  },
    /// We are finished processing children nodes, the next node is a sibling.
    Up,
}

impl<'a, Message> fmt::Debug for DomItem<'a, Message>
where
    Message: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DomItem::Node { node: Some(_), element: e, store: _ } => write!(f, "Node {{ node: Some(_), element: {:?}, store: _ }}", e),
            DomItem::Node { node: None, element: e, store: _ } => write!(f, "Node {{ node: None, element: {:?}, store: _ }}", e),
            DomItem::Attr { name: n, value: v } => write!(f, "Attr {{ name: {:?}, value: {:?} }}", n, v),
            DomItem::Event { trigger: t, handler: h } => write!(f, "Event {{ trigger: {:?}, handler: {:?} }}", t, h),
            DomItem::Up => write!(f, "Up"),
        }
    }
}

impl<'a, Message> cmp::PartialEq for DomItem<'a, Message>
where
    Message: cmp::PartialEq
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                DomItem::Node { node: _, element: e1, store: _ },
                DomItem::Node { node: _, element: e2, store: _ }
            )
            => e1 == e2,
            (
                DomItem::Attr { name: n1, value: v1 },
                DomItem::Attr { name: n2, value: v2 }
            )
            => n1 == n2 && v1 == v2,
            (
                DomItem::Event { trigger: t1, handler: h1 },
                DomItem::Event { trigger: t2, handler: h2 }
            )
            => t1 == t2 && h1 == h2,
            (DomItem::Up, DomItem::Up) => true,
            (_, _) => false,
        }
    }
}

type DomIter<'a, Message> = Iterator<Item = DomItem<'a, Message>>;

// make nodes as part of the patch step, but store the links back to store them here? Then we'll
// have refs twice though?

struct Attr<'a> {
    name: &'a str,
    value: &'a str,
}

struct Event<'a, Message> {
    trigger: &'a str,
    handler: EventHandler<'a, Message>,
}

trait DomTree<'a, Message> {
    fn element() -> &'a str;
    fn attributes() -> Iterator<Item = Attr<'a>>;
    fn events() -> Iterator<Item = Event<'a, Message>>;
//    fn children() -> Iterator<Item = DomTree<'a, Message>>;
}

enum Patch<'new, Message> {
    RemoveNode(web_sys::Element),
    CreateNode { store: Box<FnMut(web_sys::Element) + 'new>, element: String },
    CopyNode { store: Box<FnMut(web_sys::Element) + 'new>, node: web_sys::Element },
    AddAttribute { name: String, value: String },
    RemoveAttribute(String),
    AddListener { trigger: String, handler: EventHandler<'new, Message> },
    RemoveListener(String),
    Up,
}

impl<'a, Message> fmt::Debug for Patch<'a, Message>
where
    Message: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Patch::RemoveNode(_) => write!(f, "RemoveNode(_)"),
            Patch::CreateNode { store: _, element: s } => write!(f, "CreateNode {{ store: _, element: {:?} }}", s),
            Patch::CopyNode { store: _, node: _ } => write!(f, "CopyNode {{ store: _, node: _ }}"),
            Patch::AddAttribute { name: n, value: v } => write!(f, "AddAttribute {{ name: {:?}, value: {:?} }}", n, v),
            Patch::RemoveAttribute(s) => write!(f, "RemoveAttribute({:?})", s),
            Patch::AddListener { trigger: t, handler: h } => write!(f, "AddListener {{ trigger: {:?}, handler: {:?} }}", t, h),
            Patch::RemoveListener(s) => write!(f, "RemoveListener({:?})", s),
            Patch::Up => write!(f, "Up"),
        }
    }
}

impl<'a, Message> cmp::PartialEq for Patch<'a, Message>
where
    Message: cmp::PartialEq
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Patch::RemoveNode(_), Patch::RemoveNode(_)) => true,
            (
                Patch::CreateNode { store: _, element: e1 },
                Patch::CreateNode { store: _, element: e2 },
            )
            => e1 == e2,
            (
                Patch::CopyNode { store: _, node: _ },
                Patch::CopyNode { store: _, node: _ },
            )
            => true,
            (
                Patch::AddAttribute { name: n1, value: v1 },
                Patch::AddAttribute { name: n2, value: v2 },
            )
            => n1 == n2 && v1 == v2,
            (
                Patch::RemoveAttribute(s1),
                Patch::RemoveAttribute(s2),
            )
            => s1 == s2,
            (
                Patch::AddListener { trigger: t1, handler: h1 },
                Patch::AddListener { trigger: t2, handler: h2 },
            )
            => t1 == t2 && h1 == h2,
            (
                Patch::RemoveListener(s1),
                Patch::RemoveListener(s2),
            )
            => s1 == s2,
            (Patch::Up, Patch::Up) => true,
            (_, _) => false,
        }
    }
}

type PatchSet<'a, Message> = Vec<Patch<'a, Message>>;

fn diff<'a, Message, I>(old: &mut I, new: &mut I) -> PatchSet<'a, Message>
where
    Message: PartialEq + Clone + fmt::Debug,
    I: Iterator<Item = DomItem<'a, Message>>,
{
    let mut patch_set = PatchSet::new();

    let mut o_item = old.next();
    let mut n_item = new.next();
    let mut new_node = false;
    let mut old_node = false;

    loop {
        match (o_item.take(), n_item.take()) {
            (None, None) => { // return patch set
                break;
            }
            (o @ None, Some(n))
            | (o @ Some(DomItem::Up), Some(n))
            => { // create remaining new nodes
                match n {
                    DomItem::Node { node: _, element, store } => {
                        patch_set.push(Patch::CreateNode { store, element: element.to_owned() });
                    }
                    DomItem::Attr { name, value } => {
                        patch_set.push(Patch::AddAttribute { name: name.to_owned(), value: value.to_owned() });
                    }
                    DomItem::Event { trigger, handler } => {
                        patch_set.push(Patch::AddListener { trigger: trigger.to_owned(), handler: handler.clone() });
                    }
                    DomItem::Up => {
                        patch_set.push(Patch::Up);
                    }
                }

                n_item = new.next();

                if o.is_some() {
                    o_item = old.next();
                }
            }
            (Some(o), n @ None)
            | (Some(o), n @ Some(DomItem::Up))
            => { // delete remaining old nodes
                // XXX need to only remove the top level nodes
                if let DomItem::Node { node: Some(n), element: _, store: _ } = o {
                    patch_set.push(Patch::RemoveNode(n.clone()));
                }

                o_item = old.next();

                if n.is_some() {
                    patch_set.push(Patch::Up);
                    n_item = new.next();
                }
            }
            (Some(o), Some(n)) => { // compare nodes
                match (o, n) {
                    (
                        DomItem::Node { node: o_node, element: o_element, store: _ },
                        DomItem::Node { node: n_node, element: n_element, store }
                    ) => { // compare elements
                        // if the elements match, use the web_sys::Element
                        if o_element == n_element {
                            // create or copy the node if necessary
                            match (o_node, n_node) {
                                (None, None) => {
                                    patch_set.push(Patch::CreateNode { store, element: n_element.to_owned() });
                                    new_node = true;
                                }
                                (Some(o_elem), None) => {
                                    patch_set.push(Patch::CopyNode { store: store, node: o_elem.clone() });
                                    old_node = true;
                                }
                                // this shouldn't happen, but is harmless if it does
                                (_, Some(_)) => {}
                            }
                            o_item = old.next();
                            n_item = new.next();
                        }
                        // elements don't match, remove the old and make a new one
                        else {
                            if let Some(o_elem) = o_node {
                                patch_set.push(Patch::RemoveNode(o_elem.clone()));
                            }
                            if let Some(n_elem) = n_node {
                                patch_set.push(Patch::RemoveNode(n_elem.clone()));
                            }
                            patch_set.push(Patch::CreateNode { store, element: n_element.to_owned() });
                            new_node = true;
                            
                            // skip the rest of the items in the old tree for this element, this
                            // will cause attributes and such to be created on the new element
                            loop {
                                o_item = old.next();
                                match o_item.take() {
                                    Some(DomItem::Up) | None => break,
                                    _ => o_item = old.next(),
                                }
                            }
                            n_item = new.next();
                        }
                    }
                    (
                        DomItem::Attr { name: o_name, value: o_value },
                        DomItem::Attr { name: n_name, value: n_value }
                    ) => { // compare attributes
                        if new_node {
                            // add attribute
                            patch_set.push(Patch::AddAttribute { name: n_name.to_owned(), value: n_value.to_owned() });
                        }
                        if o_name != n_name || o_value != n_value {
                            if old_node {
                                // remove old attribute
                                patch_set.push(Patch::RemoveAttribute(o_name.to_owned()));
                            }
                            if !new_node {
                                // add new attribute
                                patch_set.push(Patch::AddAttribute { name: n_name.to_owned(), value: n_value.to_owned() });
                            }
                        }
                        o_item = old.next();
                        n_item = new.next();
                    }
                    (
                        DomItem::Event { trigger: o_trigger, handler: o_handler },
                        DomItem::Event { trigger: n_trigger, handler: n_handler }
                    ) => { // compare event listeners
                        if new_node {
                            // add listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger.to_owned(), handler: n_handler.clone() });
                        }
                        if o_trigger != n_trigger || o_handler != n_handler {
                            if old_node {
                                // remove old listener
                                patch_set.push(Patch::RemoveListener(o_trigger.to_owned()));
                            }
                            if !new_node {
                                // add new listener
                                patch_set.push(Patch::AddListener { trigger: n_trigger.to_owned(), handler: n_handler.clone() });
                            }
                        }
                        o_item = old.next();
                        n_item = new.next();
                    }
                    (DomItem::Up, DomItem::Up) => { // end of two items
                        patch_set.push(Patch::Up);

                        new_node = false;
                        old_node = false;

                        o_item = old.next();
                        n_item = new.next();
                    }
                    // add attribute to new node
                    (_, DomItem::Attr { name, value }) => {
                        patch_set.push(Patch::AddAttribute { name: name.to_owned(), value: value.to_owned() });
                        n_item = new.next();
                    }
                    // add event to new node
                    (_, DomItem::Event { trigger, handler }) => {
                        patch_set.push(Patch::AddListener { trigger: trigger.to_owned(), handler: handler.clone() });
                        n_item = new.next();
                    }
                    // remove attribute from old node
                    (DomItem::Attr { name, value: _ }, _) => {
                        if old_node {
                            patch_set.push(Patch::RemoveAttribute(name.to_owned()));
                        }
                        o_item = old.next();
                    }
                    // remove event from old node
                    (DomItem::Event { trigger, handler: _ }, _) => {
                        if old_node {
                            patch_set.push(Patch::RemoveListener(trigger.to_owned()));
                        }
                        o_item = old.next();
                    }
                    // all other combinations should be unreachable
                    (a, b) => unreachable!("({:?}, {:?})", a, b),
                }
            }
        }
    }

    patch_set
}

fn patch(/* IntoIterator<Patch> */) {
    // how to patch? Go through patch set and store nodes as necessary? PatchSet holds mut refs
    // back to the tree and updates new when you run through the set?
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[derive(PartialEq)]
    struct Event<Message> {
        trigger: String,
        handler: EventHandler<Message>,
    }

    struct Dom<Message> {
        element: String,
        attributes: Vec<Attr>,
        events: Vec<Event<Message>>,
        children: Vec<Dom<Message>>,
        node: Option<web_sys::Element>,
    }

    impl<Message> Dom<Message> {

        fn dom(&mut self) -> Vec<DomItem<Message>>
        where
            Message: Clone,
        {
            use std::iter;

            // until generators are stable, this is the best we can do
            iter::once((&mut self.node, &self.element))
                .map(|(node, element)| DomItem::Node {
                    node: node.clone(),
                    element: element,
                    store: Box::new(move |n| *node = Some(n)),
                })
            .chain(self.attributes.iter()
                .map(|attr| DomItem::Attr {
                    name: &attr.name,
                    value: &attr.value
                })
            )
            .chain(self.events.iter()
                .map(|e| DomItem::Event {
                    trigger: &e.trigger,
                    handler: match e.handler {
                        EventHandler::Msg(ref m) => super::EventHandler::Msg(m),
                        EventHandler::Map(f) => super::EventHandler::Fn(f),
                    }
                })
            )
            .chain(self.children.iter_mut()
               .flat_map(|c| Dom::dom(c))
            )
            .chain(iter::once(DomItem::Up))
            .collect()
        }
    }

    macro_rules! compare {
        ( $patch_set:ident, [ $( $x:expr ,)* ] ) => {
            compare!($patch_set, [ $($x),* ]);
        };
        ( $patch_set:ident, [ $( $x:expr),* ] ) => {
            let cmp: PatchSet<Msg> = vec!($($x),*);

            assert_eq!($patch_set.len(), cmp.len(), "lengths don't match\n  left: {:?}\n right: {:?}", $patch_set, cmp);

            for (l, r) in $patch_set.into_iter().zip(cmp) {
                match (l, r) {
                    (Patch::CreateNode { store: _, element: e1 }, Patch::CreateNode { store: _, element: e2 }) => {
                        assert_eq!(e1, e2, "unexpected CreateNode");
                    }
                    (Patch::CopyNode { store: _, node: _ }, Patch::CopyNode { store: _, node: _ }) => {}
                    (Patch::AddAttribute { name: n1, value: v1 }, Patch::AddAttribute { name: n2, value: v2 }) => {
                        assert_eq!(n1, n2, "attribute names don't match");
                        assert_eq!(v1, v2, "attribute values don't match");
                    }
                    (Patch::RemoveAttribute(a1), Patch::RemoveAttribute(a2)) => {
                        assert_eq!(a1, a2, "attribute names don't match");
                    }
                    (Patch::AddListener { trigger: t1, handler: h1 }, Patch::AddListener { trigger: t2, handler: h2 }) => {
                        assert_eq!(t1, t2, "trigger names don't match");
                        assert_eq!(h1, h2, "handlers don't match");
                    }
                    (Patch::RemoveListener(l1), Patch::RemoveListener(l2)) => {
                        assert_eq!(l1, l2, "listner names don't match");
                    }
                    (Patch::RemoveNode(_), Patch::RemoveNode(_)) => {}
                    (Patch::Up, Patch::Up) => {}
                    (i1, i2) => panic!("patch items don't match: {:?} {:?}", i1, i2),
                }
            }
        };
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Msg {}

    #[test]
    fn null_diff() {
        let mut old: Dom<Msg> = Dom {
            element: "div".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec!(),
            node: None,
        };

        let mut new: Dom<Msg> = Dom {
            element: "div".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec!(),
            node: None,
        };

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CreateNode { store: Box::new(|_|()), element: "div".to_owned() },
                Patch::Up,
            ]
        );
    }

    #[test]
    fn basic_diff() {
        let mut old: Dom<Msg> = Dom {
            element: "div".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec!(),
            node: None,
        };

        let mut new: Dom<Msg> = Dom {
            element: "span".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec!(),
            node: None,
        };

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CreateNode { store: Box::new(|_|()), element: "span".to_owned() },
                Patch::Up,
            ]
        );
    }

    #[test]
    fn old_child_nodes() {
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
                        Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
                    ],
                    children: vec![],
                    node: None,
                },
                Dom {
                    element: "i".into(),
                    attributes: vec![
                        Attr { name: "class".into(), value: "item".into() },
                        Attr { name: "id".into(), value: "id".into() },
                    ],
                    events: vec![
                        Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
                    ],
                    children: vec![],
                    node: None,
                },
            ],
            node: None,
        };

        let mut new: Dom<Msg> = Dom {
            element: "div".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec!(),
            node: None,
        };

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CreateNode { store: Box::new(|_|()), element: "div".to_owned() },
                Patch::Up,
            ]
        );
    }

    #[test]
    fn new_child_nodes() {
        let mut old: Dom<Msg> = Dom {
            element: "div".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec![],
            node: None,
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
                        Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
                    ],
                    children: vec![],
                    node: None,
                },
                Dom {
                    element: "i".into(),
                    attributes: vec![
                        Attr { name: "class".into(), value: "item".into() },
                        Attr { name: "id".into(), value: "id2".into() },
                    ],
                    events: vec![
                        Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
                    ],
                    children: vec![],
                    node: None,
                },
            ],
            node: None,
        };

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CreateNode { store: Box::new(|_|()), element: "div".to_owned() },
                Patch::CreateNode { store: Box::new(|_|()), element: "b".to_owned() },
                Patch::AddAttribute { name: "class".to_owned(), value: "item".to_owned() },
                Patch::AddAttribute { name: "id".to_owned(), value: "id1".to_owned() },
                Patch::AddListener { trigger: "onclick".to_owned(), handler: super::EventHandler::Msg(&Msg {}) },
                Patch::Up,
                Patch::CreateNode { store: Box::new(|_|()), element: "i".to_owned() },
                Patch::AddAttribute { name: "class".to_owned(), value: "item".to_owned() },
                Patch::AddAttribute { name: "id".to_owned(), value: "id2".to_owned() },
                Patch::AddListener { trigger: "onclick".to_owned(), handler: super::EventHandler::Msg(&Msg {}) },
                Patch::Up,
                Patch::Up,
            ]
        );
    }

    //
    // wasm tests
    //

    use wasm_bindgen_test::*;
    use wasm_bindgen_test::wasm_bindgen_test_configure;
    wasm_bindgen_test_configure!(run_in_browser);

    fn e(name: &str) -> web_sys::Element {
        web_sys::window().expect("expected window")
            .document().expect("expected document")
            .create_element(name).expect("expected element")
    }

    #[wasm_bindgen_test]
    fn null_diff_with_element() {
        let mut old: Dom<Msg> = Dom {
            element: "div".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec!(),
            node: Some(e("div")),
        };

        let mut new: Dom<Msg> = Dom {
            element: "div".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec!(),
            node: None,
        };

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CopyNode { store: Box::new(|_|()), node: e("div") },
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
            node: Some(e("div")),
        };

        let mut new: Dom<Msg> = Dom {
            element: "span".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec!(),
            node: None,
        };

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::RemoveNode(e("div")),
                Patch::CreateNode { store: Box::new(|_|()), element: "span".to_owned() },
                Patch::Up,
            ]
        );
    }
}
