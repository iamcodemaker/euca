//! Tools to get the difference between two virtual dom trees.

use std::fmt;
use std::mem;
use crate::patch::PatchSet;
use crate::patch::Patch;
use crate::vdom::DomItem;
use crate::vdom::Storage;
use crate::vdom::WebItem;

/// Return the series of steps required to move from the given old/existing virtual dom to the
/// given new virtual dom.
pub fn diff<'a, Message, I1, I2>(mut old: I1, mut new: I2, storage: &'a mut Storage) -> PatchSet<'a, Message> where
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
    let mut sto = storage.iter_mut();

    loop {
        match (o_item.take(), n_item.take()) {
            (None, None) => { // return patch set
                break;
            }
            (None, Some(n)) => { // create remaining new nodes
                match n {
                    DomItem::Element { element } => {
                        patch_set.push(Patch::CreateElement { element });
                    }
                    DomItem::Text { text } => {
                        patch_set.push(Patch::CreateText { text });
                    }
                    DomItem::Attr { name, value } => {
                        patch_set.push(Patch::SetAttribute { name, value });
                    }
                    DomItem::Event { trigger, handler } => {
                        patch_set.push(Patch::AddListener { trigger, handler: handler.into() });
                    }
                    DomItem::Up => {
                        patch_set.push(Patch::Up);
                    }
                }

                n_item = new.next();
            }
            (Some(o), None) => { // delete remaining old nodes
                match o {
                    DomItem::Element { .. } => {
                        let web_item = sto.next().expect("dom storage to match dom iter");
                        let take = Box::new(move || {
                            let mut taken_item = WebItem::Taken;
                            mem::swap(web_item, &mut taken_item);
                            match taken_item {
                                WebItem::Element(i) => i,
                                _ => panic!("storage type mismatch"),
                            }
                        });

                        // ignore child nodes
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveElement(take));
                        }

                        state.push(NodeState::OldChild);
                    }
                    DomItem::Text { .. } => {
                        let web_item = sto.next().expect("dom storage to match dom iter");
                        let take = Box::new(move || {
                            let mut taken_item = WebItem::Taken;
                            mem::swap(web_item, &mut taken_item);
                            match taken_item {
                                WebItem::Text(i) => i,
                                _ => panic!("storage type mismatch"),
                            }
                        });

                        // ignore child nodes
                        if !state.is_child() {
                            patch_set.push(Patch::RemoveText(take));
                        }

                        state.push(NodeState::OldChild);
                    }
                    DomItem::Up => {
                        state.pop();
                    }
                    DomItem::Event { .. } => {
                        let _ = sto.next().expect("dom storage to match dom iter");
                    }
                    // ignore attributes
                    DomItem::Attr { .. } => {}
                }

                o_item = old.next();
            }
            (Some(o), Some(n)) => { // compare nodes
                match (o, n) {
                    (
                        DomItem::Element { element: o_element },
                        DomItem::Element { element: n_element }
                    ) => { // compare elements
                        let web_item = sto.next().expect("dom storage to match dom iter");
                        let take = Box::new(move || {
                            let mut taken_item = WebItem::Taken;
                            mem::swap(web_item, &mut taken_item);
                            match taken_item {
                                WebItem::Element(i) => i,
                                _ => panic!("storage type mismatch"),
                            }
                        });

                        // if the elements match, use the web_sys::Element
                        if o_element == n_element {
                            // copy the node
                            patch_set.push(Patch::CopyElement { take });
                            state.push(NodeState::Copy);

                            o_item = old.next();
                            n_item = new.next();
                        }
                        // elements don't match, remove the old and make a new one
                        else {
                            patch_set.push(Patch::RemoveElement(take));
                            patch_set.push(Patch::CreateElement { element: n_element });
                            state.push(NodeState::Create);
                            
                            // skip the rest of the items in the old tree for this element, this
                            // will cause attributes and such to be created on the new element
                            loop {
                                o_item = old.next();
                                match o_item.take() {
                                    Some(DomItem::Element { .. }) => {
                                        let _ = sto.next().expect("dom storage to match dom iter");
                                        state.push(NodeState::OldChild);
                                    }
                                    Some(DomItem::Up) if state.is_child() => {
                                        state.pop();
                                    }
                                    Some(DomItem::Text { .. }) | Some(DomItem::Event { .. }) => {
                                        let _ = sto.next().expect("dom storage to match dom iter");
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
                        DomItem::Text { text: o_text },
                        DomItem::Text { text: n_text }
                    ) => { // compare text
                        let web_item = sto.next().expect("dom storage to match dom iter");
                        let take = Box::new(move || {
                            let mut taken_item = WebItem::Taken;
                            mem::swap(web_item, &mut taken_item);
                            match taken_item {
                                WebItem::Text(i) => i,
                                _ => panic!("storage type mismatch"),
                            }
                        });

                        // if the text matches, use the web_sys::Text
                        if o_text == n_text {
                            // copy the node
                            patch_set.push(Patch::CopyText { take });
                            state.push(NodeState::Copy);
                        }
                        // text doesn't match, update it
                        else {
                            patch_set.push(Patch::ReplaceText { take, text: n_text });
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
                            patch_set.push(Patch::SetAttribute { name: n_name, value: n_value });
                        }
                        // names are different
                        else if o_name != n_name {
                            if state.is_copy() {
                                // remove old attribute
                                patch_set.push(Patch::RemoveAttribute(o_name));
                            }
                            // add new attribute
                            patch_set.push(Patch::SetAttribute { name: n_name, value: n_value });
                        }
                        // only values are different
                        else if o_value != n_value {
                            // set new attribute value
                            patch_set.push(Patch::SetAttribute { name: n_name, value: n_value });
                        }
                        // values are the same, check for special attributes. These are attributes
                        // attributes that the browser can change as the result of user actions, so
                        // we won't detect that if we only go by the state of the vdom. To work
                        // around that, we just always set these.
                        else {
                            match n_name {
                                "checked" | "selected" | "spellcheck" => {
                                    patch_set.push(Patch::SetAttribute { name: n_name, value: n_value })
                                }
                                _ => {}
                            }
                        }
                        o_item = old.next();
                        n_item = new.next();
                    }
                    (
                        DomItem::Event { trigger: o_trigger, handler: o_handler },
                        DomItem::Event { trigger: n_trigger, handler: n_handler }
                    ) => { // compare event listeners
                        let web_item = sto.next().expect("dom storage to match dom iter");
                        let take = Box::new(move || {
                            let mut taken_item = WebItem::Taken;
                            mem::swap(web_item, &mut taken_item);
                            match taken_item {
                                WebItem::Closure(i) => i,
                                _ => panic!("storage type mismatch"),
                            }
                        });

                        if o_trigger != n_trigger || o_handler != n_handler {
                            if state.is_copy() {
                                // remove old listener
                                patch_set.push(Patch::RemoveListener { trigger: o_trigger, take });
                            }
                            // add new listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into() });
                        }
                        else if state.is_create() {
                            // add listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into() });
                        }
                        else {
                            // just copy the existing listener
                            patch_set.push(Patch::CopyListener { take });
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
                    (o, DomItem::Element { element }) => {
                        patch_set.push(Patch::CreateElement { element });
                        state.push(NodeState::NewChild);
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // add a new text node
                    (o, DomItem::Text { text }) => {
                        patch_set.push(Patch::CreateText { text });
                        state.push(NodeState::NewChild);
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // add attribute to new node
                    (o, DomItem::Attr { name, value }) => {
                        patch_set.push(Patch::SetAttribute { name, value });
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // add event to new node
                    (o, DomItem::Event { trigger, handler }) => {
                        patch_set.push(Patch::AddListener { trigger, handler: handler.into() });
                        o_item = Some(o);
                        n_item = new.next();
                    }
                    // remove the old node if present
                    (DomItem::Element { .. }, n) => {
                        let web_item = sto.next().expect("dom storage to match dom iter");
                        let take = Box::new(move || {
                            let mut taken_item = WebItem::Taken;
                            mem::swap(web_item, &mut taken_item);
                            match taken_item {
                                WebItem::Element(i) => i,
                                _ => panic!("storage type mismatch"),
                            }
                        });

                        if !state.is_child() {
                            patch_set.push(Patch::RemoveElement(take));
                        }
                        state.push(NodeState::OldChild);
                        o_item = old.next();
                        n_item = Some(n);
                    }
                    // remove the old text if present
                    (DomItem::Text { .. }, n) => {
                        let web_item = sto.next().expect("dom storage to match dom iter");
                        let take = Box::new(move || {
                            let mut taken_item = WebItem::Taken;
                            mem::swap(web_item, &mut taken_item);
                            match taken_item {
                                WebItem::Text(i) => i,
                                _ => panic!("storage type mismatch"),
                            }
                        });

                        if !state.is_child() {
                            patch_set.push(Patch::RemoveText(take));
                        }
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
                    (DomItem::Event { trigger, .. }, n) => {
                        let web_item = sto.next().expect("dom storage to match dom iter");
                        let take = Box::new(move || {
                            let mut taken_item = WebItem::Taken;
                            mem::swap(web_item, &mut taken_item);
                            match taken_item {
                                WebItem::Closure(i) => i,
                                _ => panic!("storage type mismatch"),
                            }
                        });

                        if state.is_copy() {
                            patch_set.push(Patch::RemoveListener { trigger, take: take });
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
