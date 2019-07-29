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
use std::rc::Rc;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use crate::vdom::EventHandler;
use crate::vdom::WebItem;
use crate::vdom::Storage;
use crate::app::Dispatch;
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
pub enum Patch<'a, Message> {
    /// Remove an element.
    RemoveElement(Box<FnMut() -> web_sys::Element + 'a>),
    /// Create an element of the given type.
    CreateElement {
        /// The name/type of element that will be created.
        element: &'a str,
    },
    /// Copy and element from the old dom tree to the new dom tree.
    CopyElement(Box<FnMut() -> web_sys::Element + 'a>),
    /// Remove a text element.
    RemoveText(Box<FnMut() -> web_sys::Text + 'a>),
    /// Replace the value of a text element.
    ReplaceText {
        /// Called once to take an existing text node from the old virtual dom.
        take: Box<FnMut() -> web_sys::Text + 'a>,
        /// The replacement text for the existing text node.
        text: &'a str,
    },
    /// Create a text element.
    CreateText {
        /// The text value of the node to create.
        text: &'a str,
    },
    /// Copy the reference we have to the text element to the new dom.
    CopyText(Box<FnMut() -> web_sys::Text + 'a>),
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
    CopyListener(Box<FnMut() -> Closure<FnMut(web_sys::Event)> + 'a>),
    /// Remove an event listener.
    RemoveListener {
        /// The trigger for the event to remove.
        trigger: &'a str,
        /// Called once to take an existing closure from the old virtual dom.
        take: Box<FnMut() -> Closure<FnMut(web_sys::Event)> + 'a>,
    },
    /// This marks the end of operations on the last node.
    Up,
}

impl<'a, Message> fmt::Debug for Patch<'a, Message> where
    Message: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Patch::RemoveElement(_) => write!(f, "RemoveElement(_)"),
            Patch::CreateElement { element: s } => write!(f, "CreateElement {{ element: {:?} }}", s),
            Patch::CopyElement(_) => write!(f, "CopyElement(_)"),
            Patch::RemoveText(_) => write!(f, "RemoveText(_)"),
            Patch::ReplaceText { take: _, text: t }  => write!(f, "ReplaceText {{ take: _, text: {:?} }}", t),
            Patch::CreateText { text: t } => write!(f, "CreateText {{ text: {:?} }}", t),
            Patch::CopyText(_) => write!(f, "CopyText(_)"),
            Patch::SetAttribute { name: n, value: v } => write!(f, "SetAttribute {{ name: {:?}, value: {:?} }}", n, v),
            Patch::RemoveAttribute(s) => write!(f, "RemoveAttribute({:?})", s),
            Patch::AddListener { trigger: t, handler: h } => write!(f, "AddListener {{ trigger: {:?}, handler: {:?} }}", t, h),
            Patch::CopyListener(_) => write!(f, "CopyListener(_)"),
            Patch::RemoveListener { trigger: t, take: _ } => write!(f, "RemoveListener {{ trigger: {:?}), take: _ }}", t),
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
                $node.dyn_ref::<web_sys::Element>()
                    .expect("attributes can only be added to elements")
                    .set_attribute($name, $value)
                    .expect("failed to set attribute");
            }
        }
    }
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
pub struct PatchSet<'a, Message> {
    /// The patches in this patch set.
    pub patches: Vec<Patch<'a, Message>>,
    storage: Vec<WebItem>,
}

impl<'a, Message> PatchSet<'a, Message> {
    /// Create an empty PatchSet.
    pub fn new() -> Self {
        PatchSet {
            patches: vec![],
            storage: vec![],
        }
    }

    /// Push a patch on to the end of the PatchSet.
    pub fn push(&mut self, patch: Patch<'a, Message>) {
        self.patches.push(patch)
    }

    /// Return the length of the PatchSet.
    pub fn len(&self) -> usize {
        return self.patches.len()
    }

    /// Return true if applying this PatchSet won't actually alter the browser's dom representation
    /// and false otherwise.
    pub fn is_noop(&self) -> bool {
        use Patch::*;

        self.patches.iter().all(|p| match p {
            // these patches just copy stuff into the new virtual dom tree, thus if we just keep
            // the old dom tree, the end result is the same
            CopyElement(_) | CopyListener(_)
            | CopyText(_) | Up
            => true,
            // these patches change the dom
            RemoveElement(_) | CreateElement { .. }
            | RemoveListener { .. } | AddListener { .. }
            | RemoveAttribute(_) | SetAttribute { .. }
            | RemoveText(_) | CreateText { .. } | ReplaceText { .. }
            => false,
        })
    }

    /// Apply the given PatchSet creating any elements under the given parent node. Events are
    /// dispatched via the given [`Dispatch`]er.
    ///
    /// [`Dispatch`]: ../app/trait.Dispatch.html
    pub fn apply<D>(self, parent: web_sys::Element, app: Rc<RefCell<D>>) -> Storage where
        Message: 'static + Clone,
        EventHandler<'a, Message>: Clone,
        D: Dispatch<Message> + 'static,
    {
        let mut node_stack: Vec<web_sys::Node> = vec![parent.unchecked_into()];

        let PatchSet { patches, mut storage } = self;

        let document = web_sys::window().expect("expected window")
            .document().expect("expected document");

        for p in patches.into_iter() {
            match p {
                Patch::RemoveElement(mut take) => {
                    node_stack.last()
                        .expect("no previous node")
                        .remove_child(&take())
                        .expect("failed to remove child node");
                }
                Patch::CreateElement { element } => {
                    let node = document.create_element(&element).expect("failed to create element");
                    storage.push(WebItem::Element(node.clone()));
                    node_stack.last()
                        .expect("no previous node")
                        .append_child(&node)
                        .expect("failed to append child node");
                    node_stack.push(node.into());
                }
                Patch::CopyElement(mut take) => {
                    let node = take();
                    storage.push(WebItem::Element(node.clone()));
                    node_stack.push(node.into());
                }
                Patch::RemoveText(mut take) => {
                    node_stack.last()
                        .expect("no previous node")
                        .remove_child(&take())
                        .expect("failed to remove child node");
                }
                Patch::ReplaceText { mut take, text } => {
                    let node = take();
                    node.set_data(&text);
                    storage.push(WebItem::Text(node.clone()));
                    node_stack.push(node.into());
                }
                Patch::CreateText { text } => {
                    let node = document.create_text_node(&text);
                    storage.push(WebItem::Text(node.clone()));
                    node_stack.last()
                        .expect("no previous node")
                        .append_child(&node)
                        .expect("failed to append child node");
                    node_stack.push(node.into());
                }
                Patch::CopyText(mut take) => {
                    let node = take();
                    storage.push(WebItem::Text(node.clone()));
                    node_stack.push(node.into());
                }
                Patch::SetAttribute { name, value } => {
                    let node = node_stack.last().expect("no previous node");

                    let mut set_value = false;

                    // handle the "value" attribute for certain elements
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
                                    D::dispatch(app.clone(), msg.clone())
                                }) as Box<FnMut(web_sys::Event)>
                            )
                        }
                        EventHandler::Fn(fun) => {
                            Closure::wrap(
                                Box::new(move |event| {
                                    D::dispatch(app.clone(), fun(event))
                                }) as Box<FnMut(web_sys::Event)>
                            )
                        }
                    };
                    let node = node_stack.last().expect("no previous node");
                    (node.as_ref() as &web_sys::EventTarget)
                        .add_event_listener_with_callback(&trigger, closure.as_ref().unchecked_ref())
                        .expect("failed to add event listener");
                    storage.push(WebItem::Closure(closure));
                }
                Patch::CopyListener(mut take) => {
                    storage.push(WebItem::Closure(take()));
                }
                Patch::RemoveListener { trigger, mut take } => {
                    let node = node_stack.last().expect("no previous node");
                    (node.as_ref() as &web_sys::EventTarget)
                        .remove_event_listener_with_callback(&trigger, take().as_ref().unchecked_ref())
                        .expect("failed to remove event listener");
                }
                Patch::Up => {
                    node_stack.pop();
                }
            }
        }

        storage
    }
}

impl<'a, Message> From<Vec<Patch<'a, Message>>> for PatchSet<'a, Message> {
    fn from(v: Vec<Patch<'a, Message>>) -> Self {
        PatchSet {
            patches: v,
            storage: vec![],
        }
    }
}

impl<'a, Message> IntoIterator for PatchSet<'a, Message> {
    type Item = Patch<'a, Message>;
    type IntoIter = ::std::vec::IntoIter<Patch<'a, Message>>;

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

    fn elem(name: &str) -> web_sys::Element {
        web_sys::window().expect("expected window")
            .document().expect("expected document")
            .create_element(name).expect("expected element")
    }

    #[derive(Clone)]
    enum Msg {}

    #[test]
    fn empty_patch_set_is_noop() {
        assert!(PatchSet::<Msg>::new().is_noop());
    }

    #[wasm_bindgen_test]
    fn noop_patch_set_is_noop() {
        let patch_set: PatchSet<Msg> = vec![
            Patch::CopyElement(Box::new(|| elem("test"))),
            Patch::Up,
        ].into();

        assert!(patch_set.is_noop());
    }

    #[test]
    fn not_noop() {
        let patch_set: PatchSet<Msg> = vec![
            Patch::CreateElement {
                element: "",
            },
        ].into();

        assert!(!patch_set.is_noop());
    }

    #[wasm_bindgen_test]
    fn copy_element() {
        let patch_set: PatchSet<Msg> = vec![
            Patch::CopyElement(Box::new(|| elem("test"))),
            Patch::Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

        assert!(!storage.is_empty());
    }

    #[wasm_bindgen_test]
    fn add_attribute() {
        use Patch::*;

        let patch_set: PatchSet<Msg> = vec![
            CopyElement(
                Box::new(|| {
                    let e = elem("test");
                    assert!(e.get_attribute("name").is_none());
                    e
                }),
            ),
            SetAttribute { name: "name", value: "value" },
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

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

        let patch_set: PatchSet<Msg> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "checked", value: "true" },
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

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

        let patch_set: PatchSet<Msg> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "disabled", value: "true" },
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

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

        let patch_set: PatchSet<Msg> = vec![
            CopyElement(
                Box::new(|| {
                    let e = elem("test");
                    e.set_attribute("name", "value").expect("setting attribute failed");
                    e
                }),
            ),
            RemoveAttribute("name"),
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

        let attribute = match storage[0] {
            WebItem::Element(ref e) => e.get_attribute("name"),
            _ => panic!("element not stored as expected"),
        };
        assert!(attribute.is_none());
    }

    #[wasm_bindgen_test]
    fn remove_attribute_checked() {
        use Patch::*;

        let patch_set: PatchSet<Msg> = vec![
            CopyElement(
                Box::new(|| {
                    let e = elem("input");
                    e.set_attribute("checked", "true").expect("setting attribute failed");
                    e
                }),
            ),
            RemoveAttribute("checked"),
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

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

        let patch_set: PatchSet<Msg> = vec![
            CopyElement(
                Box::new(|| {
                    let e = elem("input");
                    e.set_attribute("disabled", "true").expect("setting attribute failed");
                    e
                }),
            ),
            RemoveAttribute("disabled"),
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

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

        let patch_set: PatchSet<Msg> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "checked", value: "false" },
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

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

        let patch_set: PatchSet<Msg> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "disabled", value: "false" },
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

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

        let patch_set: PatchSet<Msg> = vec![
            CreateElement {
                element: "input",
            },
            SetAttribute { name: "autofocus", value: "false" },
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

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

        let patch_set: PatchSet<Msg> = vec![
            CreateElement {
                element: "option",
            },
            SetAttribute { name: "selected", value: "false" },
            Up,
        ].into();

        struct App {};
        impl Dispatch<Msg> for App {
            fn dispatch(_: Rc<RefCell<Self>>, _: Msg) {}
        }

        let app = Rc::new(RefCell::new(App {}));
        let parent = elem("div");
        let storage = patch_set.apply(parent, app);

        let element = match storage[0] {
            WebItem::Element(ref e) => e,
            _ => panic!("element not stored as expected"),
        };
        let option = element.dyn_ref::<web_sys::HtmlOptionElement>().expect("expected input element");

        assert!(!option.selected());
    }
}
