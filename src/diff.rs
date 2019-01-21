//! Tools to get the difference between two virtual dom trees.

use std::fmt;
use crate::patch::PatchSet;
use crate::patch::Patch;
use crate::dom::DomItem;
use crate::dom::Storage;

/// Return the series of steps required to move from the given old/existing virtual dom to the
/// given new virtual dom.
pub fn diff<'a, Message, I1, I2>(mut old: I1, mut new: I2) -> PatchSet<'a, Message> where
    Message: 'a + PartialEq + Clone + fmt::Debug,
    I1: Iterator<Item = DomItem<'a, Message>>,
    I2: Iterator<Item = DomItem<'a, Message>>,
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
                    DomItem::Element { node: Storage::Read(take), .. } => {
                        // ignore child nodes
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveElement(take));
                        }

                        state.push(NodeState::OldChild);
                    }
                    DomItem::Element { node: Storage::Write(_), .. } => {
                        panic!("old node should not have Storage::Write(_)");
                    }
                    DomItem::Text { node: Storage::Read(take), .. } => {
                        // ignore child nodes
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveText(take));
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
                }

                o_item = old.next();
            }
            (Some(o), Some(n)) => { // compare nodes
                match (o, n) {
                    (
                        DomItem::Element { node: Storage::Read(take), element: o_element },
                        DomItem::Element { node: Storage::Write(store), element: n_element }
                    ) => { // compare elements
                        // if the elements match, use the web_sys::Element
                        if o_element == n_element {
                            // copy the node
                            patch_set.push(Patch::CopyElement { store, take });
                            state.push(NodeState::Copy);

                            o_item = old.next();
                            n_item = new.next();
                        }
                        // elements don't match, remove the old and make a new one
                        else {
                            patch_set.push(Patch::RemoveElement(take));
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
                                    // XXX do we need special handling for events?
                                    _ => {}
                                }
                            }
                            n_item = new.next();
                        }
                    }
                    (
                        DomItem::Text { node: Storage::Read(take), text: o_text },
                        DomItem::Text { node: Storage::Write(store), text: n_text }
                    ) => { // compare text
                        // if the text matches, use the web_sys::Text
                        if o_text == n_text {
                            // copy the node
                            patch_set.push(Patch::CopyText { store, take });
                            state.push(NodeState::Copy);
                        }
                        // text doesn't match, update it
                        else {
                            patch_set.push(Patch::ReplaceText { store, take, text: n_text });
                            state.push(NodeState::Copy);
                        }
                        o_item = old.next();
                        n_item = new.next();
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
                        DomItem::Event { trigger: o_trigger, handler: o_handler, closure: Storage::Read(take) },
                        DomItem::Event { trigger: n_trigger, handler: n_handler, closure: Storage::Write(store) }
                    ) => { // compare event listeners
                        if o_trigger != n_trigger || o_handler != n_handler {
                            if state.is_copy() {
                                // remove old listener
                                patch_set.push(Patch::RemoveListener { trigger: o_trigger, take });
                            }
                            // add new listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into(), store });
                        }
                        else if state.is_create() {
                            // add listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into(), store });
                        }
                        else {
                            // just copy the existing listener
                            patch_set.push(Patch::CopyListener { store, take });
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
                    (DomItem::Element { node: Storage::Read(take), .. }, n) => {
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveElement(take));
                        }
                        state.push(NodeState::OldChild);
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // invalid
                    (DomItem::Element { node: Storage::Write(_), .. }, _) => {
                        panic!("old node should not have Storage::Write(_)");
                    }
                    // remove the old text if present
                    (DomItem::Text { node: Storage::Read(take), .. }, n) => {
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveText(take));
                        }
                        state.push(NodeState::OldChild);
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // invalid
                    (DomItem::Text { node: Storage::Write(_), .. }, _) => {
                        panic!("old text should not have Storage::Write(_)");
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
                    (DomItem::Event { trigger, closure: Storage::Read(take), .. }, n) => {
                        if state.is_copy() {
                            patch_set.push(Patch::RemoveListener { trigger, take: take });
                        }
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // invalid
                    (DomItem::Event { closure: Storage::Write(_), .. }, _) => {
                        panic!("old event should not have Storage::Write(_)");
                    }
                }
            }
        }
    }

    patch_set
}
