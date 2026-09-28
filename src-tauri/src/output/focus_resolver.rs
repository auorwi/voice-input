//! Resolve the actual keyboard focus when a toolkit initially exposes only a window.
use super::focused_input::FocusKind;

pub(super) struct NodeState {
    pub kind: FocusKind,
    pub focused: bool,
    pub container: bool,
}

pub(super) trait FocusTree {
    type Node;
    fn app_focus(&mut self) -> Option<Self::Node>;
    fn window(&mut self) -> Option<Self::Node>;
    fn id(&self, node: &Self::Node) -> u64;
    fn state(&mut self, node: &Self::Node) -> NodeState;
    fn children(&mut self, node: &Self::Node, limit: usize) -> Option<Vec<Self::Node>>;
    fn expired(&self) -> bool;
    fn retry(&mut self);
}

pub(super) fn resolve<T: FocusTree>(tree: &mut T) -> Option<T::Node> {
    if let Some(direct) = tree.app_focus() {
        // A concrete control (including a button or secure field) is authoritative.
        if !tree.state(&direct).container && !tree.expired() {
            return Some(direct);
        }
    }
    let window = tree.window()?;
    let window_id = tree.id(&window);
    for attempt in 0..2 {
        let window = tree.window()?;
        if tree.id(&window) != window_id || tree.expired() {
            return None;
        }
        let mut stack = vec![(window, 0)];
        let mut visited = std::collections::HashSet::new();
        let mut candidates = Vec::new();
        let mut blocked = false;
        let mut incomplete = false;
        while let Some((node, depth)) = stack.pop() {
            if tree.expired() || visited.len() >= 96 {
                return None;
            }
            if !visited.insert(tree.id(&node)) {
                continue;
            }
            let state = tree.state(&node);
            if state.focused && !state.container {
                if state.kind == FocusKind::Editable {
                    candidates.push(node);
                } else {
                    blocked = true;
                }
            } else if state.container && depth < 12 {
                // Reading children allows lazy toolkits to expose their controls. Inspect only
                // structural containers, never message lists or document content.
                let remaining = 96 - visited.len() - stack.len().min(96 - visited.len());
                let Some(children) = tree.children(&node, remaining) else {
                    incomplete = true;
                    continue;
                };
                incomplete |= remaining == 0 || children.len() == remaining;
                for child in children.into_iter().rev() {
                    stack.push((child, depth + 1));
                }
            } else if state.container {
                incomplete = true;
            }
        }
        let current_window = tree.window()?;
        if tree.id(&current_window) != window_id || tree.expired() {
            return None;
        }
        if let Some(direct) = tree.app_focus() {
            if !tree.state(&direct).container && !tree.expired() {
                return Some(direct);
            }
        }
        if !blocked && !incomplete && candidates.len() == 1 {
            let node = candidates.pop()?;
            let state = tree.state(&node);
            if state.focused && state.kind == FocusKind::Editable && !tree.expired() {
                return Some(node);
            }
        }
        if blocked || candidates.len() > 1 {
            return None;
        }
        if attempt == 0 {
            tree.retry();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeTree {
        nodes: HashMap<u64, (NodeState, Vec<u64>)>,
        direct: Option<u64>,
        window: Option<u64>,
        direct_after_read: Option<u64>,
        window_after_read: Option<u64>,
        failed_children: Option<u64>,
        focus_after_retry: Option<u64>,
        reads: usize,
    }

    impl FakeTree {
        fn new() -> Self {
            let mut tree = Self {
                nodes: HashMap::new(),
                direct: Some(1),
                window: Some(1),
                direct_after_read: None,
                window_after_read: None,
                failed_children: None,
                focus_after_retry: None,
                reads: 0,
            };
            tree.add(1, FocusKind::NotEditable, false, true, vec![2]);
            tree.add(2, FocusKind::NotEditable, false, true, vec![3, 4]);
            tree.add(3, FocusKind::Editable, false, false, vec![]); // search field
            tree.add(4, FocusKind::Editable, true, false, vec![]); // composer
            tree
        }
        fn add(
            &mut self,
            id: u64,
            kind: FocusKind,
            focused: bool,
            container: bool,
            children: Vec<u64>,
        ) {
            self.nodes.insert(
                id,
                (
                    NodeState {
                        kind,
                        focused,
                        container,
                    },
                    children,
                ),
            );
        }
    }
    impl FocusTree for FakeTree {
        type Node = u64;
        fn app_focus(&mut self) -> Option<u64> {
            self.direct
        }
        fn window(&mut self) -> Option<u64> {
            self.window
        }
        fn id(&self, node: &u64) -> u64 {
            *node
        }
        fn state(&mut self, node: &u64) -> NodeState {
            let s = &self.nodes[node].0;
            NodeState {
                kind: s.kind,
                focused: s.focused,
                container: s.container,
            }
        }
        fn children(&mut self, node: &u64, limit: usize) -> Option<Vec<u64>> {
            self.reads += 1;
            if self.failed_children == Some(*node) {
                return None;
            }
            if let Some(next) = self.direct_after_read.take() {
                self.direct = Some(next);
            }
            if let Some(next) = self.window_after_read.take() {
                self.window = Some(next);
            }
            Some(self.nodes[node].1.iter().take(limit).copied().collect())
        }
        fn expired(&self) -> bool {
            self.reads >= 128
        }
        fn retry(&mut self) {
            if let Some(next) = self.focus_after_retry.take() {
                self.direct = Some(next);
            }
        }
    }

    #[test]
    fn window_focus_resolves_to_focused_composer_not_first_editable_search_box() {
        let mut tree = FakeTree::new();
        assert_eq!(resolve(&mut tree), Some(4));
    }

    #[test]
    fn reads_window_children_to_activate_lazy_toolkit_then_rechecks_direct_focus() {
        let mut tree = FakeTree::new();
        tree.nodes.get_mut(&1).unwrap().1.clear();
        tree.direct_after_read = Some(4);
        assert_eq!(resolve(&mut tree), Some(4));
    }

    #[test]
    fn does_not_guess_when_no_editable_descendant_has_keyboard_focus() {
        let mut tree = FakeTree::new();
        tree.nodes.get_mut(&4).unwrap().0.focused = false;
        assert_eq!(resolve(&mut tree), None);
    }

    #[test]
    fn explicit_button_focus_does_not_redirect_to_a_text_box() {
        let mut tree = FakeTree::new();
        tree.add(5, FocusKind::NotEditable, true, false, vec![]);
        tree.direct = Some(5);
        assert_eq!(resolve(&mut tree), Some(5));
        assert_eq!(tree.reads, 0);
    }

    #[test]
    fn changed_window_during_resolution_aborts() {
        let mut tree = FakeTree::new();
        tree.window_after_read = Some(99);
        assert_eq!(resolve(&mut tree), None);
    }

    #[test]
    fn two_claimed_focused_editors_are_ambiguous() {
        let mut tree = FakeTree::new();
        tree.nodes.get_mut(&3).unwrap().0.focused = true;
        assert_eq!(resolve(&mut tree), None);
    }

    #[test]
    fn focused_secure_or_read_only_field_is_never_replaced_with_an_editable_sibling() {
        let mut tree = FakeTree::new();
        tree.nodes.get_mut(&4).unwrap().0.kind = FocusKind::NotEditable;
        assert_eq!(resolve(&mut tree), None);
    }

    #[test]
    fn traversal_stops_on_cycles_and_does_not_expand_message_lists() {
        let mut tree = FakeTree::new();
        tree.nodes.get_mut(&2).unwrap().1 = vec![1, 5];
        tree.add(5, FocusKind::NotEditable, false, false, vec![4]);
        assert_eq!(resolve(&mut tree), None);
        assert!(tree.reads < 10);
    }

    #[test]
    fn explicit_focus_change_takes_precedence_over_previous_descendant() {
        let mut tree = FakeTree::new();
        tree.add(5, FocusKind::NotEditable, true, false, vec![]);
        tree.direct_after_read = Some(5);
        assert_eq!(resolve(&mut tree), Some(5));
    }

    #[test]
    fn an_incomplete_tree_cannot_authorize_an_earlier_candidate() {
        let mut tree = FakeTree::new();
        tree.nodes.get_mut(&2).unwrap().1 = (4..150).collect();
        for id in 5..150 {
            tree.add(id, FocusKind::Editable, false, false, vec![]);
        }
        assert_eq!(resolve(&mut tree), None);
    }

    #[test]
    fn delayed_second_read_can_discover_the_keyboard_focus() {
        let mut tree = FakeTree::new();
        tree.direct = None;
        tree.nodes.get_mut(&1).unwrap().1.clear();
        tree.focus_after_retry = Some(4);
        assert_eq!(resolve(&mut tree), Some(4));
    }

    #[test]
    fn failed_sibling_read_cannot_make_a_candidate_look_unique() {
        let mut tree = FakeTree::new();
        tree.nodes.get_mut(&2).unwrap().1.push(5);
        tree.add(5, FocusKind::NotEditable, false, true, vec![]);
        tree.failed_children = Some(5);
        assert_eq!(resolve(&mut tree), None);
        tree.direct_after_read = Some(4);
        assert_eq!(resolve(&mut tree), Some(4));
    }

    #[test]
    fn unavailable_window_or_expired_deadline_cannot_guess_an_editor() {
        let mut tree = FakeTree::new();
        tree.window = None;
        assert_eq!(resolve(&mut tree), None);
        tree.window = Some(1);
        tree.reads = 128;
        assert_eq!(resolve(&mut tree), None);
    }
}
