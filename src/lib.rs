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
    Event { trigger: &'a str, handler: EventHandler<'a, Message> },
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
    CreateNode { store: Box<FnMut(web_sys::Element) + 'new>, element: &'new str },
    CopyNode { store: Box<FnMut(web_sys::Element) + 'new>, node: web_sys::Element },
    AddAttribute { name: &'new str, value: &'new str },
    RemoveAttribute(&'new str),
    AddListener { trigger: &'new str, handler: EventHandler<'new, Message> },
    RemoveListener(&'new str),
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
    #[derive(PartialEq)]
    enum NodeState {
        Create,
        Copy,
        NewChild,
        OldChild,
    }

    struct State(Vec<NodeState>);

    impl State {
        fn new() -> Self {
            State(vec![])
        }
        fn push(&mut self, state: NodeState) {
            self.0.push(state)
        }
        fn pop(&mut self) -> Option<NodeState> {
            self.0.pop()
        }
        fn is_create(&self) -> bool {
            self.0.last()
                .map_or(false, |ns| *ns == NodeState::Create)
        }
        fn is_copy(&self) -> bool {
            self.0.last()
                .map_or(false, |ns| *ns == NodeState::Copy)
        }
        fn is_child(&self) -> bool {
            self.0.last()
                .map_or(false, |ns| *ns == NodeState::NewChild || *ns == NodeState::OldChild)
        }
        fn is_old_child(&self) -> bool {
            self.0.last()
                .map_or(false, |ns| *ns == NodeState::OldChild)
        }
        fn is_new_child(&self) -> bool {
            self.0.last()
                .map_or(false, |ns| *ns == NodeState::NewChild)
        }
    }

    let mut patch_set = PatchSet::new();

    let mut state = State::new();

    let mut o_item = old.next();
    let mut n_item = new.next();

    loop {
        match (o_item.take(), n_item.take()) {
            (None, None) => { // return patch set
                break;
            }
            (None, Some(n)) => { // create remaining new nodes
                match n {
                    DomItem::Node { node: _, element, store } => {
                        patch_set.push(Patch::CreateNode { store, element });
                    }
                    DomItem::Attr { name, value } => {
                        patch_set.push(Patch::AddAttribute { name, value });
                    }
                    DomItem::Event { trigger, handler } => {
                        patch_set.push(Patch::AddListener { trigger, handler: handler.clone() });
                    }
                    DomItem::Up => {
                        patch_set.push(Patch::Up);
                    }
                }

                n_item = new.next();
            }
            (Some(o), None) => { // delete remaining old nodes
                match o {
                    DomItem::Node { node: None, element: _, store: _ } => {
                        state.push(NodeState::OldChild);
                    }
                    DomItem::Node { node: Some(node), element: _, store: _ } => {
                        // ignore child nodes
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveNode(node.clone()));
                        }

                        state.push(NodeState::OldChild);
                    }
                    DomItem::Up => {
                        state.pop();
                    }
                    // XXX do we need to remove events?
                    DomItem::Event { trigger: _, handler: _ } => {}
                    // ignore attributes
                    DomItem::Attr { name: _, value: _ } => {}
                }

                o_item = old.next();
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
                                    patch_set.push(Patch::CreateNode { store, element: n_element });
                                    state.push(NodeState::Create);
                                }
                                (Some(o_elem), None) => {
                                    patch_set.push(Patch::CopyNode { store: store, node: o_elem.clone() });
                                    state.push(NodeState::Copy);
                                }
                                // just diff the existing nodes
                                (Some(_), Some(_)) => {}
                                // this shouldn't happen, but is harmless if it does
                                (None, Some(_)) => {}
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
                            patch_set.push(Patch::CreateNode { store, element: n_element });
                            state.push(NodeState::Create);
                            
                            // skip the rest of the items in the old tree for this element, this
                            // will cause attributes and such to be created on the new element
                            loop {
                                o_item = old.next();
                                match o_item.take() {
                                    Some(DomItem::Node { node: _, element: _, store: _ }) => {
                                        state.push(NodeState::OldChild);
                                    }
                                    Some(DomItem::Up) if state.is_child() => {
                                        state.pop();
                                    }
                                    o @ Some(DomItem::Up) | o @ None => {
                                        o_item = o;
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            n_item = new.next();
                        }
                    }
                    (
                        DomItem::Attr { name: o_name, value: o_value },
                        DomItem::Attr { name: n_name, value: n_value }
                    ) => { // compare attributes
                        if state.is_create() {
                            // add attribute
                            patch_set.push(Patch::AddAttribute { name: n_name, value: n_value });
                        }
                        if o_name != n_name || o_value != n_value {
                            if state.is_copy() {
                                // remove old attribute
                                patch_set.push(Patch::RemoveAttribute(o_name));
                            }
                            if !state.is_create() {
                                // add new attribute
                                patch_set.push(Patch::AddAttribute { name: n_name, value: n_value });
                            }
                        }
                        o_item = old.next();
                        n_item = new.next();
                    }
                    (
                        DomItem::Event { trigger: o_trigger, handler: o_handler },
                        DomItem::Event { trigger: n_trigger, handler: n_handler }
                    ) => { // compare event listeners
                        if state.is_create() {
                            // add listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.clone() });
                        }
                        if o_trigger != n_trigger || o_handler != n_handler {
                            if state.is_copy() {
                                // remove old listener
                                patch_set.push(Patch::RemoveListener(o_trigger));
                            }
                            if !state.is_create() {
                                // add new listener
                                patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.clone() });
                            }
                        }
                        o_item = old.next();
                        n_item = new.next();
                    }
                    (o @ DomItem::Up, n @ DomItem::Up) => { // end of two items
                        // don't advance old if we are iterating through new's children
                        if !state.is_new_child() {
                            o_item = old.next();
                        }
                        else {
                            o_item = Some(o);
                        }
                        // don't advance new if we are iterating through old's children
                        if !state.is_old_child() {
                            patch_set.push(Patch::Up);
                            n_item = new.next();
                        }
                        else {
                            n_item = Some(n);
                        }

                        state.pop();
                    }
                    // add a new child node
                    (o, DomItem::Node { node: _, element, store }) => {
                        patch_set.push(Patch::CreateNode { store, element });
                        state.push(NodeState::NewChild);
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // add attribute to new node
                    (o, DomItem::Attr { name, value }) => {
                        patch_set.push(Patch::AddAttribute { name, value });
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // add event to new node
                    (o, DomItem::Event { trigger, handler }) => {
                        patch_set.push(Patch::AddListener { trigger, handler: handler.clone() });
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // remove the old node if present
                    (DomItem::Node { node: Some(node), element: _, store: _ }, n) => {
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveNode(node.clone()));
                        }
                        state.push(NodeState::OldChild);
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // just iterate through the old node
                    (DomItem::Node { node: None, element: _, store: _ }, n) => {
                        state.push(NodeState::OldChild);
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // remove attribute from old node
                    (DomItem::Attr { name, value: _ }, n) => {
                        if state.is_copy() {
                            patch_set.push(Patch::RemoveAttribute(name));
                        }
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // remove event from old node
                    (DomItem::Event { trigger, handler: _ }, n) => {
                        if state.is_copy() {
                            patch_set.push(Patch::RemoveListener(trigger));
                        }
                        o_item = old.next();
                        n_item = Some(n);
                    }
                }
            }
        }
    }

    patch_set
}

fn patch<'a, Message>(parent: web_sys::Element, patch_set: PatchSet<'a, Message>) {
    let mut node_stack = vec![parent];

    let document = web_sys::window().expect("expected window")
        .document().expect("expected document");

    for p in patch_set.into_iter() {
        match p {
            Patch::RemoveNode(node) => {
                node_stack.last()
                    .unwrap()
                    .remove_child(&node)
                    .expect("failed to remove child node");
            }
            Patch::CreateNode { mut store, element } => {
                let node = document.create_element(element).expect("failed to create element");
                store(node.clone());
                node_stack.last()
                    .unwrap()
                    .append_child(&node)
                    .expect("failed to append child node");
                node_stack.push(node);
            }
            Patch::CopyNode { mut store, node } => {
                store(node.clone());
                node_stack.push(node);
            }
            Patch::AddAttribute { name, value } => {
                node_stack.last()
                    .unwrap()
                    .set_attribute(name, value)
                    .expect("failed to set attribute");
            }
            Patch::RemoveAttribute(name) => {
                node_stack.last()
                    .unwrap()
                    .remove_attribute(name)
                    .expect("failed to remove attribute");
            }
            Patch::AddListener { trigger, handler } => {
                // XXX call add_event_listener_with_callback()
            }
            Patch::RemoveListener(trigger) => {
                // XXX call remove_event_listener_with_callback()
            }
            Patch::Up => {
                node_stack.pop();
            }
        }
    }
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
                Patch::CreateNode { store: Box::new(|_|()), element: "div" },
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
                Patch::CreateNode { store: Box::new(|_|()), element: "span" },
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
                Patch::CreateNode { store: Box::new(|_|()), element: "div" },
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
                Patch::CreateNode { store: Box::new(|_|()), element: "div" },
                Patch::CreateNode { store: Box::new(|_|()), element: "b" },
                Patch::AddAttribute { name: "class", value: "item" },
                Patch::AddAttribute { name: "id", value: "id1" },
                Patch::AddListener { trigger: "onclick", handler: super::EventHandler::Msg(&Msg {}) },
                Patch::Up,
                Patch::CreateNode { store: Box::new(|_|()), element: "i" },
                Patch::AddAttribute { name: "class", value: "item" },
                Patch::AddAttribute { name: "id", value: "id2" },
                Patch::AddListener { trigger: "onclick", handler: super::EventHandler::Msg(&Msg {}) },
                Patch::Up,
                Patch::Up,
            ]
        );
    }

    #[test]
    fn diff_old_child_nodes() {
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
                Patch::CreateNode { store: Box::new(|_|()), element: "div" },
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
                Patch::CreateNode { store: Box::new(|_|()), element: "span" },
                Patch::Up,
            ]
        );
    }

    #[wasm_bindgen_test]
    fn old_child_nodes_with_element() {
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
                    node: Some(e("b")),
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
                    node: Some(e("i")),
                },
            ],
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
                Patch::RemoveNode(e("b")),
                Patch::RemoveNode(e("i")),
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
                        Event { trigger: "onclick".into(), handler: EventHandler::Msg(Msg {}) },
                    ],
                    children: vec![],
                    node: Some(e("b")),
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
                    node: Some(e("i")),
                },
            ],
            node: Some(e("span")),
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
                Patch::RemoveNode(e("span")),
                Patch::CreateNode { store: Box::new(|_|()), element: "div" },
                Patch::Up,
            ]
        );
    }

}
