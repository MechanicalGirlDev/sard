//! Dock layout: a tree of splits whose leaves are tab groups. Tabs can be dragged onto
//! another group (move), onto a group's edge quarter (split there), or dropped by the
//! host onto nothing (a tear-out, reported through [`DockResponse::torn_out`]).
//!
//! The tree algebra is pure and unit-tested; [`Ui::dock`] renders it.

use glam::Vec2;

use crate::window::event::Cursor;

use super::builder::Ui;
use super::frame::LAYER_POPUP;
use super::types::{Dir, Id, Rect, Sense};

/// A node of the dock tree.
#[derive(Debug, Clone, PartialEq)]
pub enum DockNode {
    /// A tab group.
    Tabs {
        /// Stable id (assigned by the tree).
        id: u32,
        /// Panel ids, in tab order.
        panels: Vec<String>,
        /// Index of the shown tab.
        active: usize,
    },
    /// A split.
    Split {
        /// Direction the children are laid along.
        dir: Dir,
        /// Relative sizes (flex weights), one per child.
        sizes: Vec<f32>,
        /// Children.
        children: Vec<Self>,
    },
}

impl DockNode {
    /// A tab group (ids are assigned when inserted into a tree).
    pub fn tabs(panels: Vec<String>) -> Self {
        Self::Tabs {
            id: 0,
            panels,
            active: 0,
        }
    }

    /// A split with equal sizes.
    pub fn split(dir: Dir, children: Vec<Self>) -> Self {
        let n = children.len().max(1);
        Self::Split {
            dir,
            sizes: vec![1.0; n],
            children,
        }
    }

    /// A split with explicit weights.
    pub fn split_sized(dir: Dir, sizes: Vec<f32>, children: Vec<Self>) -> Self {
        Self::Split {
            dir,
            sizes,
            children,
        }
    }

    fn assign_ids(&mut self, next: &mut u32) {
        match self {
            Self::Tabs { id, .. } => {
                if *id == 0 {
                    *id = *next;
                    *next += 1;
                }
            }
            Self::Split { children, .. } => {
                for c in children {
                    c.assign_ids(next);
                }
            }
        }
    }

    fn max_id(&self) -> u32 {
        match self {
            Self::Tabs { id, .. } => *id,
            Self::Split { children, .. } => children.iter().map(Self::max_id).max().unwrap_or(0),
        }
    }

    /// All panel ids in tree order.
    pub fn panels(&self) -> Vec<String> {
        match self {
            Self::Tabs { panels, .. } => panels.clone(),
            Self::Split { children, .. } => children.iter().flat_map(Self::panels).collect(),
        }
    }

    fn find_tabs_mut(&mut self, tabs_id: u32) -> Option<&mut Self> {
        match self {
            Self::Tabs { id, .. } if *id == tabs_id => Some(self),
            Self::Tabs { .. } => None,
            Self::Split { children, .. } => {
                children.iter_mut().find_map(|c| c.find_tabs_mut(tabs_id))
            }
        }
    }

    fn tabs_of_panel(&self, panel: &str) -> Option<u32> {
        match self {
            Self::Tabs { id, panels, .. } => panels.iter().any(|p| p == panel).then_some(*id),
            Self::Split { children, .. } => children.iter().find_map(|c| c.tabs_of_panel(panel)),
        }
    }

    fn first_tabs(&self) -> Option<u32> {
        match self {
            Self::Tabs { id, .. } => Some(*id),
            Self::Split { children, .. } => children.iter().find_map(Self::first_tabs),
        }
    }

    fn remove_panel(&mut self, panel: &str) -> bool {
        match self {
            Self::Tabs { panels, active, .. } => {
                let before = panels.len();
                panels.retain(|p| p != panel);
                if panels.len() == before {
                    false
                } else {
                    *active = (*active).min(panels.len().saturating_sub(1));
                    true
                }
            }
            Self::Split { children, .. } => children.iter_mut().any(|c| c.remove_panel(panel)),
        }
    }

    /// Remove empty groups, unwrap single-child splits, splice same-direction splits.
    fn prune(self) -> Option<Self> {
        match self {
            Self::Tabs { panels, .. } if panels.is_empty() => None,
            t @ Self::Tabs { .. } => Some(t),
            Self::Split {
                dir,
                sizes,
                children,
            } => {
                let mut out_children = Vec::new();
                let mut out_sizes = Vec::new();
                for (c, s) in children
                    .into_iter()
                    .zip(sizes.into_iter().chain(core::iter::repeat(1.0)))
                {
                    let Some(c) = c.prune() else { continue };
                    match c {
                        Self::Split {
                            dir: d2,
                            sizes: s2,
                            children: c2,
                        } if d2 == dir => {
                            // Splice: distribute this child's share over its children.
                            let total = s2.iter().sum::<f32>().max(1e-6);
                            for (cc, ss) in c2.into_iter().zip(s2) {
                                out_children.push(cc);
                                out_sizes.push(s * ss / total);
                            }
                        }
                        other => {
                            out_children.push(other);
                            out_sizes.push(s);
                        }
                    }
                }
                match out_children.len() {
                    0 => None,
                    1 => out_children.pop(),
                    _ => Some(Self::Split {
                        dir,
                        sizes: out_sizes,
                        children: out_children,
                    }),
                }
            }
        }
    }

    /// Split the group `tabs_id` so that `new` sits on `side` of it. When the parent is
    /// already a split of that direction, `new` becomes a sibling instead of nesting.
    fn split_at(&mut self, tabs_id: u32, side: Side, new: Self) -> bool {
        // Try to splice into a same-direction parent first.
        if let Self::Split {
            dir,
            sizes,
            children,
        } = self
        {
            let want = side.dir();
            if *dir == want {
                if let Some(i) = children
                    .iter()
                    .position(|c| matches!(c, Self::Tabs { id, .. } if *id == tabs_id))
                {
                    let share = sizes.get(i).copied().unwrap_or(1.0);
                    let at = if side.is_before() { i } else { i + 1 };
                    children.insert(at, new);
                    sizes.insert(at, share * 0.5);
                    if let Some(s) = sizes.get_mut(if side.is_before() { i + 1 } else { i }) {
                        *s = share * 0.5;
                    }
                    return true;
                }
            }
            for c in children.iter_mut() {
                if c.split_at(tabs_id, side, new.clone()) {
                    return true;
                }
            }
            return false;
        }
        // A tab group at the root (or a group child of a cross-direction split): wrap it.
        if matches!(self, Self::Tabs { id, .. } if *id == tabs_id) {
            let old = core::mem::replace(self, Self::tabs(Vec::new()));
            let (children, sizes) = if side.is_before() {
                (vec![new, old], vec![0.35, 0.65])
            } else {
                (vec![old, new], vec![0.65, 0.35])
            };
            *self = Self::Split {
                dir: side.dir(),
                sizes,
                children,
            };
            return true;
        }
        false
    }
}

/// Which edge of a tab group a panel is dropped on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Left.
    Left,
    /// Right.
    Right,
    /// Top.
    Top,
    /// Bottom.
    Bottom,
}

impl Side {
    fn dir(self) -> Dir {
        match self {
            Self::Left | Self::Right => Dir::Horizontal,
            Self::Top | Self::Bottom => Dir::Vertical,
        }
    }

    fn is_before(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

/// Where a dragged panel is dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropTarget {
    /// Into the tab group as its last tab.
    Tabs(u32),
    /// Onto an edge of the tab group (split there).
    Edge(u32, Side),
}

/// The dock tree.
#[derive(Debug, Clone, PartialEq)]
pub struct DockTree {
    root: Option<DockNode>,
    next_id: u32,
}

impl Default for DockTree {
    fn default() -> Self {
        Self::new(DockNode::tabs(Vec::new()))
    }
}

impl DockTree {
    /// A tree from a root node (group ids are assigned).
    pub fn new(mut root: DockNode) -> Self {
        let mut next = root.max_id() + 1;
        root.assign_ids(&mut next);
        Self {
            root: root.prune(),
            next_id: next,
        }
    }

    /// The root node.
    pub fn root(&self) -> Option<&DockNode> {
        self.root.as_ref()
    }

    /// All panel ids in tree order.
    pub fn panels(&self) -> Vec<String> {
        self.root.as_ref().map_or_else(Vec::new, DockNode::panels)
    }

    /// Whether `panel` is in the tree.
    pub fn contains(&self, panel: &str) -> bool {
        self.tabs_of(panel).is_some()
    }

    /// The group holding `panel`.
    pub fn tabs_of(&self, panel: &str) -> Option<u32> {
        self.root.as_ref().and_then(|r| r.tabs_of_panel(panel))
    }

    fn fresh_tabs(&mut self, panels: Vec<String>) -> DockNode {
        let id = self.next_id;
        self.next_id += 1;
        DockNode::Tabs {
            id,
            panels,
            active: 0,
        }
    }

    /// Remove `panel` (empty groups / degenerate splits collapse).
    pub fn close_panel(&mut self, panel: &str) {
        if let Some(r) = self.root.as_mut() {
            let _ = r.remove_panel(panel);
        }
        self.root = self.root.take().and_then(DockNode::prune);
    }

    /// Show `panel`: activate it if present, otherwise add it to `near` (a group id) or
    /// the first group, or as the root when the tree is empty.
    pub fn open_panel(&mut self, panel: &str, near: Option<u32>) {
        if self.contains(panel) {
            self.activate(panel);
            return;
        }
        let target = near
            .filter(|id| {
                self.root
                    .as_mut()
                    .is_some_and(|r| r.find_tabs_mut(*id).is_some())
            })
            .or_else(|| self.root.as_ref().and_then(DockNode::first_tabs));
        if let Some(DockNode::Tabs { panels, active, .. }) =
            target.and_then(|id| self.root.as_mut().and_then(|r| r.find_tabs_mut(id)))
        {
            panels.push(panel.to_owned());
            *active = panels.len() - 1;
        } else {
            let node = self.fresh_tabs(vec![panel.to_owned()]);
            self.root = Some(node);
        }
    }

    /// Make `panel` the active tab of its group.
    pub fn activate(&mut self, panel: &str) {
        let Some(id) = self.tabs_of(panel) else {
            return;
        };
        if let Some(DockNode::Tabs { panels, active, .. }) =
            self.root.as_mut().and_then(|r| r.find_tabs_mut(id))
        {
            if let Some(i) = panels.iter().position(|p| p == panel) {
                *active = i;
            }
        }
    }

    /// The active panel of group `tabs_id`.
    pub fn active_of(&self, tabs_id: u32) -> Option<String> {
        fn walk(n: &DockNode, tabs_id: u32) -> Option<String> {
            match n {
                DockNode::Tabs {
                    id, panels, active, ..
                } if *id == tabs_id => panels.get(*active).cloned(),
                DockNode::Tabs { .. } => None,
                DockNode::Split { children, .. } => children.iter().find_map(|c| walk(c, tabs_id)),
            }
        }
        self.root.as_ref().and_then(|r| walk(r, tabs_id))
    }

    /// Move `panel` to `target`. Dropping a panel onto its own group is a no-op; splitting
    /// a group that holds only this panel is a no-op too.
    pub fn move_panel(&mut self, panel: &str, target: DropTarget) {
        let source = self.tabs_of(panel);
        let target_id = match target {
            DropTarget::Tabs(id) | DropTarget::Edge(id, _) => id,
        };
        if source == Some(target_id) {
            let alone = self.root.as_ref().is_some_and(|r| {
                r.tabs_of_panel(panel).is_some() && self.group_len(target_id) == 1
            });
            if matches!(target, DropTarget::Tabs(_)) || alone {
                return;
            }
        }
        if let Some(r) = self.root.as_mut() {
            let _ = r.remove_panel(panel);
        }
        match target {
            DropTarget::Tabs(id) => match self.root.as_mut().and_then(|r| r.find_tabs_mut(id)) {
                Some(DockNode::Tabs { panels, active, .. }) => {
                    panels.push(panel.to_owned());
                    *active = panels.len() - 1;
                }
                _ => self.open_panel(panel, None),
            },
            DropTarget::Edge(id, side) => {
                let new = self.fresh_tabs(vec![panel.to_owned()]);
                let placed = self
                    .root
                    .as_mut()
                    .is_some_and(|r| r.split_at(id, side, new));
                if !placed {
                    self.open_panel(panel, None);
                }
            }
        }
        self.root = self.root.take().and_then(DockNode::prune);
    }

    fn group_len(&self, tabs_id: u32) -> usize {
        fn walk(n: &DockNode, tabs_id: u32) -> Option<usize> {
            match n {
                DockNode::Tabs { id, panels, .. } if *id == tabs_id => Some(panels.len()),
                DockNode::Tabs { .. } => None,
                DockNode::Split { children, .. } => children.iter().find_map(|c| walk(c, tabs_id)),
            }
        }
        self.root
            .as_ref()
            .and_then(|r| walk(r, tabs_id))
            .unwrap_or(0)
    }

    /// Change the share between children `i` and `i + 1` of the split at `path`
    /// (indices from the root) by `delta` (fraction of the split's total).
    pub fn resize(&mut self, path: &[usize], i: usize, delta: f32) {
        let mut node = self.root.as_mut();
        for &p in path {
            node = match node {
                Some(DockNode::Split { children, .. }) => children.get_mut(p),
                _ => None,
            };
        }
        let Some(DockNode::Split { sizes, .. }) = node else {
            return;
        };
        if i + 1 < sizes.len() {
            let total: f32 = sizes.iter().sum();
            let pair = sizes[i] + sizes[i + 1];
            let min = (total * 0.05).min(pair * 0.5);
            let a = (sizes[i] + delta * total).clamp(min, pair - min);
            sizes[i] = a;
            sizes[i + 1] = pair - a;
        }
    }
}

/// What the host supplies to render a dock: titles, closability and panel bodies.
pub trait DockPanels {
    /// Tab title of `panel`.
    fn title(&self, panel: &str) -> String;

    /// Whether the tab can be closed with a middle click.
    fn closable(&self, _panel: &str) -> bool {
        true
    }

    /// Whether the panel body is left unpainted (a 3D viewport rendered underneath the
    /// GUI shows through).
    fn transparent(&self, _panel: &str) -> bool {
        false
    }

    /// Draw the body of `panel`.
    fn show(&mut self, panel: &str, ui: &mut Ui<'_>);
}

/// What happened to the dock this frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DockResponse {
    /// A tab was dropped outside every group (the host may open a new window for it).
    pub torn_out: Option<(String, Vec2)>,
    /// A panel was closed by its tab.
    pub closed: Vec<String>,
    /// Every tab drawn this frame and its rect (for hosts that need to point at one).
    pub tabs: Vec<(String, Rect)>,
}

/// Placement of one group, computed during layout.
struct Placed {
    id: u32,
    rect: Rect,
    strip: Rect,
}

impl Ui<'_> {
    /// Render `tree` over the remaining region.
    ///
    /// Each group is a tab strip (window ground) over a panel body; the 1 px gaps between
    /// groups are painted as hard rules (the ground itself is not painted, so a
    /// transparent 3D panel shows through). Splitters are those gaps with a wider
    /// invisible grab area; they turn accent on hover.
    pub fn dock(&mut self, tree: &mut DockTree, panels: &mut dyn DockPanels) -> DockResponse {
        let style = self.style().clone();
        let p = style.palette;
        let area = self.available();
        let _ = self.allocate(area.size());
        let root_id = self.make_id("dock");
        let mut response = DockResponse::default();
        let Some(root) = tree.root.clone() else {
            return response;
        };

        // Pass 1: layout (also handles splitter drags on the tree).
        let mut placed: Vec<Placed> = Vec::new();
        self.layout_node(tree, &root, &mut Vec::new(), area, root_id, &mut placed);

        // Pass 2: draw groups (strips + bodies), track tab drags.
        let drag_id = root_id.with("drag");
        let dragging: Option<String> = self.gui().string(drag_id).cloned();
        let mut pressed_tab: Option<(String, u32)> = None;
        let mut drop_target: Option<DropTarget> = None;
        let mouse = self.gui().mouse();
        let tab_font = style.font_size_small;
        for pl in &placed {
            let Some(DockNode::Tabs {
                panels: names,
                active,
                ..
            }) = tree.root.as_mut().and_then(|r| r.find_tabs_mut(pl.id))
            else {
                continue;
            };
            let names = names.clone();
            let active_i = (*active).min(names.len().saturating_sub(1));
            // Strip (clipped: an overflowing tab row must not spill into the neighbour).
            self.gui().paint_rect(pl.strip, p.background);
            self.gui().paint_rect(
                Rect::new(pl.strip.min.x, pl.strip.max.y - 1.0, pl.strip.width(), 1.0),
                p.line_hard,
            );
            self.gui().push_clip(pl.strip);
            let strip_id = root_id.with(("strip", pl.id));
            let mut x = pl.strip.min.x;
            let mut new_active = active_i;
            for (i, name) in names.iter().enumerate() {
                let title = panels.title(name);
                let m = self.gui().measure(&title, tab_font);
                let w = (m.x + 20.0).min(160.0);
                let tab = Rect::new(x, pl.strip.min.y, w, pl.strip.height() - 1.0);
                let r = self.interact(strip_id.with(name), tab, Sense::DRAG);
                response.tabs.push((name.clone(), tab));
                let is_active = i == active_i;
                let (bg, fg) = if is_active {
                    (p.panel, p.text)
                } else if r.hovered {
                    (p.row_hover, p.text_dim)
                } else {
                    (p.background, p.text_faint)
                };
                self.gui().paint_rect(tab, bg);
                if is_active {
                    self.gui().paint_rect(
                        Rect::new(tab.min.x, tab.max.y - 2.0, tab.width(), 2.0),
                        p.accent,
                    );
                }
                // Right rule.
                self.gui().paint_rect(
                    Rect::new(tab.max.x, tab.min.y, 1.0, tab.height()),
                    p.line_hard,
                );
                self.gui().push_clip(tab.shrink2(Vec2::new(4.0, 0.0)));
                self.gui().paint_text(
                    &title,
                    Vec2::new(tab.min.x + 10.0, tab.center().y - m.y * 0.5),
                    tab_font,
                    fg,
                );
                self.gui().pop_clip();
                if r.hovered {
                    self.gui().set_cursor(Cursor::Pointer);
                }
                if r.pressed {
                    new_active = i;
                    pressed_tab = Some((name.clone(), pl.id));
                }
                if r.hovered
                    && self.gui().mouse_released(crate::MouseButton::Middle)
                    && panels.closable(name)
                {
                    response.closed.push(name.clone());
                }
                x += w + 1.0;
            }
            self.gui().pop_clip();
            if let Some(DockNode::Tabs { active, .. }) =
                tree.root.as_mut().and_then(|r| r.find_tabs_mut(pl.id))
            {
                *active = new_active;
            }
            // Drop target under the cursor while dragging.
            if dragging.is_some() && pl.rect.contains(mouse) {
                drop_target = Some(if pl.strip.contains(mouse) {
                    DropTarget::Tabs(pl.id)
                } else {
                    let body = pl.rect;
                    let rel = (mouse - body.min) / body.size().max(Vec2::ONE);
                    let q = 0.25;
                    let edges = [
                        (rel.x, Side::Left),
                        (1.0 - rel.x, Side::Right),
                        (rel.y, Side::Top),
                        (1.0 - rel.y, Side::Bottom),
                    ];
                    let (nearest, side) =
                        edges
                            .into_iter()
                            .fold(edges[0], |a, b| if b.0 < a.0 { b } else { a });
                    if nearest >= q {
                        DropTarget::Tabs(pl.id)
                    } else {
                        DropTarget::Edge(pl.id, side)
                    }
                });
            }
            // Body.
            let body = Rect::from_min_max(Vec2::new(pl.rect.min.x, pl.strip.max.y), pl.rect.max);
            if !names.get(new_active).is_some_and(|n| panels.transparent(n)) {
                self.gui().paint_rect(body, p.panel);
            }
            if let Some(name) = names.get(new_active) {
                let inner = body.shrink2(Vec2::new(style.padding, style.padding * 0.5));
                let mut c = self.child(root_id.with(("body", name.as_str())), inner, Dir::Vertical);
                c.gui().push_clip(body);
                panels.show(name, &mut c);
                c.gui().pop_clip();
            }
        }

        // Drag bookkeeping.
        if let Some((name, _)) = pressed_tab {
            self.gui().set_string(drag_id, Some(name));
            self.gui().set_vec2(drag_id, Some(mouse));
        }
        if let Some(name) = dragging {
            let moved = self
                .gui()
                .vec2(drag_id)
                .is_some_and(|start| (mouse - start).length() > 5.0);
            let held = self.gui().mouse_down(crate::MouseButton::Left);
            if held && moved {
                // Drop highlight (accent tint + accent outline over the affected half)
                // and a ghost tab.
                let prev = self.gui().layer();
                self.gui().set_layer(LAYER_POPUP);
                if let Some(t) = drop_target {
                    let hl = placed
                        .iter()
                        .find(|pl| match t {
                            DropTarget::Tabs(id) | DropTarget::Edge(id, _) => pl.id == id,
                        })
                        .map(|pl| match t {
                            DropTarget::Tabs(_) => pl.rect,
                            DropTarget::Edge(_, Side::Left) => {
                                pl.rect.split_left(pl.rect.width() * 0.5).0
                            }
                            DropTarget::Edge(_, Side::Right) => {
                                pl.rect.split_right(pl.rect.width() * 0.5).1
                            }
                            DropTarget::Edge(_, Side::Top) => {
                                pl.rect.split_top(pl.rect.height() * 0.5).0
                            }
                            DropTarget::Edge(_, Side::Bottom) => {
                                pl.rect.split_bottom(pl.rect.height() * 0.5).1
                            }
                        });
                    if let Some(hl) = hl {
                        self.gui().paint_rect(hl, p.accent_bg);
                        self.gui().paint_rect_outline(hl, 1.0, p.accent);
                    }
                }
                let title = panels.title(&name);
                let m = self.gui().measure(&title, tab_font);
                let ghost = Rect::from_min_size(
                    mouse + Vec2::new(8.0, 8.0),
                    Vec2::new(m.x + 20.0, style.tab_height),
                );
                self.gui().paint_rect(ghost, p.popup);
                self.gui().paint_rect_outline(ghost, 1.0, p.accent);
                self.gui().paint_text(
                    &title,
                    Vec2::new(ghost.min.x + 10.0, ghost.center().y - m.y * 0.5),
                    tab_font,
                    p.text,
                );
                self.gui().set_layer(prev);
                self.gui().set_cursor(Cursor::Grabbing);
            } else if !held {
                if moved {
                    match drop_target {
                        Some(t) => tree.move_panel(&name, t),
                        None => response.torn_out = Some((name.clone(), mouse)),
                    }
                }
                self.gui().set_string(drag_id, None);
                self.gui().set_vec2(drag_id, None);
            }
        }
        for name in &response.closed {
            tree.close_panel(name);
        }
        response
    }

    /// Assign rects to groups, draw splitters, and apply splitter drags.
    fn layout_node(
        &mut self,
        tree: &mut DockTree,
        node: &DockNode,
        path: &mut Vec<usize>,
        rect: Rect,
        root_id: Id,
        out: &mut Vec<Placed>,
    ) {
        let style = self.style().clone();
        match node {
            DockNode::Tabs { id, .. } => {
                let (strip, _) = rect.split_top(style.tab_height);
                out.push(Placed {
                    id: *id,
                    rect,
                    strip,
                });
            }
            DockNode::Split {
                dir,
                sizes,
                children,
            } => {
                let n = children.len();
                // The visible gap is 1 px (the ground shows through); the grab area is
                // `style.splitter` wide, centred on it.
                let gap = 1.0;
                let grab = ((style.splitter - gap) * 0.5).max(1.0);
                let total = sizes.iter().sum::<f32>().max(1e-6);
                let extent = match dir {
                    Dir::Horizontal => rect.width(),
                    Dir::Vertical => rect.height(),
                } - gap * (n as f32 - 1.0).max(0.0);
                let mut offset = 0.0;
                for (i, child) in children.iter().enumerate() {
                    let share = sizes.get(i).copied().unwrap_or(1.0) / total;
                    let len = (extent * share).round();
                    let child_rect = match dir {
                        Dir::Horizontal => {
                            Rect::new(rect.min.x + offset, rect.min.y, len, rect.height())
                        }
                        Dir::Vertical => {
                            Rect::new(rect.min.x, rect.min.y + offset, rect.width(), len)
                        }
                    };
                    path.push(i);
                    self.layout_node(tree, child, path, child_rect, root_id, out);
                    let _ = path.pop();
                    offset += len;
                    if i + 1 < n {
                        let (handle, hit) = match dir {
                            Dir::Horizontal => (
                                Rect::new(rect.min.x + offset, rect.min.y, gap, rect.height()),
                                Rect::new(
                                    rect.min.x + offset - grab,
                                    rect.min.y,
                                    gap + grab * 2.0,
                                    rect.height(),
                                ),
                            ),
                            Dir::Vertical => (
                                Rect::new(rect.min.x, rect.min.y + offset, rect.width(), gap),
                                Rect::new(
                                    rect.min.x,
                                    rect.min.y + offset - grab,
                                    rect.width(),
                                    gap + grab * 2.0,
                                ),
                            ),
                        };
                        let hid = root_id.with(("splitter", path.clone(), i));
                        let r = self.interact(hid, hit, Sense::DRAG);
                        // The gap is the only place the ground shows: paint it as a rule
                        // (a transparent 3D panel underneath must not be covered).
                        let c = if r.dragged || r.hovered {
                            self.gui().set_cursor(match dir {
                                Dir::Horizontal => Cursor::EwResize,
                                Dir::Vertical => Cursor::NsResize,
                            });
                            style.palette.accent
                        } else {
                            style.palette.line_hard
                        };
                        self.gui().paint_rect(handle, c);
                        if r.dragged && extent > 0.0 {
                            let d = match dir {
                                Dir::Horizontal => r.drag_delta.x,
                                Dir::Vertical => r.drag_delta.y,
                            } / extent;
                            tree.resize(path, i, d);
                        }
                        offset += gap;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> DockTree {
        DockTree::new(DockNode::split(
            Dir::Horizontal,
            vec![
                DockNode::tabs(vec!["a".into(), "b".into()]),
                DockNode::tabs(vec!["view".into()]),
                DockNode::tabs(vec!["c".into()]),
            ],
        ))
    }

    #[test]
    fn move_between_groups_and_prune() {
        let mut t = tree();
        let c = t.tabs_of("c").unwrap();
        let a = t.tabs_of("a").unwrap();
        t.move_panel("c", DropTarget::Tabs(a));
        assert_eq!(t.tabs_of("c"), Some(a));
        // The emptied group is gone and the split has two children.
        assert!(matches!(t.root(), Some(DockNode::Split { children, .. }) if children.len() == 2));
        assert_eq!(t.active_of(a).as_deref(), Some("c"));
        assert_ne!(t.tabs_of("c"), Some(c));
    }

    #[test]
    fn edge_drop_splices_into_same_direction_parent() {
        let mut t = tree();
        let view = t.tabs_of("view").unwrap();
        t.move_panel("a", DropTarget::Edge(view, Side::Left));
        // Still one horizontal split, now with four children (no nesting).
        match t.root() {
            Some(DockNode::Split { dir, children, .. }) => {
                assert_eq!(*dir, Dir::Horizontal);
                assert_eq!(children.len(), 4);
                assert!(children.iter().all(|c| matches!(c, DockNode::Tabs { .. })));
            }
            other => panic!("{other:?}"),
        }
        let order = t.panels();
        assert_eq!(order, vec!["b", "a", "view", "c"]);
    }

    #[test]
    fn edge_drop_nests_in_cross_direction() {
        let mut t = tree();
        let view = t.tabs_of("view").unwrap();
        t.move_panel("c", DropTarget::Edge(view, Side::Bottom));
        match t.root() {
            Some(DockNode::Split { children, .. }) => {
                assert_eq!(children.len(), 2);
                assert!(
                    matches!(&children[1], DockNode::Split { dir: Dir::Vertical, children: cc, .. } if cc.len() == 2)
                );
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(t.panels(), vec!["a", "b", "view", "c"]);
    }

    #[test]
    fn close_open_and_self_drop() {
        let mut t = tree();
        t.close_panel("view");
        assert!(!t.contains("view"));
        assert!(matches!(t.root(), Some(DockNode::Split { children, .. }) if children.len() == 2));
        t.open_panel("view", None);
        assert!(t.contains("view"));
        assert_eq!(t.tabs_of("view"), t.tabs_of("a"));
        // Dropping onto its own group changes nothing.
        let before = t.clone();
        let g = t.tabs_of("a").unwrap();
        t.move_panel("a", DropTarget::Tabs(g));
        assert_eq!(t, before);
        // Closing everything empties the tree; opening again makes a root group.
        for p in t.panels() {
            t.close_panel(&p);
        }
        assert!(t.root().is_none());
        t.open_panel("x", None);
        assert_eq!(t.panels(), vec!["x"]);
    }

    #[test]
    fn resize_keeps_total_and_minimum() {
        let mut t = tree();
        t.resize(&[], 0, 0.2);
        match t.root() {
            Some(DockNode::Split { sizes, .. }) => {
                assert!((sizes.iter().sum::<f32>() - 3.0).abs() < 1e-4);
                assert!(sizes[0] > sizes[1]);
            }
            other => panic!("{other:?}"),
        }
        t.resize(&[], 0, -5.0);
        match t.root() {
            Some(DockNode::Split { sizes, .. }) => assert!(sizes[0] >= 3.0 * 0.05 - 1e-4),
            other => panic!("{other:?}"),
        }
    }
}
