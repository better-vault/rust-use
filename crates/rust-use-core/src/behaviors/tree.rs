use super::{action, dispatch, param, param_opt};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Page, Query, Value};

#[derive(Debug, Clone)]
struct Node<T> {
    label: String,
    data: T,
    children: Vec<Self>,
    expanded: bool,
}

impl<T> Node<T> {
    fn new(label: &str, data: T) -> Self {
        Self {
            label: label.into(),
            data,
            children: Vec::new(),
            expanded: false,
        }
    }

    fn expanded_count(&self) -> usize {
        usize::from(self.expanded)
            + self
                .children
                .iter()
                .map(Self::expanded_count)
                .sum::<usize>()
    }

    fn reveal(nodes: &mut [Self], path: &str, expand_target: bool) -> Option<usize> {
        let (label, rest) = path
            .split_once('/')
            .map_or((path, None), |(label, rest)| (label, Some(rest)));
        let node = nodes.iter_mut().find(|node| node.label == label)?;
        let opened = match rest {
            Some(rest) => Self::reveal(&mut node.children, rest, expand_target)?,
            None if !expand_target => return Some(0),
            None => 0,
        };
        let was_expanded = std::mem::replace(&mut node.expanded, true);
        Some(opened + usize::from(!was_expanded))
    }
}

/// An ordered tree with expansion and selection addressed by path.
///
/// Paths join nonempty labels with `/`. Root nodes have depth zero.
#[derive(Debug, Clone)]
pub struct Tree<T = ()> {
    roots: Vec<Node<T>>,
    selected: Option<String>,
    expanded_count: usize,
}

/// A row in the visible portion of a [`Tree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow<'a> {
    /// Nesting level, starting at zero for root nodes.
    pub depth: usize,
    /// Full path from the root.
    pub path: String,
    /// The node's label.
    pub label: &'a str,
    /// Whether the node has any children.
    pub has_children: bool,
    /// Whether the node is expanded.
    pub expanded: bool,
}

impl<T> Default for Tree<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Tree<T> {
    /// Create an empty tree.
    pub fn new() -> Self {
        Self {
            roots: Vec::new(),
            selected: None,
            expanded_count: 0,
        }
    }

    /// Create a collapsed tree from paths, using default data for every node.
    /// Siblings follow the order in which they first appear. Paths with empty labels are skipped.
    pub fn from_paths<I: IntoIterator<Item = S>, S: AsRef<str>>(paths: I) -> Self
    where
        T: Default,
    {
        let mut tree = Self::new();
        for path in paths {
            let path = path.as_ref();
            if !valid_path(path) {
                continue;
            }
            let mut siblings = &mut tree.roots;
            for label in path.split('/') {
                let index = child_index(siblings, label).unwrap_or_else(|| {
                    siblings.push(Node::new(label, T::default()));
                    siblings.len() - 1
                });
                siblings = &mut siblings[index].children;
            }
        }
        tree
    }

    /// Append a node to its siblings or replace the data at an existing path.
    ///
    /// # Errors
    /// Rejects paths with empty labels or a missing parent.
    pub fn insert(&mut self, path: &str, data: T) -> Result<(), ActionError> {
        if !valid_path(path) {
            return Err(ActionError::Rejected(format!(
                "invalid path {path:?}; labels must be nonempty and '/' separates labels"
            )));
        }
        let (parent, label) = split_parent(path);
        let Some(siblings) = self.children_mut(parent) else {
            return Err(self.missing_path(parent));
        };
        if let Some(node) = siblings.iter_mut().find(|node| node.label == label) {
            node.data = data;
        } else {
            siblings.push(Node::new(label, data));
        }
        Ok(())
    }

    /// Remove a node and its subtree, clearing selection inside that subtree.
    pub fn remove(&mut self, path: &str) -> Option<T> {
        if !valid_path(path) {
            return None;
        }
        let (parent, label) = split_parent(path);
        let siblings = self.children_mut(parent)?;
        let index = child_index(siblings, label)?;
        let node = siblings.remove(index);
        self.expanded_count -= node.expanded_count();
        if self.selected().is_some_and(|sel| in_subtree(sel, path)) {
            self.selected = None;
        }
        Some(node.data)
    }

    /// Borrow the data at a path.
    pub fn get(&self, path: &str) -> Option<&T> {
        self.node(path).map(|node| &node.data)
    }

    /// Expand a node and all of its ancestors.
    ///
    /// # Errors
    /// Rejects unknown paths and lists the nearest ancestor's children.
    pub fn expand(&mut self, path: &str) -> Result<(), ActionError> {
        self.reveal(path, true)
    }

    /// Collapse a node, preserving expansion and selection in its subtree.
    ///
    /// # Errors
    /// Rejects unknown paths and lists the nearest ancestor's children.
    pub fn collapse(&mut self, path: &str) -> Result<(), ActionError> {
        let Some(node) = self.node_mut(path) else {
            return Err(self.missing_path(path));
        };
        let was_expanded = std::mem::take(&mut node.expanded);
        self.expanded_count -= usize::from(was_expanded);
        Ok(())
    }

    /// Select a node and expand its ancestors to reveal it.
    ///
    /// # Errors
    /// Rejects unknown paths and lists the nearest ancestor's children.
    pub fn select(&mut self, path: &str) -> Result<(), ActionError> {
        self.reveal(path, false)?;
        self.selected = Some(path.into());
        Ok(())
    }

    /// Borrow the selected path, if any.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Whether a node exists and is expanded.
    pub fn is_expanded(&self, path: &str) -> bool {
        self.node(path).is_some_and(|node| node.expanded)
    }

    /// Collect visible rows in depth-first sibling order.
    pub fn visible_rows(&self) -> Vec<TreeRow<'_>> {
        self.rows(true).collect()
    }

    /// Move a node and its subtree, updating any selected path in the subtree.
    /// Use an empty parent path for the root. The index is the final sibling index,
    /// and `None` appends.
    ///
    /// # Errors
    /// Rejects unknown paths, moves into the node's own subtree, duplicate
    /// destination labels, and out-of-range indices.
    pub fn move_node(
        &mut self,
        path: &str,
        parent: &str,
        at: Option<usize>,
    ) -> Result<(), ActionError> {
        if !valid_path(path) {
            return Err(self.missing_path(path));
        }
        let (old_parent, label) = split_parent(path);
        let Some(index) = self
            .children(old_parent)
            .and_then(|siblings| child_index(siblings, label))
        else {
            return Err(self.missing_path(path));
        };
        let destination = self
            .children(parent)
            .ok_or_else(|| self.missing_path(parent))?;
        if in_subtree(parent, path) {
            return Err(ActionError::Rejected(format!(
                "cannot move {path} into its own subtree"
            )));
        }
        let same_parent = parent == old_parent;
        if !same_parent && child_index(destination, label).is_some() {
            return Err(ActionError::Rejected(format!(
                "a node already exists at {}",
                join_path(parent, label)
            )));
        }
        let end = destination.len() - usize::from(same_parent);
        let at = at.unwrap_or(end);
        if at > end {
            return Err(ActionError::Rejected(format!(
                "destination index {at} out of range (expected 0..={end})"
            )));
        }

        let selected = self
            .selected()
            .filter(|sel| !same_parent && in_subtree(sel, path))
            .map(|sel| join_path(parent, label) + &sel[path.len()..]);
        let Some(siblings) = self.children_mut(old_parent) else {
            return Err(self.missing_path(path));
        };
        let node = siblings.remove(index);
        // The checked destination survives removal because it is outside the moved subtree.
        let Some(destination) = self.children_mut(parent) else {
            return Err(self.missing_path(parent));
        };
        destination.insert(at, node);
        if selected.is_some() {
            self.selected = selected;
        }
        Ok(())
    }

    fn reveal(&mut self, path: &str, expand_target: bool) -> Result<(), ActionError> {
        let opened = Node::reveal(&mut self.roots, path, expand_target)
            .ok_or_else(|| self.missing_path(path))?;
        self.expanded_count += opened;
        Ok(())
    }

    fn node(&self, path: &str) -> Option<&Node<T>> {
        let mut labels = path.split('/');
        let first = labels.next()?;
        let node = self.roots.iter().find(|node| node.label == first)?;
        labels.try_fold(node, |node, label| {
            node.children.iter().find(|child| child.label == label)
        })
    }

    fn node_mut(&mut self, path: &str) -> Option<&mut Node<T>> {
        let mut labels = path.split('/');
        let first = labels.next()?;
        let node = self.roots.iter_mut().find(|node| node.label == first)?;
        labels.try_fold(node, |node, label| {
            node.children.iter_mut().find(|child| child.label == label)
        })
    }

    fn children(&self, path: &str) -> Option<&[Node<T>]> {
        if path.is_empty() {
            Some(&self.roots)
        } else {
            self.node(path).map(|node| node.children.as_slice())
        }
    }

    fn children_mut(&mut self, path: &str) -> Option<&mut Vec<Node<T>>> {
        if path.is_empty() {
            Some(&mut self.roots)
        } else {
            self.node_mut(path).map(|node| &mut node.children)
        }
    }

    fn missing_path(&self, path: &str) -> ActionError {
        let mut siblings = self.roots.as_slice();
        let mut prefix_len = 0;
        for label in path.split('/') {
            let Some(node) = siblings.iter().find(|node| node.label == label) else {
                break;
            };
            prefix_len += label.len() + usize::from(prefix_len != 0);
            siblings = &node.children;
        }
        let ancestor = if prefix_len == 0 {
            "<root>"
        } else {
            &path[..prefix_len]
        };
        let labels: Vec<_> = siblings.iter().map(|node| node.label.as_str()).collect();
        ActionError::Rejected(format!(
            "no node at {path}; children of {ancestor}: {}",
            labels.join(", ")
        ))
    }

    fn rows(&self, visible_only: bool) -> impl Iterator<Item = TreeRow<'_>> {
        let mut stack = vec![(0, self.roots.iter())];
        let mut path = String::new();
        std::iter::from_fn(move || {
            while let Some((parent_len, siblings)) = stack.last_mut() {
                path.truncate(*parent_len);
                if let Some(node) = siblings.next() {
                    if !path.is_empty() {
                        path.push('/');
                    }
                    path.push_str(&node.label);
                    let depth = stack.len() - 1;
                    let has_children = !node.children.is_empty();
                    if has_children && (!visible_only || node.expanded) {
                        stack.push((path.len(), node.children.iter()));
                    }
                    return Some(TreeRow {
                        depth,
                        path: path.clone(),
                        label: &node.label,
                        has_children,
                        expanded: node.expanded,
                    });
                }
                stack.pop();
            }
            None
        })
    }
}

fn valid_path(path: &str) -> bool {
    path.split('/').all(|label| !label.is_empty())
}

fn split_parent(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

fn join_path(parent: &str, label: &str) -> String {
    if parent.is_empty() {
        label.into()
    } else {
        format!("{parent}/{label}")
    }
}

fn in_subtree(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn child_index<T>(siblings: &[Node<T>], label: &str) -> Option<usize> {
    siblings.iter().position(|node| node.label == label)
}

const ACTIONS: [ActionSpec; 4] = [
    action("expand", "Expand a node by path.", &[param("path", "str")]),
    action(
        "collapse",
        "Collapse a node by path.",
        &[param("path", "str")],
    ),
    action("select", "Select a node by path.", &[param("path", "str")]),
    action(
        "move",
        "Move a subtree under a parent path.",
        &[
            param("path", "str"),
            param("parent", "str"),
            param_opt("at", "int"),
        ],
    ),
];

impl<T: Send + 'static> Behavior for Tree<T> {
    fn describe(&self, out: &mut Describe) {
        if let Some(selected) = self.selected() {
            out.attr("sel", selected);
        }
        out.attr("open", self.expanded_count);
    }

    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }

    fn owns(&self, action: &str) -> bool {
        ACTIONS.iter().any(|spec| spec.name == action)
    }

    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            let path: String = args.get(0, "path")?;
            match name {
                "expand" => self.expand(&path),
                "collapse" => self.collapse(&path),
                "select" => self.select(&path),
                _ => self.move_node(
                    &path,
                    &args.get::<String>(1, "parent")?,
                    args.get_opt(2, "at")?,
                ),
            }
        })
    }

    fn queryable(&self) -> bool {
        true
    }

    fn query(&self, q: &Query) -> Option<Page> {
        let items = self
            .rows(false)
            .filter(|row| q.text.as_deref().is_none_or(|text| row.path.contains(text)))
            .map(|row| Value::String(row.path));
        Some(Page::from_iter(items, q))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_reveals_rows_in_sibling_order() -> Result<(), ActionError> {
        let mut tree =
            Tree::<u8>::from_paths(["src/agent/format.rs", "README", "src/lib.rs", "src"]);
        assert_eq!(tree.get("src"), Some(&0));
        assert_eq!(tree.get("src/agent"), Some(&0));
        assert_eq!(tree.get("src/agent/format.rs"), Some(&0));
        assert_eq!(tree.visible_rows().len(), 2);

        tree.expand("src/agent")?;
        assert!(tree.is_expanded("src"));
        assert!(tree.is_expanded("src/agent"));
        let rows: Vec<_> = tree
            .visible_rows()
            .into_iter()
            .map(|row| {
                (
                    row.depth,
                    row.path,
                    row.label,
                    row.has_children,
                    row.expanded,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                (0, "src".into(), "src", true, true),
                (1, "src/agent".into(), "agent", true, true),
                (2, "src/agent/format.rs".into(), "format.rs", false, false),
                (1, "src/lib.rs".into(), "lib.rs", false, false),
                (0, "README".into(), "README", false, false),
            ]
        );

        tree.collapse("src")?;
        assert!(tree.is_expanded("src/agent"));
        assert_eq!(tree.visible_rows().len(), 2);
        tree.select("src/agent/format.rs")?;
        assert_eq!(tree.selected(), Some("src/agent/format.rs"));
        assert!(
            tree.visible_rows()
                .iter()
                .any(|row| row.path == "src/agent/format.rs")
        );
        assert!(!tree.is_expanded("src/agent/format.rs"));
        let mut description = Describe::new();
        tree.describe(&mut description);
        assert_eq!(description.get("open"), Some(&2.into()));
        Ok(())
    }

    #[test]
    fn moves_subtrees_and_tracks_selection() -> Result<(), ActionError> {
        let mut tree =
            Tree::<()>::from_paths(["src/agent/format.rs", "docs/guide", "README", "guide"]);
        tree.select("src/agent/format.rs")?;
        assert_eq!(
            tree.move_node("src", "src/agent", None),
            Err(ActionError::Rejected(
                "cannot move src into its own subtree".into()
            ))
        );
        assert!(tree.move_node("src/agent", "docs", Some(2)).is_err());
        assert!(tree.move_node("docs/guide", "", None).is_err());
        assert_eq!(tree.selected(), Some("src/agent/format.rs"));
        assert_eq!(tree.get("src/agent/format.rs"), Some(&()));

        tree.move_node("src/agent", "docs", Some(0))?;
        assert_eq!(tree.selected(), Some("docs/agent/format.rs"));
        assert_eq!(tree.get("src/agent/format.rs"), None);
        assert!(tree.is_expanded("docs/agent"));
        tree.expand("docs")?;
        assert_eq!(tree.visible_rows()[2].path, "docs/agent");
        tree.move_node("docs/agent", "docs", None)?;
        assert_eq!(tree.visible_rows()[2].path, "docs/guide");
        tree.move_node("docs/agent", "", Some(0))?;
        assert_eq!(tree.visible_rows()[0].path, "agent");
        assert_eq!(tree.selected(), Some("agent/format.rs"));

        assert_eq!(tree.remove("agent"), Some(()));
        assert_eq!(tree.selected(), None);
        let mut description = Describe::new();
        tree.describe(&mut description);
        assert_eq!(description.get("open"), Some(&2.into()));
        Ok(())
    }

    #[test]
    fn rejects_invalid_paths_and_reports_the_nearest_ancestor() -> Result<(), ActionError> {
        let mut tree = Tree::new();
        tree.insert("src", 1)?;
        tree.insert("src/agent", 2)?;
        tree.insert("src/lib.rs", 3)?;
        tree.insert("src", 4)?;
        assert_eq!(tree.get("src"), Some(&4));
        assert_eq!(tree.get("src/agent"), Some(&2));
        assert_eq!(
            tree.insert("src/missing/file", 5),
            Err(ActionError::Rejected(
                "no node at src/missing; children of src: agent, lib.rs".into()
            ))
        );
        assert!(matches!(
            tree.insert("src//invalid", 5),
            Err(ActionError::Rejected(reason)) if reason.contains("labels must be nonempty")
        ));
        let error = ActionError::Rejected(
            "no node at src/agents/format.rs; children of src: agent, lib.rs".into(),
        );
        assert_eq!(tree.expand("src/agents/format.rs"), Err(error.clone()));
        assert_eq!(tree.select("src/agents/format.rs"), Err(error.clone()));
        assert_eq!(tree.collapse("src/agents/format.rs"), Err(error.clone()));
        assert_eq!(
            tree.move_node("src", "src/agents/format.rs", None),
            Err(error)
        );
        assert_eq!(
            tree.select("missing"),
            Err(ActionError::Rejected(
                "no node at missing; children of <root>: src".into()
            ))
        );
        assert!(!tree.is_expanded("src"));
        assert_eq!(tree.selected(), None);
        Ok(())
    }

    #[test]
    fn queries_hidden_paths_with_pagination() {
        let tree = Tree::<()>::from_paths(["src/agent/format.rs", "src/lib.rs", "docs/format.md"]);
        for (text, from, item, total, next) in [
            ("src/", 0, "src/agent", 3, Some(1)),
            ("format", 1, "docs/format.md", 2, None),
        ] {
            assert_eq!(
                tree.query(&Query {
                    text: Some(text.into()),
                    from,
                    limit: 1
                }),
                Some(Page {
                    items: vec![item.into()],
                    total,
                    next
                }),
            );
        }
    }
}
