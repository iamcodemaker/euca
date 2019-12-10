//! Tools to get the difference between two virtual dom trees.

use std::fmt;
use std::mem;
use wasm_bindgen::prelude::Closure;
use crate::patch::PatchSet;
use crate::patch::Patch;
use crate::vdom::DomItem;
use crate::vdom::Storage;
use crate::vdom::WebItem;
use crate::component::Component;

fn take_element<'a, Message>(item: &'a mut WebItem<Message>) -> Box<dyn FnMut() -> web_sys::Element + 'a> {
    Box::new(move || {
        let mut taken_item = WebItem::Taken;
        mem::swap(item, &mut taken_item);
        match taken_item {
            WebItem::Element(i) => i,
            _ => panic!("storage type mismatch"),
        }
    })
}

fn take_text<'a, Message>(item: &'a mut WebItem<Message>) -> Box<dyn FnMut() -> web_sys::Text + 'a> {
    Box::new(move || {
        let mut taken_item = WebItem::Taken;
        mem::swap(item, &mut taken_item);
        match taken_item {
            WebItem::Text(i) => i,
            _ => panic!("storage type mismatch"),
        }
    })
}

fn take_closure<'a, Message>(item: &'a mut WebItem<Message>) -> Box<dyn FnMut() -> Closure<dyn FnMut(web_sys::Event)> + 'a> {
    Box::new(move || {
        let mut taken_item = WebItem::Taken;
        mem::swap(item, &mut taken_item);
        match taken_item {
            WebItem::Closure(i) => i,
            _ => panic!("storage type mismatch"),
        }
    })
}



/// Return the series of steps required to move from the given old/existing virtual dom to the
/// given new virtual dom.
pub fn diff<'a, Message, I1, I2>(mut old: I1, mut new: I2, storage: &'a mut Storage<Message>) -> PatchSet<'a, Message> where
    Message: 'a + PartialEq + Clone + fmt::Debug,
    I1: Iterator<Item = DomItem<'a, Message>>,
    I2: Iterator<Item = DomItem<'a, Message>>,
{
    #[derive(PartialEq)]
    enum NodeState {
        Create,
        Copy,
        Child,
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
                .map_or(false, |ns| *ns == NodeState::Child)
        }
    }

    let mut patch_set = PatchSet::new();

    let mut o_state = State::new();
    let mut n_state = State::new();

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
                    DomItem::Element(element) => {
                        patch_set.push(Patch::CreateElement { element });
                    }
                    DomItem::Text(text) => {
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
                    DomItem::Element(_) => {
                        let web_item = sto.next().expect("dom storage to match dom iter");

                        // ignore child nodes
                        if !o_state.is_child() {
                            patch_set.push(Patch::RemoveElement(take_element(web_item)));
                        }

                        o_state.push(NodeState::Child);
                    }
                    DomItem::Text(_) => {
                        let web_item = sto.next().expect("dom storage to match dom iter");

                        // ignore child nodes
                        if !o_state.is_child() {
                            patch_set.push(Patch::RemoveText(take_text(web_item)));
                        }

                        o_state.push(NodeState::Child);
                    }
                    DomItem::Up => {
                        if o_state.is_child() {
                            o_state.pop();
                        }
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
                        DomItem::Element(o_element),
                        DomItem::Element(n_element)
                    ) if o_element == n_element => { // compare elements
                        let web_item = sto.next().expect("dom storage to match dom iter");

                        // copy the node
                        patch_set.push(Patch::CopyElement(take_element(web_item)));
                        o_state.push(NodeState::Copy);

                        o_item = old.next();
                        n_item = new.next();
                    }
                    (
                        DomItem::Text(o_text),
                        DomItem::Text(n_text)
                    ) => { // compare text
                        let web_item = sto.next().expect("dom storage to match dom iter");

                        // if the text matches, use the web_sys::Text
                        if o_text == n_text {
                            // copy the node
                            patch_set.push(Patch::CopyText(take_text(web_item)));
                        }
                        // text doesn't match, update it
                        else {
                            patch_set.push(Patch::ReplaceText { take: take_text(web_item) , text: n_text });
                        }
                        o_state.push(NodeState::Copy);

                        o_item = old.next();
                        n_item = new.next();
                    }
                    (
                        DomItem::Attr { name: o_name, value: o_value },
                        DomItem::Attr { name: n_name, value: n_value }
                    ) => { // compare attributes
                        if n_state.is_create() {
                            // add attribute
                            patch_set.push(Patch::SetAttribute { name: n_name, value: n_value });
                        }
                        // names are different
                        else if o_name != n_name {
                            if o_state.is_copy() {
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

                        if o_trigger != n_trigger || o_handler != n_handler {
                            if o_state.is_copy() {
                                // remove old listener
                                patch_set.push(Patch::RemoveListener { trigger: o_trigger, take: take_closure(web_item) });
                            }
                            // add new listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into() });
                        }
                        else if n_state.is_create() {
                            // add listener
                            patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into() });
                        }
                        else {
                            // just copy the existing listener
                            patch_set.push(Patch::CopyListener(take_closure(web_item)));
                        }
                        o_item = old.next();
                        n_item = new.next();
                    }
                    (DomItem::Up, DomItem::Up) => { // end of two items
                        patch_set.push(Patch::Up);

                        if o_state.is_copy() {
                            o_state.pop();
                        }

                        if n_state.is_create() {
                            n_state.pop();
                        }

                        o_item = old.next();
                        n_item = new.next();
                    }
                    (o, n) => { // no match
                        // remove the old item
                        match o {
                            DomItem::Up => { // end of old item
                                o_item = Some(o);
                            }
                            // remove the old node if present
                            DomItem::Element(_) => {
                                let web_item = sto.next().expect("dom storage to match dom iter");

                                patch_set.push(Patch::RemoveElement(take_element(web_item)));

                                // skip the rest of the items in the old tree for this element, this
                                // will cause attributes and such to be created on the new element
                                let mut depth = 0;
                                loop {
                                    match old.next() {
                                        // child element: remove from storage, track sub-tree depth
                                        Some(DomItem::Element(_)) => {
                                            let _ = sto.next().expect("dom storage to match dom iter");
                                            depth += 1;
                                        }
                                        // child text: remove from storage, track sub-tree depth
                                        Some(DomItem::Text(_)) => {
                                            let _ = sto.next().expect("dom storage to match dom iter");
                                            depth += 1;
                                        }
                                        // end of child: track sub-tree depth
                                        Some(DomItem::Up) if depth > 0 => {
                                            depth -= 1;
                                        }
                                        // event: remove from storage
                                        Some(DomItem::Event { .. }) => {
                                            let _ = sto.next().expect("dom storage to match dom iter");
                                        }
                                        // attribute: ignore
                                        Some(DomItem::Attr { .. }) => { }
                                        // end of node: stop processing
                                        Some(DomItem::Up) => {
                                            o_item = old.next();
                                            break;
                                        }
                                        o @ None => {
                                            o_item = o;
                                            break;
                                        }
                                    }
                                }
                            }
                            // remove the old text if present
                            DomItem::Text(_) => {
                                let web_item = sto.next().expect("dom storage to match dom iter");

                                patch_set.push(Patch::RemoveText(take_text(web_item)));

                                // skip the rest of the items in the old tree for this element, this
                                // will cause attributes and such to be created on the new element
                                let mut depth = 0;
                                loop {
                                    match old.next() {
                                        // child element: remove from storage, track sub-tree depth
                                        Some(DomItem::Element(_)) => {
                                            let _ = sto.next().expect("dom storage to match dom iter");
                                            depth += 1;
                                        }
                                        // child text: remove from storage, track sub-tree depth
                                        Some(DomItem::Text(_)) => {
                                            let _ = sto.next().expect("dom storage to match dom iter");
                                            depth += 1;
                                        }
                                        // end of child: track sub-tree depth
                                        Some(DomItem::Up) if depth > 0 => {
                                            depth -= 1;
                                        }
                                        // event: remove from storage
                                        Some(DomItem::Event { .. }) => {
                                            let _ = sto.next().expect("dom storage to match dom iter");
                                        }
                                        // attribute: ignore
                                        Some(DomItem::Attr { .. }) => { }
                                        // end of node: stop processing
                                        Some(DomItem::Up) => {
                                            o_item = old.next();
                                            break;
                                        }
                                        o @ None => {
                                            o_item = o;
                                            break;
                                        }
                                    }
                                }
                            }
                            // remove attribute from old node
                            DomItem::Attr { name, value: _ } => {
                                if o_state.is_copy() {
                                    patch_set.push(Patch::RemoveAttribute(name));
                                }
                                o_item = old.next();
                            }
                            // remove event from old node
                            DomItem::Event { trigger, .. } => {
                                let web_item = sto.next().expect("dom storage to match dom iter");

                                if o_state.is_copy() {
                                    patch_set.push(Patch::RemoveListener { trigger, take: take_closure(web_item) });
                                }
                                o_item = old.next();
                            }
                        }

                        // add the new item
                        match n {
                            DomItem::Up => { // end of new item
                                n_item = Some(n);
                            }
                            // add a new child node
                            DomItem::Element(element) => {
                                patch_set.push(Patch::CreateElement { element });

                                // add this entire element tree
                                let mut depth = 0;
                                loop {
                                    match new.next() {
                                        Some(DomItem::Element(element)) => {
                                            patch_set.push(Patch::CreateElement { element });
                                            depth += 1;
                                        }
                                        Some(DomItem::Text(text)) => {
                                            patch_set.push(Patch::CreateText { text });
                                            depth += 1;
                                        }
                                        Some(DomItem::Up) if depth > 0 => {
                                            patch_set.push(Patch::Up);
                                            depth -= 1;
                                        }
                                        Some(DomItem::Event { trigger, handler }) => {
                                            patch_set.push(Patch::AddListener { trigger, handler: handler.into() });
                                        }
                                        Some(DomItem::Attr { name, value }) => {
                                            patch_set.push(Patch::SetAttribute { name, value });
                                        }
                                        Some(DomItem::Up) => {
                                            patch_set.push(Patch::Up);
                                            n_item = new.next();
                                            break;
                                        }
                                        n @ None => {
                                            n_item = n;
                                            break;
                                        }
                                    }
                                }
                            }
                            // add a new text node
                            DomItem::Text(text) => {
                                patch_set.push(Patch::CreateText { text });

                                // add this entire element tree
                                let mut depth = 0;
                                loop {
                                    match new.next() {
                                        Some(DomItem::Element(element)) => {
                                            patch_set.push(Patch::CreateElement { element });
                                            depth += 1;
                                        }
                                        Some(DomItem::Text(text)) => {
                                            patch_set.push(Patch::CreateText { text });
                                            depth += 1;
                                        }
                                        Some(DomItem::Up) if depth > 0 => {
                                            patch_set.push(Patch::Up);
                                            depth -= 1;
                                        }
                                        Some(DomItem::Event { trigger, handler }) => {
                                            patch_set.push(Patch::AddListener { trigger, handler: handler.into() });
                                        }
                                        Some(DomItem::Attr { name, value }) => {
                                            patch_set.push(Patch::SetAttribute { name, value });
                                        }
                                        Some(DomItem::Up) => {
                                            patch_set.push(Patch::Up);
                                            n_item = new.next();
                                            break;
                                        }
                                        n @ None => {
                                            n_item = n;
                                            break;
                                        }
                                    }
                                }
                            }
                            // add attribute to new node
                            DomItem::Attr { name, value } => {
                                patch_set.push(Patch::SetAttribute { name, value });
                                n_item = new.next();
                            }
                            // add event to new node
                            DomItem::Event { trigger, handler } => {
                                patch_set.push(Patch::AddListener { trigger, handler: handler.into() });
                                n_item = new.next();
                            }
                        }
                    }
                }
            }
        }
    }

    patch_set
}
