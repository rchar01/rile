// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::buffer::{BufferId, Position};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub buffer: BufferId,
    pub cursor: Position,
    pub first_visible_line: usize,
    pub first_visible_column: usize,
    pub text_rows: usize,
}

impl Viewport {
    pub const fn new(buffer: BufferId) -> Self {
        Self {
            buffer,
            cursor: Position::new(0, 0),
            first_visible_line: 0,
            first_visible_column: 0,
            text_rows: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    pub row: usize,
    pub column: usize,
    pub rows: usize,
    pub columns: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowLayout {
    pub id: WindowId,
    pub rect: WindowRect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    id: WindowId,
    viewport: Viewport,
}

impl Window {
    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    pub fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SplitNode {
    Leaf(WindowId),
    Split {
        axis: SplitAxis,
        children: Vec<SplitNode>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSet {
    windows: Vec<Window>,
    root: SplitNode,
    current: WindowId,
    next_id: usize,
}

impl WindowSet {
    pub fn new(buffer: BufferId) -> Self {
        let id = WindowId(0);
        Self {
            windows: vec![Window {
                id,
                viewport: Viewport::new(buffer),
            }],
            root: SplitNode::Leaf(id),
            current: id,
            next_id: 1,
        }
    }

    pub fn current_id(&self) -> WindowId {
        self.current
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn current(&self) -> &Window {
        self.window(self.current)
            .expect("current window must exist")
    }

    pub fn current_mut(&mut self) -> &mut Window {
        self.window_mut(self.current)
            .expect("current window must exist")
    }

    pub fn next_window_id(&self) -> WindowId {
        let Some(index) = self
            .windows
            .iter()
            .position(|window| window.id == self.current)
        else {
            return self.current;
        };
        let next = (index + 1) % self.windows.len();
        self.windows[next].id
    }

    pub fn window_showing_buffer(&self, buffer: BufferId) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|window| window.viewport.buffer == buffer)
            .map(Window::id)
    }

    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|window| window.id == id)
    }

    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.iter_mut().find(|window| window.id == id)
    }

    pub fn split_current(&mut self, axis: SplitAxis) -> WindowId {
        let id = WindowId(self.next_id);
        self.next_id += 1;
        let viewport = *self.current().viewport();
        self.windows.push(Window { id, viewport });
        replace_leaf(
            &mut self.root,
            self.current,
            SplitNode::Split {
                axis,
                children: vec![SplitNode::Leaf(self.current), SplitNode::Leaf(id)],
            },
        );
        id
    }

    pub fn delete_current(&mut self) {
        if self.windows.len() <= 1 {
            return;
        }

        let deleted = self.current;
        let index = self
            .windows
            .iter()
            .position(|window| window.id == deleted)
            .expect("current window must exist");
        self.windows.remove(index);
        remove_leaf(&mut self.root, deleted);
        collapse_singles(&mut self.root);
        let next_index = index.min(self.windows.len() - 1);
        self.current = self.windows[next_index].id;
    }

    pub fn delete_others(&mut self) {
        let current = self.current().clone();
        self.windows = vec![current];
        self.root = SplitNode::Leaf(self.current);
    }

    pub fn other_window(&mut self) {
        let Some(index) = self
            .windows
            .iter()
            .position(|window| window.id == self.current)
        else {
            return;
        };
        let next = (index + 1) % self.windows.len();
        self.current = self.windows[next].id;
    }

    pub fn replace_buffer(&mut self, old: BufferId, new: BufferId) {
        for window in &mut self.windows {
            if window.viewport.buffer == old {
                window.viewport = Viewport::new(new);
            }
        }
    }

    pub fn layouts(&self, rows: usize, columns: usize) -> Vec<WindowLayout> {
        let mut layouts = Vec::new();
        layout_node(
            &self.root,
            WindowRect {
                row: 0,
                column: 0,
                rows,
                columns,
            },
            &mut layouts,
        );
        layouts
    }
}

fn replace_leaf(node: &mut SplitNode, target: WindowId, replacement: SplitNode) -> bool {
    match node {
        SplitNode::Leaf(id) if *id == target => {
            *node = replacement;
            true
        }
        SplitNode::Leaf(_) => false,
        SplitNode::Split { children, .. } => {
            for child in children {
                if replace_leaf(child, target, replacement.clone()) {
                    return true;
                }
            }
            false
        }
    }
}

fn remove_leaf(node: &mut SplitNode, target: WindowId) -> bool {
    match node {
        SplitNode::Leaf(_) => false,
        SplitNode::Split { children, .. } => {
            children.retain(|child| !matches!(child, SplitNode::Leaf(id) if *id == target));
            for child in children {
                remove_leaf(child, target);
            }
            true
        }
    }
}

fn collapse_singles(node: &mut SplitNode) {
    if let SplitNode::Split { children, .. } = node {
        for child in children.iter_mut() {
            collapse_singles(child);
        }
        if children.len() == 1 {
            *node = children.remove(0);
        }
    }
}

fn layout_node(node: &SplitNode, rect: WindowRect, layouts: &mut Vec<WindowLayout>) {
    match node {
        SplitNode::Leaf(id) => layouts.push(WindowLayout { id: *id, rect }),
        SplitNode::Split { axis, children } => match axis {
            SplitAxis::Horizontal => {
                let parts = split_dimension(rect.rows, children.len());
                let mut row = rect.row;
                for (child, rows) in children.iter().zip(parts) {
                    layout_node(
                        child,
                        WindowRect {
                            row,
                            column: rect.column,
                            rows,
                            columns: rect.columns,
                        },
                        layouts,
                    );
                    row += rows;
                }
            }
            SplitAxis::Vertical => {
                let parts = split_dimension(rect.columns, children.len());
                let mut column = rect.column;
                for (child, columns) in children.iter().zip(parts) {
                    layout_node(
                        child,
                        WindowRect {
                            row: rect.row,
                            column,
                            rows: rect.rows,
                            columns,
                        },
                        layouts,
                    );
                    column += columns;
                }
            }
        },
    }
}

fn split_dimension(size: usize, count: usize) -> Vec<usize> {
    let base = size / count;
    let remainder = size % count;
    (0..count)
        .map(|index| base + usize::from(index < remainder))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{SplitAxis, WindowRect, WindowSet};
    use crate::buffer::{BufferId, Position};

    #[test]
    fn splits_current_window_horizontally_and_vertically() {
        let mut windows = WindowSet::new(BufferId(1));
        let original = windows.current_id();

        let second = windows.split_current(SplitAxis::Horizontal);
        assert_ne!(original, second);
        assert_eq!(windows.len(), 2);
        assert_eq!(windows.current_id(), original);

        let layouts = windows.layouts(10, 80);
        assert_eq!(
            layouts[0].rect,
            WindowRect {
                row: 0,
                column: 0,
                rows: 5,
                columns: 80
            }
        );
        assert_eq!(
            layouts[1].rect,
            WindowRect {
                row: 5,
                column: 0,
                rows: 5,
                columns: 80
            }
        );

        windows.split_current(SplitAxis::Vertical);
        let layouts = windows.layouts(10, 80);
        assert_eq!(layouts.len(), 3);
        assert_eq!(
            layouts[0].rect,
            WindowRect {
                row: 0,
                column: 0,
                rows: 5,
                columns: 40
            }
        );
        assert_eq!(
            layouts[1].rect,
            WindowRect {
                row: 0,
                column: 40,
                rows: 5,
                columns: 40
            }
        );
    }

    #[test]
    fn cycles_and_deletes_windows() {
        let mut windows = WindowSet::new(BufferId(1));
        let first = windows.current_id();
        let second = windows.split_current(SplitAxis::Horizontal);
        assert_eq!(windows.current_id(), first);

        windows.other_window();
        assert_eq!(windows.current_id(), second);

        windows.delete_current();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows.current_id(), first);
    }

    #[test]
    fn stores_per_window_viewports() {
        let mut windows = WindowSet::new(BufferId(1));
        let first = windows.current_id();
        let second = windows.split_current(SplitAxis::Vertical);
        assert_eq!(windows.current_id(), first);

        windows
            .window_mut(second)
            .expect("window should exist")
            .viewport_mut()
            .cursor = Position::new(3, 2);
        windows.other_window();

        assert_eq!(windows.current_id(), second);
        assert_eq!(windows.current().viewport().cursor, Position::new(3, 2));
    }
}
