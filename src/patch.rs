//! Dom patching functionality.
//!
//! This module implements the [`Patch`] and [`PatchSet`] types which provide the tools necessary
//! to describe a set of changes to a dom tree. Also provided is the [`PatchSet::apply`] method
//! which will apply a patch set to the browser's dom tree creating elements as the children of the
//! given parent element and dispatching events using the given dispatcher.
//!
//! [`Patch`]: enum.Patch.html
//! [`PatchSet`]: struct.PatchSet.html
//! [`PatchSet::apply`]: struct.PatchSet.html#method.apply

use std::fmt;
use std::collections::hash_map::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use crate::vdom::EventHandler;
use crate::vdom::WebItem;
use crate::vdom::Storage;
use crate::app::{Dispatch, Dispatcher, SideEffect};
use crate::component::Component;
use log::warn;

/// This enum describes all of the operations we need to preform to move the dom to the desired
/// state. The patch operations expect [`web_sys::Element`], [`web_sys::Text`], and [`Closure`]
/// items to be stored and retrieved from some concrete dom structure which is not provided. The
/// patch operation stores closures which will be called at most once, and either take ownership of
/// and return the desired element or take ownership of and store the given element. Some of the
/// patch operations do not operate on the actual dom but instead move elements and closures from
/// the old virtual dom tree to the new virtual dom tree for storage.
///
/// [`web_sys::Element`]: https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Element.html
/// [`web_sys::Text`]: https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Text.html
/// [`Closure`]: https://rustwasm.github.io/wasm-bindgen/api/wasm_bindgen/closure/struct.Closure.html
pub enum Patch<'a, Message, Command> {
    /// Remove an element.
    RemoveElement(&'a mut WebItem<Message>),
    /// Create an element of the given type.
    CreateElement {
        /// The name/type of element that will be created.
        element: &'a str,
    },
    /// Reference a keyed thing.
    ReferenceKey(u64),
    /// Copy and element from the old dom tree to the new dom tree.
    CopyElement(&'a mut WebItem<Message>),
    /// Move the given element from it's old position in the dom to a new position.
    MoveElement(&'a mut WebItem<Message>),
    /// Remove a text element.
    RemoveText(&'a mut WebItem<Message>),
    /// Replace the value of a text element.
    ReplaceText {
        /// Called once to take an existing text node from the old virtual dom.
        take: &'a mut WebItem<Message>,
        /// The replacement text for the existing text node.
        text: &'a str,
    },
    /// Create a text element.
    CreateText {
        /// The text value of the node to create.
        text: &'a str,
    },
    /// Copy the reference we have to the text element to the new dom.
    CopyText(&'a mut WebItem<Message>),
    /// Update this element by setting innerHTML.
    SetInnerHtml(&'a str),
    /// Remove all of the children of the parent of this element.
    UnsetInnerHtml,
    /// Create a Component.
    CreateComponent {
        /// The initial message to send to the component.
        msg: Message,
        /// The function used to create the component.
        create: fn(Dispatcher<Message, Command>) -> Box<dyn Component<Message>>,
    },
    /// Copy a component from the old dom to the new one.
    CopyComponent(&'a mut WebItem<Message>),
    /// Move a component from the old dom to the new one.
    MoveComponent(&'a mut WebItem<Message>),
    /// Send a message to a component.
    UpdateComponent {
        /// Called once to take an existing component node from the old virtual dom.
        take: &'a mut WebItem<Message>,
        /// The message to send.
        msg: Message,
    },
    /// Move a component and Send a message to it.
    MupdateComponent {
        /// The storage for this component.
        take: &'a mut WebItem<Message>,
        /// The message to send.
        msg: Message,
    },
    /// Remove a component.
    RemoveComponent(&'a mut WebItem<Message>),
    /// Set an attribute.
    SetAttribute {
        /// The name of the attribute to set.
        name: &'a str,
        /// The value of the attribute to set.
        value: &'a str,
    },
    /// Remove an attribute.
    RemoveAttribute(&'a str),
    /// Add an event listener.
    AddListener {
        /// The trigger for the event to watch.
        trigger: &'a str,
        /// A handler for the event.
        handler: EventHandler<'a, Message>,
    },
    /// Copy an event listener from the old dom tree to the new dom tree.
    CopyListener(&'a mut WebItem<Message>),
    /// Remove an event listener.
    RemoveListener {
        /// The trigger for the event to remove.
        trigger: &'a str,
        /// Called once to take an existing closure from the old virtual dom.
        take: &'a mut WebItem<Message>,
    },
    /// This marks the end of operations on the last node.
    Up,
}

impl<'a, Message, Command> fmt::Debug for Patch<'a, Message, Command> where
    Message: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Patch::RemoveElement(e) => write!(f, "RemoveElement({:?})", e),
            Patch::CreateElement { element: s } => write!(f, "CreateElement {{ element: {:?} }}", s),
            Patch::ReferenceKey(k) => write!(f, "ReferenceKey({})", k),
            Patch::CopyElement(e) => write!(f, "CopyElement({:?})", e),
            Patch::MoveElement(k) => write!(f, "MoveElement({:?})", k),
            Patch::RemoveText(wt) => write!(f, "RemoveText({:?})", wt),
            Patch::ReplaceText { take: wt, text: t }  => write!(f, "ReplaceText {{ take: {:?}, text: {:?} }}", wt, t),
            Patch::CreateText { text: t } => write!(f, "CreateText {{ text: {:?} }}", t),
            Patch::CopyText(wt) => write!(f, "CopyText({:?})", wt),
            Patch::SetInnerHtml(html) => write!(f, "SetInnerHtml({:?})", html),
            Patch::UnsetInnerHtml => write!(f, "UnsetInnerHtml"),
            Patch::CreateComponent { msg, create: _ } => write!(f, "CreateComponent {{ msg: {:?}, create: _ }}", msg),
            Patch::UpdateComponent { take: c, msg } => write!(f, "UpdateComponent {{ take: {:?}, msg: {:?} }}", c, msg),
            Patch::CopyComponent(c) => write!(f, "CopyComponent({:?})", c),
            Patch::MoveComponent(c) => write!(f, "MoveComponent({:?})", c),
            Patch::MupdateComponent { take: c, msg } => write!(f, "MupdateComponent {{ take: {:?}, msg: {:?} }}", c, msg),
            Patch::RemoveComponent(c) => write!(f, "RemoveComponent({:?})", c),
            Patch::SetAttribute { name: n, value: v } => write!(f, "SetAttribute {{ name: {:?}, value: {:?} }}", n, v),
            Patch::RemoveAttribute(s) => write!(f, "RemoveAttribute({:?})", s),
            Patch::AddListener { trigger: t, handler: h } => write!(f, "AddListener {{ trigger: {:?}, handler: {:?} }}", t, h),
            Patch::CopyListener(l) => write!(f, "CopyListener({:?})", l),
            Patch::RemoveListener { trigger: t, take: l } => write!(f, "RemoveListener {{ trigger: {:?}), take: {:?} }}", t, l),
            Patch::Up => write!(f, "Up"),
        }
    }
}

macro_rules! attribute_setter_match_arm {
    ( $node:ident, $setter:ident, $attr:literal, $value:ident, [ $node_type1:path $(, $node_type:path )* ] ) => {
        {
            if let Some(elem) = $node.dyn_ref::<$node_type1>() {
                if let Ok(value) = $value.parse() {
                    elem.$setter(value);
                }
                else if $value == $attr {
                    elem.$setter(true);
                }
                else {
                    warn!("non boolean value '{}' set for '{}' attribute", $value, $attr);
                    $node.dyn_ref::<web_sys::Element>()
                        .expect("attributes can only be added to elements")
                        .set_attribute($attr, $value)
                        .expect("failed to set attribute");
                }
            }
            $(else if let Some(elem) = $node.dyn_ref::<$node_type>() {
                if let Ok(value) = $value.parse() {
                    elem.$setter(value);
                }
                else if $value == $attr {
                    elem.$setter(true);
                }
                else {
                    warn!("non boolean value '{}' set for '{}' attribute", $value, $attr);
                    $node.dyn_ref::<web_sys::Element>()
                        .expect("attributes can only be added to elements")
                        .set_attribute($attr, $value)
                        .expect("failed to set attribute");
                }
            })*
            else {
                let elem = $node.dyn_ref::<web_sys::Element>()
                    .expect("attributes can only be added to elements");
                elem.set_attribute($attr, $value)
                    .expect("failed to set attribute");
                warn!("attribute '{}' set for '{}' element, expected one of {}",
                    $attr, elem.node_name(), stringify!($($node_type),*));
            }
        }
    };
}

macro_rules! attribute_setter {
    ( $node:ident, $name:ident, $value:ident, [ $( $attr:literal => $setter:ident [ $( $node_type:path ,)* ] ,)* ] ) => {
        attribute_setter!($node, $name, $value, [ $( $attr => $setter [ $( $node_type ),* ] ),* ] )
    };
    ( $node:ident, $name:ident, $value:ident, [ $( $attr:literal => $setter:ident [ $( $node_type:path ),* ] ),* ] ) => {
        match $name {
            $( $attr => { attribute_setter_match_arm!($node, $setter, $attr, $value, [ $($node_type),* ]) } )*
            _ => {
                attribute_setter!($node, $name, $value);
            }
        }
    };
    ( $node:ident, $name:ident, $value:ident ) => {
        $node.dyn_ref::<web_sys::Element>()
            .expect("attributes can only be added to elements")
            .set_attribute($name, $value)
            .expect("failed to set attribute");
    };
}

macro_rules! attribute_unsetter_match_arm {
    ( $node:ident, $setter:ident, $attr:literal, [ $node_type1:path $(, $node_type:path )* ] ) => {
        if let Some(elem) = $node.dyn_ref::<$node_type1>() {
            elem.$setter(false);
        }
        $(else if let Some(elem) = $node.dyn_ref::<$node_type>() {
            elem.$setter(false);
        })*
        else {
            let elem = $node.dyn_ref::<web_sys::Element>()
                .expect("attributes can only be removed from elements");
            elem.remove_attribute($attr)
                .expect("failed to set attribute");
            warn!("attribute '{}' removed for '{}' element, expected one of {}",
                $attr, elem.node_name(), stringify!($($node_type),*));
        }
    };
}

macro_rules! attribute_unsetter {
    ( $node:ident, $name:ident, [ $( $attr:literal => $setter:ident [ $( $node_type:path ,)* ] ,)* ] ) => {
        attribute_unsetter!($node, $name, [ $( $attr => $setter [ $( $node_type ),* ] ),* ] )
    };
    ( $node:ident, $name:ident, [ $( $attr:literal => $setter:ident [ $( $node_type:path ),* ] ),* ] ) => {
        match $name {
            $( $attr => { attribute_unsetter_match_arm!($node, $setter, $attr, [ $($node_type),* ]) } )*
            _ => {
                $node.dyn_ref::<web_sys::Element>()
                    .expect("attributes can only be removed from elements")
                    .remove_attribute($name)
                    .expect("failed to remove attribute");
            }
        }
    };
}

/// A series of [`Patch`]es to apply to the dom.
///
/// [`Patch`]: enum.Patch.html
#[derive(Default, Debug)]
pub struct PatchSet<'a, Message, Command> {
    /// The patches in this patch set.
    pub patches: Vec<Patch<'a, Message, Command>>,
    /// Mini patch sets for keyed nodes.
    pub keyed: HashMap<u64, Vec<Patch<'a, Message, Command>>>,
}

impl<'a, Message, Command> PatchSet<'a, Message, Command> {
    /// Create an empty PatchSet.
    pub fn new() -> Self {
        PatchSet {
            patches: vec![],
            keyed: HashMap::new(),
        }
    }

    /// Push a patch on to the end of the PatchSet.
    pub fn push(&mut self, patch: Patch<'a, Message, Command>) {
        self.patches.push(patch)
    }

    /// Move the top level patch set into a keyed entry.
    pub fn root_key(&mut self, key: u64) {
        let mut patches = vec![];
        std::mem::swap(&mut self.patches, &mut patches);
        self.keyed.insert(key, patches);
    }

    /// Put the patches from the given patch set into this PatchSet.
    pub fn extend(&mut self, other: Self) {
        let Self { patches, keyed } = other;
        self.patches.extend(patches);
        self.keyed.extend(keyed);
    }

    /// Return the length of the PatchSet.
    pub fn len(&self) -> usize {
        return self.patches.len()
    }

    /// Return true if applying this PatchSet won't actually alter the browser's dom representation
    /// and false otherwise.
    pub fn is_noop(&self) -> bool {
        use Patch::*;

        self.patches.iter()
            .chain(self.keyed.values().flatten())
            .all(|p| match p {
            // these patches just copy stuff into the new virtual dom tree, thus if we just keep
            // the old dom tree, the end result is the same
            CopyElement(_) | CopyListener(_) | ReferenceKey(_)
            | CopyText(_) | CopyComponent(_) | Up
            => true,
            // these patches change the dom
            RemoveElement(_) | CreateElement { .. }
            | MoveElement(_)
            | CreateComponent { .. } | UpdateComponent { .. }
            | MoveComponent { .. } | MupdateComponent { .. }
            | RemoveComponent(_)
            | SetInnerHtml(_) | UnsetInnerHtml
            | RemoveListener { .. } | AddListener { .. }
            | RemoveAttribute(_) | SetAttribute { .. }
            | RemoveText(_) | CreateText { .. } | ReplaceText { .. }
            => false,
        })
    }

    fn process_patch_list(
        patches: Vec<Patch<'a, Message, Command>>,
        keyed: &mut HashMap<u64, Vec<Patch<'a, Message, Command>>>,
        app: &Dispatcher<Message, Command>,
        storage: &mut Storage<Message>,
    )
    -> Vec<web_sys::Node>
    where
        Message: Clone + PartialEq + fmt::Debug + 'static,
        Command: SideEffect<Message> + 'static,
        EventHandler<'a, Message>: Clone,
    {
        let mut node_stack = NodeStack::new();
        let mut special_attributes: Vec<(web_sys::Node, &str, &str)> = vec![];

        let document = web_sys::window().expect("expected window")
            .document().expect("expected document");

        for p in patches.into_iter() {
            match p {
                Patch::ReferenceKey(key) => {
                    let patches = keyed.remove(&key)
                        .expect("patches for given key not found");
                    let nodes = Self::process_patch_list(patches, keyed, app, storage);
                    for node in nodes {
                        node_stack.push_child(node);
                    }
                }
                Patch::RemoveElement(item) => {
                    item.take().as_element()
                        .expect("unexpected WebItem, expected element")
                        .remove();
                }
                Patch::CreateElement { element } => {
                    let node = document.create_element(&element).expect("failed to create element");
                    storage.push(WebItem::Element(node.clone()));
                    node_stack.push_child(node.clone());
                    node_stack.push_parent(node);
                }
                Patch::CopyElement(item) => {
                    let item = item.take();
                    let node = item.as_element()
                        .expect("unexpected WebItem, expected element")
                        .clone();

                    storage.push(item);
                    node_stack.insert_before(Some(&node));
                    node_stack.push_parent(node);
                }
                Patch::MoveElement(item) => {
                    let item = item.take();
                    let node = item.as_element()
                        .expect("unexpected WebItem, expected element")
                        .clone();

                    storage.push(item);
                    node_stack.push_child(node.clone());
                    node_stack.push_parent(node);
                }
                Patch::RemoveText(item) => {
                    let item = item.take();
                    let node = item.as_text()
                        .expect("unexpected WebItem, expected text");

                    node_stack.last()
                        .expect("no previous node")
                        .remove_child(&node)
                        .expect("failed to remove child node");
                }
                Patch::ReplaceText { take: item, text } => {
                    let item = item.take();
                    let node = item.as_text()
                        .expect("unexpected WebItem, expected text")
                        .clone();

                    node.set_data(&text);

                    storage.push(item);
                    node_stack.insert_before(Some(&node));
                    node_stack.push_parent(node);
                }
                Patch::CreateText { text } => {
                    let node = document.create_text_node(&text);

                    storage.push(WebItem::Text(node.clone()));
                    node_stack.push_child(node.clone());
                    node_stack.push_parent(node);
                }
                Patch::CopyText(item) => {
                    let item = item.take();
                    let node = item.as_text()
                        .expect("unexpected WebItem, expected text")
                        .clone();

                    storage.push(item);
                    node_stack.insert_before(Some(&node));
                    node_stack.push_parent(node);
                }
                Patch::SetInnerHtml(html) => {
                    node_stack.last()
                        .expect("no previous node")
                        .dyn_ref::<web_sys::Element>()
                        .expect("innerHtml requested on non Element node")
                        .set_inner_html(html);
                }
                Patch::UnsetInnerHtml => {
                    let node = node_stack.last()
                        .expect("no previous node");

                    // remove all of the children of this node. These are the nodes created by the
                    // innerHtml value.
                    while let Some(child) = node.first_child() {
                        node.remove_child(&child)
                            .expect("failed to remove innerHtml child node");
                    }
                }
                Patch::SetAttribute { name, value } => {
                    let node = node_stack.last().expect("no previous node");
                    match name {
                        "autofocus" | "checked" | "disabled" | "draggable" |  "hidden"
                        | "selected" | "spellcheck" | "value"
                        => {
                            // delay setting special attributes until after everything else is done
                            special_attributes.push((node.clone(), name, value));
                        }
                        _ => attribute_setter!(node, name, value),
                    }
                }
                Patch::RemoveAttribute(name) => {
                    let node = node_stack.last().expect("no previous node");

                    // properly handle boolean attributes using special setters
                    attribute_unsetter!(node, name, [
                        "autofocus" => set_autofocus [
                            web_sys::HtmlButtonElement,
                            web_sys::HtmlInputElement,
                            web_sys::HtmlSelectElement,
                            web_sys::HtmlTextAreaElement,
                        ],
                        "checked" => set_checked [
                            web_sys::HtmlInputElement,
                            web_sys::HtmlMenuItemElement,
                        ],
                        "disabled" => set_disabled [
                            web_sys::HtmlButtonElement,
                            web_sys::HtmlFieldSetElement,
                            web_sys::HtmlInputElement,
                            web_sys::HtmlLinkElement,
                            web_sys::HtmlMenuItemElement,
                            web_sys::HtmlOptGroupElement,
                            web_sys::HtmlOptionElement,
                            web_sys::HtmlSelectElement,
                            web_sys::HtmlStyleElement,
                            web_sys::HtmlTextAreaElement,
                        ],
                        "draggable" => set_draggable [
                            web_sys::HtmlElement,
                        ],
                        "hidden" => set_hidden [
                            web_sys::HtmlElement,
                        ],
                        "selected" => set_selected [
                            web_sys::HtmlOptionElement,
                        ],
                        "spellcheck" => set_spellcheck [
                            web_sys::HtmlElement,
                        ],
                    ]);
                }
                Patch::AddListener { trigger, handler } => {
                    let app = app.clone();
                    let closure = match handler {
                        EventHandler::Msg(msg) => {
                            let msg = msg.clone();
                            Closure::wrap(
                                Box::new(move |_| {
                                    Dispatch::dispatch(&app, msg.clone())
                                }) as Box<dyn FnMut(web_sys::Event)>
                            )
                        }
                        EventHandler::Fn(fun) => {
                            Closure::wrap(
                                Box::new(move |event| {
                                    if let Some(msg) = fun(event) {
                                        Dispatch::dispatch(&app, msg);
                                    }
                                }) as Box<dyn FnMut(web_sys::Event)>
                            )
                        }
                        EventHandler::FnMsg(msg, fun) => {
                            let msg = msg.clone();
                            Closure::wrap(
                                Box::new(move |event| {
                                    if let Some(msg) = fun(msg.clone(), event) {
                                        Dispatch::dispatch(&app, msg);
                                    }
                                }) as Box<dyn FnMut(web_sys::Event)>
                            )
                        }
                        EventHandler::InputValue(fun) => {
                            Closure::wrap(
                                Box::new(move |event: web_sys::Event| {
                                    let value = match event.target() {
                                        None => String::new(),
                                        Some(target) => {
                                            if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                                                input.value()
                                            }
                                            else if let Some(input) = target.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                                                input.value()
                                            }
                                            else if let Some(input) = target.dyn_ref::<web_sys::HtmlSelectElement>() {
                                                input.value()
                                            }
                                            else {
                                                String::new()
                                            }
                                        }
                                    };
                                    if let Some(msg) = fun(value) {
                                        Dispatch::dispatch(&app, msg);
                                    }
                                }) as Box<dyn FnMut(web_sys::Event)>
                            )
                        }
                        EventHandler::InputEvent(fun) => {
                            Closure::wrap(
                                Box::new(move |event: web_sys::Event| {
                                    let event = event.dyn_into::<web_sys::InputEvent>().expect_throw("expected web_sys::InputEvent");
                                    if let Some(msg) = fun(event) {
                                        Dispatch::dispatch(&app, msg);
                                    }
                                }) as Box<dyn FnMut(web_sys::Event)>
                            )
                        }
                    };
                    let node = node_stack.last().expect("no previous node");
                    (node.as_ref() as &web_sys::EventTarget)
                        .add_event_listener_with_callback(&trigger, closure.as_ref().unchecked_ref())
                        .expect("failed to add event listener");
                    storage.push(WebItem::Closure(closure));
                }
                Patch::CopyListener(item) => {
                    storage.push(item.take());
                }
                Patch::RemoveListener { trigger, take: item } => {
                    let item = item.take();
                    let closure = item.as_closure()
                        .expect("unexpected WebItem, expected closure")
                        .as_ref().unchecked_ref();

                    let node = node_stack.last().expect("no previous node");
                    (node.as_ref() as &web_sys::EventTarget)
                        .remove_event_listener_with_callback(&trigger, closure)
                        .expect("failed to remove event listener");
                }
                Patch::CreateComponent { msg, create } => {
                    let mut component = create(app.clone());
                    for n in component.pending().into_iter() {
                        node_stack.push_child(n);
                    }
                    let node = component.node().expect("empty component?");
                    node_stack.push_parent(node);

                    component.dispatch(msg);
                    storage.push(WebItem::Component(component));
                }
                Patch::UpdateComponent { take: item, msg } => {
                    let item = item.take();
                    let component = item.as_component()
                        .expect("unexpected WebItem, expected component");

                    component.dispatch(msg);

                    let node = component.node().expect("empty component?");
                    storage.push(item);
                    node_stack.insert_before(Some(&node));
                    node_stack.push_parent(node);
                }
                Patch::MupdateComponent { take: item, msg } => {
                    let item = item.take();
                    let component = item.as_component()
                        .expect("unexpected WebItem, expected component");

                    component.dispatch(msg);

                    for n in component.nodes().into_iter() {
                        node_stack.push_child(n);
                    }
                    let node = component.node().expect("empty component?");
                    node_stack.push_parent(node);
                    storage.push(item);
                }
                Patch::CopyComponent(item) => {
                    let item = item.take();
                    let component = item.as_component()
                        .expect("unexpected WebItem, expected component");

                    let node = component.node().expect("empty component?");

                    storage.push(item);
                    node_stack.insert_before(Some(&node));
                    node_stack.push_parent(node);
                }
                Patch::MoveComponent(item) => {
                    let item = item.take();
                    let component = item.as_component()
                        .expect("unexpected WebItem, expected component");

                    for n in component.nodes().into_iter() {
                        node_stack.push_child(n);
                    }
                    let node = component.node().expect("empty component?");
                    node_stack.push_parent(node);
                    storage.push(item);
                }
                Patch::RemoveComponent(item) => {
                    let item = item.take();
                    let component = item.as_component()
                        .expect("unexpected WebItem, expected component");

                    component.detach();
                }
                Patch::Up => {
                    node_stack.pop();
                    storage.push(WebItem::Up);
                }
            }
        }

        // set special attributes. These must be done last or strange things can happen when
        // rendering in the browser. I have observed range inputs not properly updating (appears to
        // be caused by `value` getting set before `max`) and option inputs not getting set.
        for (node, name, value) in special_attributes.into_iter() {
            let mut set_value = false;

            // handle the "value" attribute for non boolean values
            if name == "value" {
                set_value = true;
                if let Some(input) = node.dyn_ref::<web_sys::HtmlInputElement>() {
                    input.set_value(value);
                }
                else if let Some(input) = node.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                    input.set_value(value);
                }
                else if let Some(input) = node.dyn_ref::<web_sys::HtmlSelectElement>() {
                    input.set_value(value);
                }
                else {
                    set_value = false;
                }
            }

            if !set_value {
                // properly handle boolean attributes using special setters
                attribute_setter!(node, name, value, [
                    "autofocus" => set_autofocus [
                        web_sys::HtmlButtonElement,
                        web_sys::HtmlInputElement,
                        web_sys::HtmlSelectElement,
                        web_sys::HtmlTextAreaElement,
                    ],
                    "checked" => set_checked [
                        web_sys::HtmlInputElement,
                        web_sys::HtmlMenuItemElement,
                    ],
                    "disabled" => set_disabled [
                        web_sys::HtmlButtonElement,
                        web_sys::HtmlFieldSetElement,
                        web_sys::HtmlInputElement,
                        web_sys::HtmlLinkElement,
                        web_sys::HtmlMenuItemElement,
                        web_sys::HtmlOptGroupElement,
                        web_sys::HtmlOptionElement,
                        web_sys::HtmlSelectElement,
                        web_sys::HtmlStyleElement,
                        web_sys::HtmlTextAreaElement,
                    ],
                    "draggable" => set_draggable [
                        web_sys::HtmlElement,
                    ],
                    "hidden" => set_hidden [
                        web_sys::HtmlElement,
                    ],
                    "selected" => set_selected [
                        web_sys::HtmlOptionElement,
                    ],
                    "spellcheck" => set_spellcheck [
                        web_sys::HtmlElement,
                    ],
                ]);
            }
        }

        assert_eq!(node_stack.depth(), 0, "the stack should be empty");
        node_stack.pop_pending()
    }

    /// Prep the given PatchSet by creating any elements in the set and placing them in Storage.
    /// While elements will be removed from the given parent, nothing will be attached.  Events
    /// will be dispatched via the given [`Dispatch`]er.
    ///
    /// [`Dispatch`]: ../app/trait.Dispatch.html
    pub fn prepare(self, app: &Dispatcher<Message, Command>) -> (Storage<Message>, Vec<web_sys::Node>) where
        Message: Clone + PartialEq + fmt::Debug + 'static,
        Command: SideEffect<Message> + fmt::Debug + 'static,
        EventHandler<'a, Message>: Clone,
    {
        let mut storage = vec![];
        let PatchSet { patches, mut keyed } = self;

        let nodes = Self::process_patch_list(patches, &mut keyed, app, &mut storage);
        (storage, nodes)
    }

    /// Apply the given PatchSet creating any elements under the given parent node. Events are
    /// dispatched via the given [`Dispatch`]er.
    ///
    /// [`Dispatch`]: ../app/trait.Dispatch.html
    pub fn apply(self, parent: &web_sys::Element, app: &Dispatcher<Message, Command>) -> Storage<Message> where
        Message: Clone + PartialEq + fmt::Debug + 'static,
        Command: SideEffect<Message> + fmt::Debug + 'static,
        EventHandler<'a, Message>: Clone,
    {
        let (storage, pending) = self.prepare(app);

        // add top level nodes
        for node in pending.iter() {
            parent
                .insert_before(node, None)
                .expect("failed to insert child node");
        }

        // return storage so it can be stored by the caller
        storage
    }
}

struct NodeStack {
    /// Parent nodes in the tree [(parent, [pending children])].
    stack: Vec<(web_sys::Node, Vec<web_sys::Node>)>,
    pending: Vec<web_sys::Node>,
}

impl NodeStack {
    fn new() -> Self {
        Self {
            stack: vec![],
            pending: vec![],
        }
    }

    /// Get the current depth of the tree.
    fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Get the current parent node off the stack, if any.
    fn last(&self) -> Option<&web_sys::Node> {
        self.stack.last().map(|(node, _)| node)
    }

    /// Add a new parent node to the stack.
    fn push_parent(&mut self, parent: impl Into<web_sys::Node>) {
        self.stack.push((parent.into(), vec![]));
    }

    /// Append a pending child node to the current parent.
    fn push_child(&mut self, child: impl Into<web_sys::Node>) {
        self.stack.last_mut()
            .map_or(&mut self.pending, |(_parent, pending)| pending)
            .push(child.into());
    }

    /// We are finished processing this parent node, remove it from the stack and append any
    /// remaining child nodes.
    fn pop(&mut self) {
        self.insert_before(None);
        self.stack.pop();
    }

    /// Pop and return pending items.
    fn pop_pending(&mut self) -> Vec<web_sys::Node> {
        let mut pending = vec![];
        std::mem::swap(&mut self.pending, &mut pending);
        pending
    }

    /// Insert any pending children into the parent before the given child node.
    fn insert_before(&mut self, child: Option<&web_sys::Node>) {
        if let Some((parent, pending)) = &mut self.stack.last_mut() {
            for node in pending.drain(..) {
                parent
                    .insert_before(&node, child)
                    .expect("failed to insert child node");
            }
        }
        else if let Some(sibling) = child {
            let parent = sibling.parent_node();
            for node in self.pending.drain(..) {
                parent.as_ref()
                    .expect("no parent node")
                    .insert_before(&node, Some(sibling))
                    .expect("failed to insert child node");
            }
        }
        else {
            unreachable!("there should never be an None sibling and an empty stack");
        }
    }
}

impl<'a, Message, Command> From<Vec<Patch<'a, Message, Command>>> for PatchSet<'a, Message, Command> {
    fn from(v: Vec<Patch<'a, Message, Command>>) -> Self {
        PatchSet {
            patches: v,
            keyed: HashMap::new(),
        }
    }
}

impl<'a, Message, Command> IntoIterator for PatchSet<'a, Message, Command> {
    type Item = Patch<'a, Message, Command>;
    type IntoIter = ::std::vec::IntoIter<Patch<'a, Message, Command>>;

    fn into_iter(self) -> Self::IntoIter {
        self.patches.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use wasm_bindgen_test::*;
    use wasm_bindgen_test::wasm_bindgen_test_configure;
    wasm_bindgen_test_configure!(run_in_browser);

    use crate::test::{App, Msg, Cmd};

    fn elem(name: &str) -> web_sys::Element {
        web_sys::window().expect("expected window")
            .document().expect("expected document")
            .create_element(name).expect("expected element")
    }

    fn leaked_elem<Message>(name: &str) -> &mut WebItem<Message> {
        Box::leak(Box::new(WebItem::Element(elem(name))))
    }

    #[test]
    fn empty_patch_set_is_noop() {
        assert!(PatchSet::<Msg, Cmd>::new().is_noop());
    }

    #[wasm_bindgen_test]
    fn noop_patch_set_is_noop() {
        let patch_set: PatchSet<Msg, Cmd> = vec![
            Patch::CopyElement(leaked_elem("test")),
            Patch::Up,
        ].into();

        assert!(patch_set.is_noop());
    }

    #[wasm_bindgen_test]
    fn keyed_noop_patch_set_is_noop() {
        let mut keyed: HashMap<_, Vec<Patch<Msg, Cmd>>> = HashMap::new();
        keyed.insert(1, vec![
            Patch::CopyElement(leaked_elem("test")),
            Patch::Up,
        ]);
        let patch_set = PatchSet {
            patches: vec![
                Patch::ReferenceKey(1),
                Patch::Up,
            ],
            keyed
        };

        assert!(patch_set.is_noop());
    }

    #[test]
    fn not_noop() {
        let patch_set: PatchSet<Msg, Cmd> = vec![
            Patch::CreateElement {
                element: "",
            },
        ].into();

        assert!(!patch_set.is_noop());
    }

    #[test]
    fn keyed_not_noop() {
        let mut keyed: HashMap<_, Vec<Patch<Msg, Cmd>>> = HashMap::new();
        keyed.insert(1, vec![
            Patch::CreateElement { element: "" },
            Patch::Up,
        ]);

        let patch_set = PatchSet {
            patches: vec![
                Patch::ReferenceKey(1),
                Patch::Up,
            ],
            keyed
        };

        assert!(!patch_set.is_noop());
    }

    #[wasm_bindgen_test]
    fn copy_element() {
        let patch_set: PatchSet<Msg, Cmd> = vec![
            Patch::CopyElement(leaked_elem("test")),
            Patch::Up,
        ].into();


        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        assert!(!storage.is_empty());
    }

    #[wasm_bindgen_test]
    fn add_attribute() {
        use Patch::*;

        let mut e = WebItem::Element({
            let e = elem("test");
            assert!(e.get_attribute("name").is_none());
            e
        });

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CopyElement(&mut e),
            SetAttribute { name: "name", value: "value" },
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let attribute = match storage[0] {
            WebItem::Element(ref e) => e.get_attribute("name"),
            _ => panic!("element not stored as expected"),
        };
        assert!(attribute.is_some());
        assert_eq!(attribute.unwrap(), "value");
    }

    #[wasm_bindgen_test]
    fn add_attribute_checked() {
        use Patch::*;

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "checked", value: "true" },
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let element = match storage[0] {
            WebItem::Element(ref e) => e,
            _ => panic!("element not stored as expected"),
        };
        let input = element.dyn_ref::<web_sys::HtmlInputElement>().expect("expected input element");

        assert!(input.checked());
    }

    #[wasm_bindgen_test]
    fn add_attribute_disabled() {
        use Patch::*;

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "disabled", value: "true" },
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let element = match storage[0] {
            WebItem::Element(ref e) => e,
            _ => panic!("element not stored as expected"),
        };
        let input = element.dyn_ref::<web_sys::HtmlInputElement>().expect("expected input element");

        assert!(input.disabled());
    }

    #[wasm_bindgen_test]
    fn remove_attribute() {
        use Patch::*;

        let mut e = WebItem::Element({
            let e = elem("test");
            e.set_attribute("name", "value").expect("setting attribute failed");
            e
        });

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CopyElement(&mut e),
            RemoveAttribute("name"),
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let attribute = match storage[0] {
            WebItem::Element(ref e) => e.get_attribute("name"),
            _ => panic!("element not stored as expected"),
        };
        assert!(attribute.is_none());
    }

    #[wasm_bindgen_test]
    fn remove_attribute_checked() {
        use Patch::*;

        let mut e = WebItem::Element({
            let e = elem("input");
            e.set_attribute("checked", "true").expect("setting attribute failed");
            e
        });

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CopyElement(&mut e),
            RemoveAttribute("checked"),
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let element = match storage[0] {
            WebItem::Element(ref e) => e,
            _ => panic!("element not stored as expected"),
        };
        let input = element.dyn_ref::<web_sys::HtmlInputElement>().expect("expected input element");

        assert!(!input.checked());
    }

    #[wasm_bindgen_test]
    fn remove_attribute_disabled() {
        use Patch::*;

        let mut e = WebItem::Element({
            let e = elem("input");
            e.set_attribute("disabled", "true").expect("setting attribute failed");
            e
        });

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CopyElement(&mut e),
            RemoveAttribute("disabled"),
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let element = match storage[0] {
            WebItem::Element(ref e) => e,
            _ => panic!("element not stored as expected"),
        };
        let input = element.dyn_ref::<web_sys::HtmlInputElement>().expect("expected input element");

        assert!(!input.disabled());
    }

    #[wasm_bindgen_test]
    fn set_attribute_checked_false() {
        use Patch::*;

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "checked", value: "false" },
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let element = match storage[0] {
            WebItem::Element(ref e) => e,
            _ => panic!("element not stored as expected"),
        };
        let input = element.dyn_ref::<web_sys::HtmlInputElement>().expect("expected input element");

        assert!(!input.checked());
    }

    #[wasm_bindgen_test]
    fn set_attribute_disabled_false() {
        use Patch::*;

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "disabled", value: "false" },
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let element = match storage[0] {
            WebItem::Element(ref e) => e,
            _ => panic!("element not stored as expected"),
        };
        let input = element.dyn_ref::<web_sys::HtmlInputElement>().expect("expected input element");

        assert!(!input.disabled());
    }

    #[wasm_bindgen_test]
    fn set_attribute_autofocus_false() {
        use Patch::*;

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "autofocus", value: "false" },
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let element = match storage[0] {
            WebItem::Element(ref e) => e,
            _ => panic!("element not stored as expected"),
        };
        let input = element.dyn_ref::<web_sys::HtmlInputElement>().expect("expected input element");

        assert!(!input.autofocus());
    }

    #[wasm_bindgen_test]
    fn set_attribute_selected_false() {
        use Patch::*;

        let patch_set: PatchSet<Msg, Cmd> = vec![
            CreateElement {
                element: "option",
            },
            SetAttribute { name: "selected", value: "false" },
            Up,
        ].into();

        let app = App::dispatcher();
        let parent = elem("div");
        let storage = patch_set.apply(&parent, &app);

        let element = match storage[0] {
            WebItem::Element(ref e) => e,
            _ => panic!("element not stored as expected"),
        };
        let option = element.dyn_ref::<web_sys::HtmlOptionElement>().expect("expected input element");

        assert!(!option.selected());
    }

    #[wasm_bindgen_test]
    fn insert_element() {
        use crate::dom::{Dom, DomVec};
        use crate::vdom::DomIter;
        use crate::diff;
        use std::iter;

        let gen1: DomVec<_> = vec![
            Dom::elem("a"),
            Dom::elem("b"),
            Dom::elem("i"),
        ].into();

        let gen2: DomVec<_> = vec![
            Dom::elem("a"),
            Dom::elem("p"),
            Dom::elem("i"),
        ].into();

        let parent = elem("div");
        let app = App::dispatcher();
        let mut storage = vec![];

        let n = gen1.dom_iter();
        let patch_set = diff::diff(iter::empty(), n, &mut storage);
        storage = patch_set.apply(&parent, &app);

        let o = gen1.dom_iter();
        let n = gen2.dom_iter();
        let patch_set = diff::diff(o, n, &mut storage);
        storage = patch_set.apply(&parent, &app);

        match storage[2] {
            WebItem::Element(ref node) => assert_eq!(node.node_name(), "P", "wrong node in storage"),
            _ => panic!("expected node to be created"),
        }

        assert_eq!(
            parent.children()
                .item(1)
                .expect("expected child node")
                .node_name(),
            "P",
            "wrong node in DOM"
        );
    }

    #[wasm_bindgen_test]
    fn insert_element_nested() {
        use crate::dom::Dom;
        use crate::vdom::DomIter;
        use crate::diff;
        use std::iter;

        let gen1 = Dom::elem("div")
            .push(Dom::elem("a"))
            .push(Dom::elem("b"))
            .push(Dom::elem("i"));

        let gen2 = Dom::elem("div")
            .push(Dom::elem("a"))
            .push(Dom::elem("p"))
            .push(Dom::elem("i"));

        let parent = elem("div");
        let app = App::dispatcher();
        let mut storage = vec![];

        let n = gen1.dom_iter();
        let patch_set = diff::diff(iter::empty(), n, &mut storage);
        storage = patch_set.apply(&parent, &app);

        let o = gen1.dom_iter();
        let n = gen2.dom_iter();
        let patch_set = diff::diff(o, n, &mut storage);
        storage = patch_set.apply(&parent, &app);

        match storage[3] {
            WebItem::Element(ref node) => assert_eq!(node.node_name(), "P", "wrong node in storage"),
            ref e => panic!("expected node to be created instead of: {:?}", e),
        }

        assert_eq!(
            parent.children()
                .item(0)
                .expect("expected outer child node")
                .children()
                .item(1)
                .expect("expected inner child node")
                .node_name(),
            "P",
            "wrong node in DOM"
        );
    }
}
