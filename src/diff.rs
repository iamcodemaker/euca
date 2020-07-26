//! Tools to get the difference between two virtual dom trees.

use std::fmt;
use wasm_bindgen::prelude::Closure;
use crate::patch::PatchSet;
use crate::patch::Patch;
use crate::vdom::DomItem;
use crate::vdom::WebItem;
use crate::component::Component;

fn take_text<'a, Message>(item: &'a mut WebItem<Message>) -> Box<dyn FnMut() -> web_sys::Text + 'a> {
    Box::new(move || {
        match item.take() {
            WebItem::Text(i) => i,
            _ => panic!("storage type mismatch"),
        }
    })
}

fn take_closure<'a, Message>(item: &'a mut WebItem<Message>) -> Box<dyn FnMut() -> Closure<dyn FnMut(web_sys::Event)> + 'a> {
    Box::new(move || {
        match item.take() {
            WebItem::Closure(i) => i,
            _ => panic!("storage type mismatch"),
        }
    })
}

fn take_component<'a, Message>(item: &'a mut WebItem<Message>) -> Box<dyn FnMut() -> Box<dyn Component<Message>> + 'a> {
    Box::new(move || {
        match item.take() {
            WebItem::Component(i) => i,
            _ => panic!("storage type mismatch"),
        }
    })
}

/// Return the series of steps required to move from the given old/existing virtual dom to the
/// given new virtual dom.
pub fn diff<'a, Message, Command, O, N, S>(
    old: O,
    new: N,
    storage: S,
)
-> PatchSet<'a, Message, Command>
where
    Message: 'a + PartialEq + Clone + fmt::Debug,
    O: IntoIterator<Item = DomItem<'a, Message, Command>>,
    N: IntoIterator<Item = DomItem<'a, Message, Command>>,
    S: IntoIterator<Item = &'a mut WebItem<Message>>,
{
    DiffImpl::new(old, new, storage).diff()
}

struct DiffImpl<'a, Message, Command, O, N, S>
where
    Message: 'a + PartialEq + Clone + fmt::Debug,
    O: IntoIterator<Item = DomItem<'a, Message, Command>>,
    N: IntoIterator<Item = DomItem<'a, Message, Command>>,
    S: IntoIterator<Item = &'a mut WebItem<Message>>,
{
    old: O::IntoIter,
    new: N::IntoIter,
    sto: S::IntoIter,
    patch_set: PatchSet<'a, Message, Command>,
}

impl<'a, Message, Command, O, N, S>
DiffImpl<'a, Message, Command, O, N, S>
where
    Message: 'a + PartialEq + Clone + fmt::Debug,
    O: IntoIterator<Item = DomItem<'a, Message, Command>>,
    N: IntoIterator<Item = DomItem<'a, Message, Command>>,
    S: IntoIterator<Item = &'a mut WebItem<Message>>,
{
    fn new(old: O, new: N, sto: S) -> Self {
        DiffImpl {
            old: old.into_iter(),
            new: new.into_iter(),
            sto: sto.into_iter(),
            patch_set: PatchSet::new(),
        }
    }

    /// Return the series of steps required to move from the given old/existing virtual dom to the
    /// given new virtual dom.
    pub fn diff(mut self) -> PatchSet<'a, Message, Command> {
        let mut o_item = self.old.next();
        let mut n_item = self.new.next();

        loop {
            match (o_item.take(), n_item.take()) {
                (None, None) => { // return patch set
                    break;
                }
                (None, Some(n)) => { // create remaining new nodes
                    n_item = self.add(n);
                }
                (Some(o), None) => { // delete remaining old nodes
                    o_item = self.remove(o);
                }
                (Some(o), Some(n)) => { // compare nodes
                    let (o_next, n_next) = self.compare(o, n);
                    o_item = o_next;
                    n_item = n_next;
                }
            }
        }

        self.patch_set
    }


    /// Compare two items.
    fn compare(
        &mut self,
        o_item: DomItem<'a, Message, Command>,
        n_item: DomItem<'a, Message, Command>,
    ) -> (Option<DomItem<'a, Message, Command>>, Option<DomItem<'a, Message, Command>>)
    {
        let patch_set = &mut self.patch_set;
        let sto = &mut self.sto;
        let old = &mut self.old;
        let new = &mut self.new;

        match (o_item, n_item) {
            (
                DomItem::Element { name: o_element, .. },
                DomItem::Element { name: n_element, .. },
            ) if o_element == n_element => { // compare elements
                let web_item = sto.next().expect("dom storage to match dom iter");

                // copy the node
                patch_set.push(Patch::CopyElement(web_item));
                (old.next(), new.next())
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

                (old.next(), new.next())
            }
            (
                DomItem::UnsafeInnerHtml(o_html),
                DomItem::UnsafeInnerHtml(n_html)
            ) => { // compare inner html
                if o_html != n_html {
                    patch_set.push(Patch::SetInnerHtml(n_html));
                }

                (old.next(), new.next())
            }
            (
                DomItem::Component { msg: o_msg, create: o_create },
                DomItem::Component { msg: n_msg, create: n_create }
            ) if o_create == n_create => { // compare components
                let web_item = sto.next().expect("dom storage to match dom iter");

                // message matches, copy the storage
                if o_msg == n_msg {
                    patch_set.push(Patch::CopyComponent(take_component(web_item)));
                }
                // message doesn't match, dispatch it to the component
                else {
                    patch_set.push(Patch::UpdateComponent { take: take_component(web_item), msg: n_msg });
                }

                (old.next(), new.next())
            }
            (
                DomItem::Attr { name: o_name, value: o_value },
                DomItem::Attr { name: n_name, value: n_value }
            ) => { // compare attributes
                // names are different
                if o_name != n_name {
                    // remove old attribute
                    patch_set.push(Patch::RemoveAttribute(o_name));

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

                (old.next(), new.next())
            }
            (
                DomItem::Event { trigger: o_trigger, handler: o_handler },
                DomItem::Event { trigger: n_trigger, handler: n_handler }
            ) => { // compare event listeners
                let web_item = sto.next().expect("dom storage to match dom iter");

                if o_trigger != n_trigger || o_handler != n_handler {
                    // remove old listener
                    patch_set.push(Patch::RemoveListener { trigger: o_trigger, take: take_closure(web_item) });

                    // add new listener
                    patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into() });
                }
                else {
                    // just copy the existing listener
                    patch_set.push(Patch::CopyListener(take_closure(web_item)));
                }

                (old.next(), new.next())
            }
            (DomItem::Up, DomItem::Up) => { // end of two items
                patch_set.push(Patch::Up);
                (old.next(), new.next())
            }
            (o, n) => { // no match
                // remove the old item
                let o_next = self.remove(o);

                // add the new item
                let n_next = self.add(n);

                (o_next, n_next)
            }
        }
    }

    /// Add patches to remove this item.
    fn remove(
        &mut self,
        item: DomItem<'a, Message, Command>,
    ) -> Option<DomItem<'a, Message, Command>>
    {
        let patch_set = &mut self.patch_set;
        let sto = &mut self.sto;
        let old = &mut self.old;

        match item {
            DomItem::Element { .. } => {
                let web_item = sto.next().expect("dom storage to match dom iter");
                patch_set.push(Patch::RemoveElement(web_item));
                self.remove_sub_tree()
            }
            DomItem::Text(_) => {
                let web_item = sto.next().expect("dom storage to match dom iter");
                patch_set.push(Patch::RemoveText(take_text(web_item)));
                self.remove_sub_tree()
            }
            DomItem::Component { .. } => {
                let web_item = sto.next().expect("dom storage to match dom iter");
                patch_set.push(Patch::RemoveComponent(take_component(web_item)));
                self.remove_sub_tree()
            }
            DomItem::UnsafeInnerHtml(_) => {
                patch_set.push(Patch::UnsetInnerHtml);
                old.next()
            }
            DomItem::Event { .. } => {
                let _ = sto.next().expect("dom storage to match dom iter");
                old.next()
            }
            // ignore attributes
            DomItem::Attr { .. } => {
                old.next()
            }
            // this should only be possible when comparing two nodes, and in that case we expect this
            // to effectively be a noop while we add items to the node we are comparing to. When
            // removing entire elements, remove_sub_tree() is called above and this condition is never
            // hit.
            DomItem::Up => {
                Some(item)
            }
        }
    }

    /// Add patches to add this item.
    fn add(
        &mut self,
        item: DomItem<'a, Message, Command>,
    ) -> Option<DomItem<'a, Message, Command>>
    {
        let patch_set = &mut self.patch_set;
        let new = &mut self.new;

        match item {
            DomItem::Element { name: element, .. } => {
                patch_set.push(Patch::CreateElement { element });
                self.add_sub_tree()
            }
            DomItem::Text(text) => {
                patch_set.push(Patch::CreateText { text });
                self.add_sub_tree()
            }
            DomItem::Component { msg, create } => {
                patch_set.push(Patch::CreateComponent { msg, create });
                self.add_sub_tree()
            }
            DomItem::UnsafeInnerHtml(html) => {
                patch_set.push(Patch::SetInnerHtml(html));
                new.next()
            }
            DomItem::Attr { name, value } => {
                patch_set.push(Patch::SetAttribute { name, value });
                new.next()
            }
            DomItem::Event { trigger, handler } => {
                patch_set.push(Patch::AddListener { trigger, handler: handler.into() });
                new.next()
            }
            // this should only be possible when comparing two nodes, and in that case we expect this
            // to effectively be a noop while we remove items from the node we are comparing to. When
            // adding entire elements, add_sub_tree() is called above and this condition is never hit.
            DomItem::Up => {
                Some(item)
            }
        }
    }

    /// Add this entire element tree.
    ///
    /// Expected to be called where `new.next()` just returned a node that may have children. This will
    /// handle creating all of the nodes up to the matching `DomItem::Up` entry.
    fn add_sub_tree(&mut self)
    -> Option<DomItem<'a, Message, Command>>
    {
        let mut depth = 0;
        let mut item = self.new.next();
        loop {
            item = match item {
                Some(DomItem::Element { name: element, .. }) => {
                    self.patch_set.push(Patch::CreateElement { element });
                    depth += 1;
                    self.new.next()
                }
                Some(DomItem::Text(text)) => {
                    self.patch_set.push(Patch::CreateText { text });
                    depth += 1;
                    self.new.next()
                }
                Some(DomItem::Component { msg, create }) => {
                    self.patch_set.push(Patch::CreateComponent { msg, create });
                    depth += 1;
                    self.new.next()
                }
                Some(DomItem::UnsafeInnerHtml(html)) => {
                    self.patch_set.push(Patch::SetInnerHtml(html));
                    self.new.next()
                }
                Some(DomItem::Event { trigger, handler }) => {
                    self.patch_set.push(Patch::AddListener { trigger, handler: handler.into() });
                    self.new.next()
                }
                Some(DomItem::Attr { name, value }) => {
                    self.patch_set.push(Patch::SetAttribute { name, value });
                    self.new.next()
                }
                Some(DomItem::Up) if depth > 0 => {
                    self.patch_set.push(Patch::Up);
                    depth -= 1;
                    self.new.next()
                }
                Some(DomItem::Up) => {
                    self.patch_set.push(Patch::Up);
                    return self.new.next();
                }
                n @ None => {
                    return n;
                }
            }
        }
    }

    /// Skip the items in this sub tree.
    ///
    /// Expected to be called where `old.next()` just returned a node that may have children. This will
    /// handle removing nodes from storage, up to the matching `DomItem::Up` entry.
    fn remove_sub_tree(&mut self)
    -> Option<DomItem<'a, Message, Command>>
    {
        // skip the rest of the items in the old tree for this element, this
        // will cause attributes and such to be created on the new element
        let mut depth = 0;
        let mut item = self.old.next();
        loop {
            item = match item {
                // child element: remove from storage, track sub-tree depth
                Some(DomItem::Element { .. }) => {
                    let _ = self.sto.next().expect("dom storage to match dom iter");
                    depth += 1;
                    self.old.next()
                }
                // child text: remove from storage, track sub-tree depth
                Some(DomItem::Text(_)) => {
                    let _ = self.sto.next().expect("dom storage to match dom iter");
                    depth += 1;
                    self.old.next()
                }
                // component: remove it from storage and the dom
                Some(DomItem::Component { .. }) => {
                    let web_item = self.sto.next().expect("dom storage to match dom iter");
                    self.patch_set.push(Patch::RemoveComponent(take_component(web_item)));
                    depth += 1;
                    self.old.next()
                }
                // event: remove from storage
                Some(DomItem::Event { .. }) => {
                    let _ = self.sto.next().expect("dom storage to match dom iter");
                    self.old.next()
                }
                // innerHtml: ignore
                Some(DomItem::UnsafeInnerHtml(_)) => {
                    self.old.next()
                }
                // attribute: ignore
                Some(DomItem::Attr { .. }) => {
                    self.old.next()
                }
                // end of child: track sub-tree depth
                Some(DomItem::Up) if depth > 0 => {
                    depth -= 1;
                    self.old.next()
                }
                // end of node: stop processing
                Some(DomItem::Up) => {
                    return self.old.next();
                }
                o @ None => {
                    return o;
                }
            }
        }
    }

} // end of impl DiffImpl
