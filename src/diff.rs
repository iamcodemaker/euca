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

fn take_component<'a, Message>(item: &'a mut WebItem<Message>) -> Box<dyn FnMut() -> Box<dyn Component<Message>> + 'a> {
    Box::new(move || {
        let mut taken_item = WebItem::Taken;
        mem::swap(item, &mut taken_item);
        match taken_item {
            WebItem::Component(i) => i,
            _ => panic!("storage type mismatch"),
        }
    })
}

/// Return the series of steps required to move from the given old/existing virtual dom to the
/// given new virtual dom.
pub fn diff<'a, Message, Command, I1, I2>(mut old: I1, mut new: I2, storage: &'a mut Storage<Message>) -> PatchSet<'a, Message, Command> where
    Message: 'a + PartialEq + Clone + fmt::Debug,
    I1: Iterator<Item = DomItem<'a, Message, Command>>,
    I2: Iterator<Item = DomItem<'a, Message, Command>>,
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
                    DomItem::UnsafeInnerHtml(html) => {
                        patch_set.push(Patch::SetInnerHtml(html));
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
                    DomItem::Component { msg, create } => {
                        patch_set.push(Patch::CreateComponent { msg, create });
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
                    DomItem::UnsafeInnerHtml(_) => {
                        // ignore child nodes
                        if !o_state.is_child() {
                            patch_set.push(Patch::UnsetInnerHtml);
                        }
                    }
                    DomItem::Up => {
                        if o_state.is_child() {
                            o_state.pop();
                        }
                    }
                    DomItem::Event { .. } => {
                        let _ = sto.next().expect("dom storage to match dom iter");
                    }
                    DomItem::Component { .. } => {
                        let web_item = sto.next().expect("dom storage to match dom iter");
                        patch_set.push(Patch::RemoveComponent(take_component(web_item)));
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
                        DomItem::UnsafeInnerHtml(o_html),
                        DomItem::UnsafeInnerHtml(n_html)
                    ) => { // compare inner html
                        if o_html != n_html {
                            patch_set.push(Patch::SetInnerHtml(n_html));
                        }

                        o_item = old.next();
                        n_item = new.next();
                    }
                    (
                        DomItem::Component { msg: o_msg, create: o_create },
                        DomItem::Component { msg: n_msg, create: n_create }
                    ) => if o_create == n_create { // compare components
                        let web_item = sto.next().expect("dom storage to match dom iter");

                        // message matches, copy the storage
                        if o_msg == n_msg {
                            patch_set.push(Patch::CopyComponent(take_component(web_item)));
                        }
                        // message doesn't match, dispatch it to the component
                        else {
                            patch_set.push(Patch::UpdateComponent { take: take_component(web_item), msg: n_msg });
                        }

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
                                        // innerHtml: ignore
                                        Some(DomItem::UnsafeInnerHtml(_)) => { }
                                        // attribute: ignore
                                        Some(DomItem::Attr { .. }) => { }
                                        // end of node: stop processing
                                        Some(DomItem::Up) => {
                                            o_item = old.next();
                                            break;
                                        }
                                        // component: remove it from storage and the dom
                                        Some(DomItem::Component { .. }) => {
                                            let web_item = sto.next().expect("dom storage to match dom iter");
                                            patch_set.push(Patch::RemoveComponent(take_component(web_item)));
                                            depth += 1;
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
                                        // innerHtml: ignore
                                        Some(DomItem::UnsafeInnerHtml(_)) => { }
                                        // attribute: ignore
                                        Some(DomItem::Attr { .. }) => { }
                                        // end of node: stop processing
                                        Some(DomItem::Up) => {
                                            o_item = old.next();
                                            break;
                                        }
                                        // component: shouldn't be possible as the child of a text
                                        // node, but remove it anyway
                                        Some(DomItem::Component { .. }) => {
                                            let web_item = sto.next().expect("dom storage to match dom iter");
                                            patch_set.push(Patch::RemoveComponent(take_component(web_item)));
                                            depth += 1;
                                        }
                                        o @ None => {
                                            o_item = o;
                                            break;
                                        }
                                    }
                                }
                            }
                            // remove inner html
                            DomItem::UnsafeInnerHtml(_) => {
                                patch_set.push(Patch::UnsetInnerHtml);
                                o_item = old.next();
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
                            // remove old component
                            DomItem::Component { .. } => {
                                let web_item = sto.next().expect("dom storage to match dom iter");
                                patch_set.push(Patch::RemoveComponent(take_component(web_item)));

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
                                        // innerHtml: ignore
                                        Some(DomItem::UnsafeInnerHtml(_)) => { }
                                        // attribute: ignore
                                        Some(DomItem::Attr { .. }) => { }
                                        // end of node: stop processing
                                        Some(DomItem::Up) => {
                                            o_item = old.next();
                                            break;
                                        }
                                        // component: shouldn't be possible as the child of a
                                        // component node, but remove it anyway
                                        Some(DomItem::Component { .. }) => {
                                            let web_item = sto.next().expect("dom storage to match dom iter");
                                            patch_set.push(Patch::RemoveComponent(take_component(web_item)));
                                            depth += 1;
                                        }
                                        o @ None => {
                                            o_item = o;
                                            break;
                                        }
                                    }
                                }
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
                                n_item = add_sub_tree(&mut new, &mut patch_set);
                            }
                            // add a new text node
                            DomItem::Text(text) => {
                                patch_set.push(Patch::CreateText { text });
                                n_item = add_sub_tree(&mut new, &mut patch_set);
                            }
                            // set inner html
                            DomItem::UnsafeInnerHtml(html) => {
                                patch_set.push(Patch::SetInnerHtml(html));
                                n_item = new.next();
                            }
                            // add a new component
                            DomItem::Component { msg, create } => {
                                patch_set.push(Patch::CreateComponent { msg, create });
                                n_item = add_sub_tree(&mut new, &mut patch_set);
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

/// Add this entire element tree.
///
/// Expected to be called where `new.next()` just returned a node that may have children. This will
/// handle creating all of the nodes up to the matching `DomItem::Up` entry.
fn add_sub_tree<'a, Message, Command, I>(new: &mut I, patch_set: &mut PatchSet<'a, Message, Command>)
-> Option<DomItem<'a, Message, Command>>
where
    Message: 'a + PartialEq + Clone + fmt::Debug,
    I: Iterator<Item = DomItem<'a, Message, Command>>,
{
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
            Some(DomItem::Component { msg, create }) => {
                patch_set.push(Patch::CreateComponent { msg, create });
                depth += 1;
            }
            Some(DomItem::UnsafeInnerHtml(html)) => {
                patch_set.push(Patch::SetInnerHtml(html));
            }
            Some(DomItem::Event { trigger, handler }) => {
                patch_set.push(Patch::AddListener { trigger, handler: handler.into() });
            }
            Some(DomItem::Attr { name, value }) => {
                patch_set.push(Patch::SetAttribute { name, value });
            }
            Some(DomItem::Up) if depth > 0 => {
                patch_set.push(Patch::Up);
                depth -= 1;
            }
            Some(DomItem::Up) => {
                patch_set.push(Patch::Up);
                return new.next();
            }
            n @ None => {
                return n;
            }
        }
    }
}
