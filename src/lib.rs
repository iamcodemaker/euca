#[deny(missing_docs)]

use web_sys;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::fmt;
use std::cmp;
use std::rc::Rc;

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
enum EventHandler<Message> {
    Msg(Message),
    Fn(fn(web_sys::Event) -> Message),
}

enum Storage<'a, T> {
    Read(Option<T>),
    Write(Box<FnMut(T) + 'a>),
}

impl<'a, T> fmt::Debug for Storage<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Storage::Read(_) => write!(f, "Read(_)"),
            Storage::Write(_) => write!(f, "Write(_)"),
        }
    }
}

/// Items representing all of the data in the DOM tree.
///
/// This is the struct emitted from the `Iterator` passed to our `diff` function. The items emitted
/// should always be in the same order, given the same input. Each entry in the enum represents
/// some aspect of a DOM node. The idea here is the sequence of items will be the same sequence of
/// things seen if we were to walk the DOM tree depth first going through all nodes and their
/// various attributes and events.
#[derive(Debug)]
enum DomItem<'a, Message> {
    /// An element in the tree.
    Element { element: &'a str, node: Storage<'a, web_sys::Element> },
    /// A text node in the tree.
    Text { text: String, node: Storage<'a, web_sys::Text> },
    /// An attribute of the last node we saw.
    Attr { name: &'a str, value: &'a str },
    /// An event handler from the last node we saw.
    Event { trigger: String, handler: EventHandler<Message>, closure: Storage<'a, Closure<FnMut(web_sys::Event)>> },
    /// We are finished processing children nodes, the next node is a sibling.
    Up,
}

impl<'a, Message> cmp::PartialEq for DomItem<'a, Message>
where
    Message: cmp::PartialEq
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                DomItem::Element { node: _, element: e1 },
                DomItem::Element { node: _, element: e2 }
            )
            => e1 == e2,
            (
                DomItem::Text { node: _, text: t1 },
                DomItem::Text { node: _, text: t2 }
            )
            => t1 == t2,
            (
                DomItem::Attr { name: n1, value: v1 },
                DomItem::Attr { name: n2, value: v2 }
            )
            => n1 == n2 && v1 == v2,
            (
                DomItem::Event { trigger: t1, handler: h1, closure: _ },
                DomItem::Event { trigger: t2, handler: h2, closure: _ }
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
    handler: EventHandler<Message>,
}

trait DomTree<'a, Message> {
    fn element() -> &'a str;
    fn attributes() -> Iterator<Item = Attr<'a>>;
    fn events() -> Iterator<Item = Event<'a, Message>>;
//    fn children() -> Iterator<Item = DomTree<'a, Message>>;
}

enum Patch<'a, Message> {
    RemoveElement(web_sys::Element),
    CreateElement { store: Box<FnMut(web_sys::Element) + 'a>, element: &'a str },
    CopyElement { store: Box<FnMut(web_sys::Element) + 'a>, node: web_sys::Element },
    RemoveText(web_sys::Text),
    CreateText { store: Box<FnMut(web_sys::Text) + 'a>, text: String },
    CopyText { store: Box<FnMut(web_sys::Text) + 'a>, node: web_sys::Text },
    AddAttribute { name: &'a str, value: &'a str },
    RemoveAttribute(&'a str),
    AddListener { trigger: String, handler: EventHandler<Message>, store: Box<FnMut(Closure<FnMut(web_sys::Event)>) + 'a> },
    RemoveListener { trigger: String, closure: Closure<FnMut(web_sys::Event)> },
    Up,
}

impl<'a, Message> fmt::Debug for Patch<'a, Message>
where
    Message: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Patch::RemoveElement(_) => write!(f, "RemoveElement(_)"),
            Patch::CreateElement { store: _, element: s } => write!(f, "CreateElement {{ store: _, element: {:?} }}", s),
            Patch::CopyElement { store: _, node: _ } => write!(f, "CopyElement {{ store: _, node: _ }}"),
            Patch::RemoveText(_) => write!(f, "RemoveText(_)"),
            Patch::CreateText { store: _, text: t } => write!(f, "CreateText {{ store: _, text: {:?} }}", t),
            Patch::CopyText { store: _, node: _ } => write!(f, "CopyText {{ store: _, node: _ }}"),
            Patch::AddAttribute { name: n, value: v } => write!(f, "AddAttribute {{ name: {:?}, value: {:?} }}", n, v),
            Patch::RemoveAttribute(s) => write!(f, "RemoveAttribute({:?})", s),
            Patch::AddListener { trigger: t, handler: h, store: _ } => write!(f, "AddListener {{ trigger: {:?}, handler: {:?}, store: _ }}", t, h),
            Patch::RemoveListener { trigger: t, closure: _ } => write!(f, "RemoveListener {{ trigger: {:?}), closure: _ }}", t),
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
            (Patch::RemoveElement(_), Patch::RemoveElement(_)) => true,
            (
                Patch::CreateElement { store: _, element: e1 },
                Patch::CreateElement { store: _, element: e2 },
            )
            => e1 == e2,
            (
                Patch::CopyElement { store: _, node: _ },
                Patch::CopyElement { store: _, node: _ },
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
                Patch::AddListener { trigger: t1, handler: h1, .. },
                Patch::AddListener { trigger: t2, handler: h2, .. },
            )
            => t1 == t2 && h1 == h2,
            (
                Patch::RemoveListener { trigger: t1, .. },
                Patch::RemoveListener { trigger: t2, .. },
            )
            => t1 == t2,
            (Patch::Up, Patch::Up) => true,
            (_, _) => false,
        }
    }
}

type PatchSet<'a, Message> = Vec<Patch<'a, Message>>;

fn diff<'a, Message, I>(old: &mut I, new: &mut I) -> PatchSet<'a, Message>
where
    Message: 'a + PartialEq + Clone + fmt::Debug,
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
                    DomItem::Element { node: Storage::Write(store), element } => {
                        patch_set.push(Patch::CreateElement { store, element });
                    }
                    DomItem::Text { node: Storage::Write(store), text } => {
                        patch_set.push(Patch::CreateText { store, text });
                    }
                    DomItem::Attr { name, value } => {
                        patch_set.push(Patch::AddAttribute { name, value });
                    }
                    DomItem::Event { trigger, handler, closure: Storage::Write(store) } => {
                        patch_set.push(Patch::AddListener { trigger, handler: handler.into(), store });
                    }
                    DomItem::Up => {
                        patch_set.push(Patch::Up);
                    }
                    DomItem::Element { node: Storage::Read(_), .. } => {
                        panic!("new node should not have Storage::Read(_)");
                    }
                    DomItem::Event { closure: Storage::Read(_), .. } => {
                        panic!("new event should not have Storage::Read(_)");
                    }
                    DomItem::Text { node: Storage::Read(_), .. } => {
                        panic!("new text should not have Storage::Read(_)");
                    }
                }

                n_item = new.next();
            }
            (Some(o), None) => { // delete remaining old nodes
                match o {
                    DomItem::Element { node: Storage::Read(mut node @ Some(_)), .. } => {
                        // ignore child nodes
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveElement(node.take().unwrap()));
                        }

                        state.push(NodeState::OldChild);
                    }
                    DomItem::Element { node: Storage::Write(_), .. } => {
                        panic!("old node should not have Storage::Write(_)");
                    }
                    DomItem::Text { node: Storage::Read(mut node @ Some(_)), .. } => {
                        // ignore child nodes
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveText(node.take().unwrap()));
                        }

                        state.push(NodeState::OldChild);
                    }
                    DomItem::Text { node: Storage::Write(_), .. } => {
                        panic!("old text should not have Storage::Write(_)");
                    }
                    DomItem::Up => {
                        state.pop();
                    }
                    // XXX do we need to remove events?
                    DomItem::Event { .. } => {}
                    // ignore attributes
                    DomItem::Attr { .. } => {}
                    // this shouldn't happen
                    DomItem::Element { node: Storage::Read(None), .. } => {
                        panic!("Storage::Read should never be None");
                    }
                    // this shouldn't happen
                    DomItem::Text { node: Storage::Read(None), .. } => {
                        panic!("Storage::Read should never be None");
                    }
                }

                o_item = old.next();
            }
            (Some(o), Some(n)) => { // compare nodes
                match (o, n) {
                    (
                        DomItem::Element { node: Storage::Read(mut node @ Some(_)), element: o_element },
                        DomItem::Element { node: Storage::Write(store), element: n_element }
                    ) => { // compare elements
                        // if the elements match, use the web_sys::Element
                        if o_element == n_element {
                            // copy the node
                            patch_set.push(Patch::CopyElement { store: store, node: node.take().unwrap() });
                            state.push(NodeState::Copy);

                            o_item = old.next();
                            n_item = new.next();
                        }
                        // elements don't match, remove the old and make a new one
                        else {
                            patch_set.push(Patch::RemoveElement(node.take().unwrap()));
                            patch_set.push(Patch::CreateElement { store, element: n_element });
                            state.push(NodeState::Create);
                            
                            // skip the rest of the items in the old tree for this element, this
                            // will cause attributes and such to be created on the new element
                            loop {
                                o_item = old.next();
                                match o_item.take() {
                                    Some(DomItem::Element { .. }) => {
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
                        DomItem::Text { node: Storage::Read(mut node @ Some(_)), text: o_text },
                        DomItem::Text { node: Storage::Write(store), text: n_text }
                    ) => { // compare text
                        // if the text matches, use the web_sys::Text
                        if o_text == n_text {
                            // copy the node
                            patch_set.push(Patch::CopyText { store: store, node: node.take().unwrap() });
                            state.push(NodeState::Copy);

                            o_item = old.next();
                            n_item = new.next();
                        }
                        // elements don't match, remove the old and make a new one
                        else {
                            patch_set.push(Patch::RemoveText(node.take().unwrap()));
                            patch_set.push(Patch::CreateText { store, text: n_text });
                            state.push(NodeState::Create);
                            
                            // skip the rest of the items in the old tree for this element, this
                            // will cause attributes and such to be created on the new element
                            loop {
                                o_item = old.next();
                                match o_item.take() {
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
                        DomItem::Event { trigger: o_trigger, handler: o_handler, closure: Storage::Read(mut closure) },
                        DomItem::Event { trigger: n_trigger, handler: n_handler, closure: Storage::Write(store) }
                    ) => { // compare event listeners
                        if o_trigger != n_trigger || o_handler != n_handler {
                            if state.is_copy() {
                                // remove old listener
                                patch_set.push(Patch::RemoveListener { trigger: o_trigger, closure: closure.take().unwrap() });
                            }
                            // add new listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into(), store });
                        }
                        else if state.is_create() {
                            // add listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into(), store });
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
                    (o, DomItem::Element { node: Storage::Write(store), element }) => {
                        patch_set.push(Patch::CreateElement { store, element });
                        state.push(NodeState::NewChild);
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // invalid
                    (_, DomItem::Element { node: Storage::Read(_), .. }) => {
                        panic!("new node should not have Storage::Read(_)");
                    }
                    // add a new text node
                    (o, DomItem::Text { node: Storage::Write(store), text }) => {
                        patch_set.push(Patch::CreateText { store, text });
                        state.push(NodeState::NewChild);
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // invalid
                    (_, DomItem::Text { node: Storage::Read(_), .. }) => {
                        panic!("new text should not have Storage::Read(_)");
                    }
                    // add attribute to new node
                    (o, DomItem::Attr { name, value }) => {
                        patch_set.push(Patch::AddAttribute { name, value });
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // add event to new node
                    (o, DomItem::Event { trigger, handler, closure: Storage::Write(store) }) => {
                        patch_set.push(Patch::AddListener { trigger, handler: handler.into(), store });
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // invalid
                    (_, DomItem::Event { closure: Storage::Read(_), .. }) => {
                        panic!("new event should not have Storage::Read(_)");
                    }
                    // remove the old node if present
                    (DomItem::Element { node: Storage::Read(mut node @ Some(_)), .. }, n) => {
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveElement(node.take().unwrap()));
                        }
                        state.push(NodeState::OldChild);
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // invalid
                    (DomItem::Element { node: Storage::Write(_), .. }, _) => {
                        panic!("old node should not have Storage::Write(_)");
                    }
                    // invalid
                    (DomItem::Element { node: Storage::Read(None), .. }, _) => {
                        panic!("Storage::Read should never be None");
                    }
                    // remove the old text if present
                    (DomItem::Text { node: Storage::Read(mut node @ Some(_)), .. }, n) => {
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveText(node.take().unwrap()));
                        }
                        state.push(NodeState::OldChild);
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // invalid
                    (DomItem::Text { node: Storage::Write(_), .. }, _) => {
                        panic!("old text should not have Storage::Write(_)");
                    }
                    // invalid
                    (DomItem::Text { node: Storage::Read(None), .. }, _) => {
                        panic!("Storage::Read should never be None");
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
                    (DomItem::Event { trigger, closure: Storage::Read(mut closure @ Some(_)), .. }, n) => {
                        if state.is_copy() {
                            patch_set.push(Patch::RemoveListener { trigger, closure: closure.take().unwrap() });
                        }
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // invalid
                    (DomItem::Event { closure: Storage::Write(_), .. }, _) => {
                        panic!("old event should not have Storage::Write(_)");
                    }
                    // invalid
                    (DomItem::Event { closure: Storage::Read(None), .. }, _) => {
                        panic!("Storage::Read should never be None");
                    }
                }
            }
        }
    }

    patch_set
}

fn patch<'a, Message>(parent: web_sys::Element, patch_set: PatchSet<'a, Message>, dispatch: Rc<Box<Fn(Message) + 'static>>)
where
    Message: 'static + Clone,
    EventHandler<Message>: Clone,
{

    let mut node_stack: Vec<web_sys::Node> = vec![parent.unchecked_into()];

    let document = web_sys::window().expect("expected window")
        .document().expect("expected document");

    for p in patch_set.into_iter() {
        match p {
            Patch::RemoveElement(node) => {
                node_stack.last()
                    .expect("no previous node")
                    .remove_child(&node)
                    .expect("failed to remove child node");
            }
            Patch::CreateElement { mut store, element } => {
                let node = document.create_element(element).expect("failed to create element");
                store(node.clone());
                node_stack.last()
                    .expect("no previous node")
                    .append_child(&node)
                    .expect("failed to append child node");
                node_stack.push(node.into());
            }
            Patch::CopyElement { mut store, node } => {
                store(node.clone());
                node_stack.push(node.into());
            }
            Patch::RemoveText(node) => {
                node_stack.last()
                    .expect("no previous node")
                    .remove_child(&node)
                    .expect("failed to remove child node");
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
            Patch::CopyText { mut store, node } => {
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
                let dispatch = dispatch.clone();
                let closure = match handler {
                    EventHandler::Msg(msg) => {
                        Closure::wrap(
                            Box::new(move |_| {
                                dispatch(msg.clone())
                            }) as Box<FnMut(web_sys::Event)>
                        )
                    }
                    EventHandler::Fn(fun) => {
                        Closure::wrap(
                            Box::new(move |event| {
                                dispatch(fun(event))
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
            Patch::RemoveListener { trigger, closure } => {
                let node = node_stack.last().expect("no previous node");
                (node.as_ref() as &web_sys::EventTarget)
                    .remove_event_listener_with_callback(&trigger, closure.as_ref().unchecked_ref())
                    .expect("failed to remove event listener");
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

    impl<Message> Dom<Message> {

        fn dom(&mut self) -> Vec<DomItem<Message>>
        where
            Message: Clone,
        {
            use std::iter;

            // until generators are stable, this is the best we can do
            iter::once((&mut self.node, &self.element))
                .map(|(node, element)| DomItem::Element {
                    element: element,
                    node: match node {
                        Some(_) => Storage::Read(node.clone()),
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
                .map(|e|
                     DomItem::Event {
                         trigger: e.trigger.to_owned(),
                         handler: match e.handler {
                             EventHandler::Msg(ref m) => super::EventHandler::Msg(m.clone()),
                             EventHandler::Map(f) => super::EventHandler::Fn(f),
                         },
                         closure: match e.closure {
                             Some(_) => Storage::Read(e.closure.take()),
                             None => Storage::Write(Box::new(move |c| e.closure = Some(c))),
                         },
                     }
                 )
            )
            .chain(self.children.iter_mut()
               .flat_map(|c| Dom::dom(c))
            )
            .chain(self.text.iter_mut()
               .flat_map(|t|
                   vec![
                       DomItem::Text {
                           text: t.text.clone(),
                           node: match t.node {
                               Some(_) => Storage::Read(t.node.clone()),
                               None => Storage::Write(Box::new(move |n| t.node = Some(n))),
                           },
                       },
                       // this is necessary because text nodes can have events associated with them
                       DomItem::Up,
                   ]
               )
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
                    (Patch::CreateElement { store: _, element: e1 }, Patch::CreateElement { store: _, element: e2 }) => {
                        assert_eq!(e1, e2, "unexpected CreateElement");
                    }
                    (Patch::CopyElement { store: _, node: _ }, Patch::CopyElement { store: _, node: _ }) => {}
                    (Patch::AddAttribute { name: n1, value: v1 }, Patch::AddAttribute { name: n2, value: v2 }) => {
                        assert_eq!(n1, n2, "attribute names don't match");
                        assert_eq!(v1, v2, "attribute values don't match");
                    }
                    (Patch::CreateText { store: _, text: t1 }, Patch::CreateText { store: _, text: t2 }) => {
                        assert_eq!(t1, t2, "unexpected CreateText");
                    }
                    (Patch::CopyText { store: _, node: _ }, Patch::CopyText { store: _, node: _ }) => {}
                    (Patch::RemoveAttribute(a1), Patch::RemoveAttribute(a2)) => {
                        assert_eq!(a1, a2, "attribute names don't match");
                    }
                    (Patch::AddListener { trigger: t1, handler: h1, store: _ }, Patch::AddListener { trigger: t2, handler: h2, store: _ }) => {
                        assert_eq!(t1, t2, "trigger names don't match");
                        assert_eq!(h1, h2, "handlers don't match");
                    }
                    (Patch::RemoveListener { trigger: t1, closure: _ }, Patch::RemoveListener { trigger: t2, closure: _ }) => {
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
        let old: Vec<DomItem<Msg>> = vec![];

        let mut new: Dom<Msg> = Dom {
            element: "span".into(),
            attributes: vec!(),
            events: vec!(),
            children: vec!(),
            text: None,
            node: None,
        };

        let mut o = old.into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CreateElement { store: Box::new(|_|()), element: "span" },
                Patch::Up,
            ]
        );
    }

    #[test]
    fn diff_add_text() {
        let old: Vec<DomItem<Msg>> = vec![];

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

        let mut o = old.into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CreateElement { store: Box::new(|_|()), element: "div" },
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

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CopyElement { store: Box::new(|_|()), node: e("div") },
                Patch::CreateElement { store: Box::new(|_|()), element: "b" },
                Patch::AddAttribute { name: "class", value: "item" },
                Patch::AddAttribute { name: "id", value: "id1" },
                Patch::AddListener { trigger: "onclick".to_owned(), handler: super::EventHandler::Msg(Msg {}), store: Box::new(|_|()) },
                Patch::Up,
                Patch::CreateElement { store: Box::new(|_|()), element: "i" },
                Patch::AddAttribute { name: "class", value: "item" },
                Patch::AddAttribute { name: "id", value: "id2" },
                Patch::AddListener { trigger: "onclick".to_owned(), handler: super::EventHandler::Msg(Msg {}), store: Box::new(|_|()) },
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

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CopyElement { store: Box::new(|_|()), node: e("div") },
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

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::RemoveElement(e("div")),
                Patch::CreateElement { store: Box::new(|_|()), element: "span" },
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

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::CopyElement { store: Box::new(|_|()), node: e("div") },
                Patch::RemoveElement(e("b")),
                Patch::RemoveElement(e("i")),
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

        let mut o = old.dom().into_iter();
        let mut n = new.dom().into_iter();
        let patch_set = diff(&mut o, &mut n);

        compare!(
            patch_set,
            [
                Patch::RemoveElement(e("span")),
                Patch::CreateElement { store: Box::new(|_|()), element: "div" },
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

        {
            let mut o = old.dom().into_iter();
            let mut n = new.dom().into_iter();
            let patch_set = diff(&mut o, &mut n);

            let parent = e("div");
            let dispatch = Rc::new(Box::new(move |_|()) as Box<Fn(Msg)>);
            patch(parent.clone(), patch_set, dispatch.clone());
        }

        assert!(new.node.is_some(), "expected node to be copied");
    }

    #[wasm_bindgen_test]
    fn basic_patch_with_element() {
        let gen1: Vec<DomItem<Msg>> = vec![];

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
        let dispatch = Rc::new(Box::new(move |_|()) as Box<Fn(Msg)>);

        {
            // first gen create element
            let mut o = gen1.into_iter();
            let mut n = gen2.dom().into_iter();
            let patch_set = diff(&mut o, &mut n);
            patch(parent.clone(), patch_set, dispatch.clone());
        }

        assert!(gen2.node.is_some(), "expected node to be created");

        {
            // second gen remove and replace element
            let mut o = gen2.dom().into_iter();
            let mut n = gen3.dom().into_iter();
            let patch_set = diff(&mut o, &mut n);
            patch(parent.clone(), patch_set, dispatch.clone());
        }

        assert!(gen3.node.is_some(), "expected node to be created");
    }
}
