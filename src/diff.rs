//! Tools to get the difference between two virtual dom trees.

use std::fmt;
use std::iter;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use crate::patch::PatchSet;
use crate::patch::Patch;
use crate::vdom::DomItem;
use crate::vdom::WebItem;

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
    /// list of old keyed DomItems (and their storage)
    old_def: HashMap<u64, (Vec<DomItem<'a, Message, Command>>, Vec<&'a mut WebItem<Message>>)>,
    /// list of new keyed DomItems
    new_def: HashMap<u64, Vec<DomItem<'a, Message, Command>>>,
    /// if true (the default), keyed items will be deferred
    defer_keyed: bool,
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
            old_def: HashMap::new(),
            new_def: HashMap::new(),
            defer_keyed: true,
        }
    }

    fn no_defer(old: O, new: N, sto: S) -> Self {
        DiffImpl {
            old: old.into_iter(),
            new: new.into_iter(),
            sto: sto.into_iter(),
            patch_set: PatchSet::new(),
            old_def: HashMap::new(),
            new_def: HashMap::new(),
            defer_keyed: false,
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

        // now look for differences between keyed nodes
        for (key, (old_items, storage)) in self.old_def.drain() {
            if let Some(new_items) = self.new_def.remove(&key) {
                // there is something to diff, store it
                let mut ps = DiffImpl::no_defer(old_items, new_items, storage).diff();
                ps.root_key(key);
                self.patch_set.extend(ps);
            }
            else {
                // node is being removed, append the removal to the top level patch set
                let ps = DiffImpl::no_defer(old_items, iter::empty(), storage).diff();
                self.patch_set.extend(ps);
            }
        }

        // any nodes left in new need to be added
        for (key, new_items) in self.new_def.drain() {
            let mut ps = DiffImpl::no_defer(iter::empty(), new_items, iter::empty()).diff();
            ps.root_key(key);
            self.patch_set.extend(ps);
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
                DomItem::Element { name: o_element, key: Some(o_key) },
                DomItem::Element { name: n_element, key: Some(n_key) },
            ) if o_element == n_element && o_key == n_key => { // compare elements and keys
                let web_item = sto.next().expect("dom storage to match dom iter");

                // move the node
                patch_set.push(Patch::MoveElement(web_item));
                (old.next(), new.next())
            }
            (
                DomItem::Element { name: o_element, key: None },
                DomItem::Element { name: n_element, key: None },
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
                    patch_set.push(Patch::CopyText(web_item));
                }
                // text doesn't match, update it
                else {
                    patch_set.push(Patch::ReplaceText { take: web_item, text: n_text });
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
                DomItem::Component { msg: o_msg, create: o_create, key: Some(o_key) },
                DomItem::Component { msg: n_msg, create: n_create, key: Some(n_key) }
            )
            if o_create == n_create && o_key == n_key
            => { // compare keyed components
                let web_item = sto.next().expect("dom storage to match dom iter");

                // message matches, copy the storage
                if o_msg == n_msg {
                    patch_set.push(Patch::MoveComponent(web_item));
                }
                // message doesn't match, dispatch it to the component
                else {
                    patch_set.push(Patch::MupdateComponent { take: web_item, msg: n_msg });
                }

                (old.next(), new.next())
            }
            (
                DomItem::Component { msg: o_msg, create: o_create, key: None },
                DomItem::Component { msg: n_msg, create: n_create, key: None }
            ) if o_create == n_create => { // compare components
                let web_item = sto.next().expect("dom storage to match dom iter");

                // message matches, copy the storage
                if o_msg == n_msg {
                    patch_set.push(Patch::CopyComponent(web_item));
                }
                // message doesn't match, dispatch it to the component
                else {
                    patch_set.push(Patch::UpdateComponent { take: web_item, msg: n_msg });
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
                    patch_set.push(Patch::RemoveListener { trigger: o_trigger, take: web_item });

                    // add new listener
                    patch_set.push(Patch::AddListener { trigger: n_trigger, handler: n_handler.into() });
                }
                else {
                    // just copy the existing listener
                    patch_set.push(Patch::CopyListener(web_item));
                }

                (old.next(), new.next())
            }
            (DomItem::Up, DomItem::Up) => { // end of two items
                let _ = sto.next().expect("dom storage to match dom iter");
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
           DomItem::Element { key: Some(_), .. }
            if self.defer_keyed
            => {
                self.defer_remove_sub_tree(item, None)
            }
            DomItem::Element { .. } => {
                let web_item = sto.next().expect("dom storage to match dom iter");
                patch_set.push(Patch::RemoveElement(web_item));
                self.remove_sub_tree()
            }
            DomItem::Text(_) => {
                let web_item = sto.next().expect("dom storage to match dom iter");
                patch_set.push(Patch::RemoveText(web_item));
                self.remove_sub_tree()
            }
            DomItem::Component { key: Some(_), .. }
            if self.defer_keyed
            => {
                self.defer_remove_sub_tree(item, None)
            }
            DomItem::Component { .. } => {
                let web_item = sto.next().expect("dom storage to match dom iter");
                patch_set.push(Patch::RemoveComponent(web_item));
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
            // ignore
            DomItem::Key(_) => {
                old.next()
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
            DomItem::Element { key: Some(_), .. }
            if self.defer_keyed
            => {
                self.defer_add_sub_tree(item, None)
            }
            DomItem::Element { name: element, .. } => {
                patch_set.push(Patch::CreateElement { element });
                self.add_sub_tree()
            }
            DomItem::Text(text) => {
                patch_set.push(Patch::CreateText { text });
                self.add_sub_tree()
            }
            DomItem::Component { key: Some(_), .. }
            if self.defer_keyed
            => {
                self.defer_add_sub_tree(item, None)
            }
            DomItem::Component { msg, create, .. } => {
                patch_set.push(Patch::CreateComponent { msg, create });
                self.add_sub_tree()
            }
            DomItem::Key(k) => {
                patch_set.push(Patch::ReferenceKey(k));
                new.next()
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
                Some(item @ DomItem::Element { key: Some(_), .. })
                if self.defer_keyed
                => {
                    self.defer_add_sub_tree(item, None)
                }
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
                Some(item @ DomItem::Component { key: Some(_), .. })
                if self.defer_keyed => {
                    self.defer_add_sub_tree(item, None)
                }
                Some(DomItem::Component { msg, create, .. }) => {
                    self.patch_set.push(Patch::CreateComponent { msg, create });
                    depth += 1;
                    self.new.next()
                }
                Some(DomItem::Key(k)) => {
                    self.patch_set.push(Patch::ReferenceKey(k));
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
                // keyed child element: defer
                Some(item @ DomItem::Element { key: Some(_), .. })
                if self.defer_keyed
                => {
                    self.defer_remove_sub_tree(item, None)
                }
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
                // keyed component: defer
                Some(item @ DomItem::Component { key: Some(_), .. })
                if self.defer_keyed
                => {
                    self.defer_remove_sub_tree(item, None)
                }
                // component: remove it from storage and the dom
                Some(DomItem::Component { .. }) => {
                    let web_item = self.sto.next().expect("dom storage to match dom iter");
                    self.patch_set.push(Patch::RemoveComponent(web_item));
                    depth += 1;
                    self.old.next()
                }
                // key reference: ignore
                Some(DomItem::Key(_)) => {
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
                    let _ = self.sto.next().expect("dom storage to match dom iter");
                    depth -= 1;
                    self.old.next()
                }
                // end of node: stop processing
                Some(DomItem::Up) => {
                    let _ = self.sto.next().expect("dom storage to match dom iter");
                    return self.old.next();
                }
                o @ None => {
                    return o;
                }
            }
        }
    }

    /// Track the items in this sub tree.
    ///
    /// Expected to be called where `old.next()` just returned a node that may have children. This will
    /// handle removing nodes from storage, up to the matching `DomItem::Up` entry.
    fn defer_remove_sub_tree(
        &mut self,
        item: DomItem<'a, Message, Command>,
        mut deferred: Option<(&mut Vec<DomItem<'a, Message, Command>>, &mut Vec<&'a mut WebItem<Message>>)>,
    ) -> Option<DomItem<'a, Message, Command>>
    {
        let key = match item {
            DomItem::Element { key: Some(key), .. } => {
                let web_item = self.sto.next().expect("dom storage to match dom iter");
                match self.old_def.entry(key) {
                    Entry::Occupied(_) => {
                        // XXX log the error to the debug console? warn?
                        if let Some((ref mut deferred_items, ref mut deferred_storage)) = deferred {
                            deferred_items.push(item);
                            deferred_storage.push(web_item);
                            None
                        }
                        else {
                            self.patch_set.push(Patch::RemoveElement(web_item));
                            return self.remove_sub_tree();
                        }
                    }
                    Entry::Vacant(e) => {
                        if let Some((ref mut deferred_items, _)) = deferred {
                            deferred_items.push(DomItem::Key(key));
                        }

                        e.insert((vec![item], vec![web_item]));
                        Some(key)
                    }
                }
            }
            DomItem::Component { key: Some(key), .. } => {
                let web_item = self.sto.next().expect("dom storage to match dom iter");
                match self.old_def.entry(key) {
                    Entry::Occupied(_) => {
                        // XXX log the error to the debug console? warn?
                        if let Some((ref mut deferred_items, ref mut deferred_storage)) = deferred {
                            deferred_items.push(item);
                            deferred_storage.push(web_item);
                            None
                        }
                        else {
                            self.patch_set.push(Patch::RemoveComponent(web_item));
                            return self.remove_sub_tree();
                        }
                    }
                    Entry::Vacant(e) => {
                        if let Some((ref mut deferred_items, _)) = deferred {
                            deferred_items.push(DomItem::Key(key));
                        }

                        e.insert((vec![item], vec![web_item]));
                        Some(key)
                    }
                }
            }
            _ => {
                panic!("expected keyed element or component");
            }
        };

        let mut def_items = vec![];
        let mut def_storage = vec![];

        // this will copy the entire sub tree for later when we compare keyed elements
        let mut item = self.old.next();
        let mut depth = 0;
        let next = loop {
            if let Some(i) = item {
                item = match i {
                    // child element: remove from storage, track sub-tree depth
                    DomItem::Element { key: Some(_), .. } => {
                        self.defer_remove_sub_tree(i, Some((&mut def_items, &mut def_storage)))
                    }
                    // child element: remove from storage, track sub-tree depth
                    DomItem::Element { .. } => {
                        def_storage.push(self.sto.next().expect("dom storage to match dom iter"));
                        def_items.push(i);
                        depth += 1;
                        self.old.next()
                    }
                    // child text: remove from storage, track sub-tree depth
                    DomItem::Text(_) => {
                        def_storage.push(self.sto.next().expect("dom storage to match dom iter"));
                        def_items.push(i);
                        depth += 1;
                        self.old.next()
                    }
                    // keyed component: defer
                    DomItem::Component { key: Some(_), .. } => {
                        self.defer_remove_sub_tree(i, Some((&mut def_items, &mut def_storage)))
                    }
                    // component: remove it from storage and the dom
                    DomItem::Component { .. } => {
                        def_storage.push(self.sto.next().expect("dom storage to match dom iter"));
                        def_items.push(i);
                        depth += 1;
                        self.old.next()
                    }
                    // key reference: defer
                    DomItem::Key(_) => {
                        def_items.push(i);
                        self.old.next()
                    }
                    // event: remove from storage
                    DomItem::Event { .. } => {
                        def_storage.push(self.sto.next().expect("dom storage to match dom iter"));
                        def_items.push(i);
                        self.old.next()
                    }
                    // innerHtml: ignore
                    DomItem::UnsafeInnerHtml(_) => {
                        def_items.push(i);
                        self.old.next()
                    }
                    // attribute: ignore
                    DomItem::Attr { .. } => {
                        def_items.push(i);
                        self.old.next()
                    }
                    // end of child: track sub-tree depth
                    DomItem::Up if depth > 0 => {
                        def_storage.push(self.sto.next().expect("dom storage to match dom iter"));
                        def_items.push(i);
                        depth -= 1;
                        self.old.next()
                    }
                    // end of node: stop processing
                    DomItem::Up => {
                        def_storage.push(self.sto.next().expect("dom storage to match dom iter"));
                        def_items.push(i);
                        break self.old.next();
                    }
                };
            }
            else {
                break None;
            }
        };

        // if this sub tree has a unique key, add the deferred items to the sub tree for that key
        if let Some(key) = key {
            let (
                ref mut items,
                ref mut storage
            ) = self.old_def.get_mut(&key)
                .expect("key should exist");

            items.extend(def_items);
            storage.extend(def_storage);
        }
        // otherwise add the defeferred items to the given vecs
        else if let Some((deferred_items, deferred_storage)) = deferred{
            deferred_items.extend(def_items);
            deferred_storage.extend(def_storage);
        }

        next
    }

    /// Defer processing of this keyed sub tree.
    ///
    /// Expected to be called where `new.next()` just returned a node that may have children.
    fn defer_add_sub_tree(
        &mut self,
        item: DomItem<'a, Message, Command>,
        mut deferred_items: Option<&mut Vec<DomItem<'a, Message, Command>>>,
    ) -> Option<DomItem<'a, Message, Command>>
    {
        let key = match item {
            DomItem::Element { name: element, key: Some(key) } => {
                match self.new_def.entry(key) {
                    Entry::Occupied(_) => {
                        // XXX log the error to the debug console? warn?
                        if let Some(ref mut deferred_items) = deferred_items {
                            deferred_items.push(item);
                            None
                        }
                        else {
                            self.patch_set.push(Patch::CreateElement { element });
                            return self.add_sub_tree();
                        }
                    }
                    Entry::Vacant(e) => {
                        if let Some(ref mut deferred_items) = deferred_items {
                            deferred_items.push(DomItem::Key(key));
                        }
                        else {
                            self.patch_set.push(Patch::ReferenceKey(key));
                        }
                        e.insert(vec![item]);
                        Some(key)
                    }
                }
            }
            DomItem::Component { ref msg, create, key: Some(key) } => {
                match self.new_def.entry(key) {
                    Entry::Occupied(_) => {
                        // XXX log the error to the debug console? warn?
                        if let Some(ref mut deferred_items) = deferred_items {
                            deferred_items.push(item);
                            None
                        }
                        else {
                            self.patch_set.push(Patch::CreateComponent { msg: msg.clone(), create });
                            return self.add_sub_tree();
                        }
                    }
                    Entry::Vacant(e) => {
                        if let Some(ref mut deferred_items) = deferred_items {
                            deferred_items.push(DomItem::Key(key));
                        }
                        else {
                            self.patch_set.push(Patch::ReferenceKey(key));
                        }
                        e.insert(vec![item]);
                        Some(key)
                    }
                }
            }
            _ => {
                panic!("expected keyed element or component");
            }
        };

        let mut def = vec![];

        // this will copy the entire sub tree for later when we compare keyed elements
        let mut depth = 0;
        let mut item = self.new.next();
        let next = loop {
            if let Some(i) = item {
                item = match i {
                    // keyed child element: defer
                    DomItem::Element { key: Some(_), .. } => {
                        self.defer_add_sub_tree(i, Some(&mut def))
                    }
                    // child element: track depth
                    DomItem::Element { .. } => {
                        def.push(i);
                        depth += 1;
                        self.new.next()
                    }
                    // child text: track depth
                    DomItem::Text(_) => {
                        def.push(i);
                        depth += 1;
                        self.new.next()
                    }
                    // keyed component: defer
                    DomItem::Component { key: Some(_), .. } => {
                        self.defer_add_sub_tree(i, Some(&mut def))
                    }
                    // component: track depth
                    DomItem::Component { .. } => {
                        def.push(i);
                        depth += 1;
                        self.new.next()
                    }
                    // key reference: defer
                    DomItem::Key(_) => {
                        def.push(i);
                        self.new.next()
                    }
                    // event: ignore
                    DomItem::Event { .. } => {
                        def.push(i);
                        self.new.next()
                    }
                    // innerHtml: ignore
                    DomItem::UnsafeInnerHtml(_) => {
                        def.push(i);
                        self.new.next()
                    }
                    // attribute: ignore
                    DomItem::Attr { .. } => {
                        def.push(i);
                        self.new.next()
                    }
                    // end of child: track sub-tree depth
                    DomItem::Up if depth > 0 => {
                        def.push(i);
                        depth -= 1;
                        self.new.next()
                    }
                    // end of node: stop processing
                    DomItem::Up => {
                        def.push(i);
                        break self.new.next();
                    }
                };
            }
            else {
                break None;
            }
        };

        // if this sub tree has a unique key, add the deferred items to the sub tree for that key
        if let Some(key) = key {
            let items = self.new_def.get_mut(&key)
                .expect("key should exist");

            items.extend(def);
        }
        // otherwise add the defeferred items to the given vec
        else if let Some(deferred_items) = deferred_items {
            deferred_items.extend(def);
        }

        next
    }
} // end of impl DiffImpl
