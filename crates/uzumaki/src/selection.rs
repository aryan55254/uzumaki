use crate::element::NodeId;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SelectionRange {
    /// Anchor point (where selection started), flat grapheme index
    pub anchor: usize,
    /// Active point / cursor position, flat grapheme index
    pub active: usize,
}

impl SelectionRange {
    pub fn new(anchor: usize, active: usize) -> Self {
        Self { anchor, active }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.active
    }

    pub fn start(&self) -> usize {
        self.anchor.min(self.active)
    }

    pub fn end(&self) -> usize {
        self.anchor.max(self.active)
    }

    pub fn set_cursor(&mut self, pos: usize) {
        self.anchor = pos;
        self.active = pos;
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DomSelection {
    /// The textSelect root that owns this selection.
    pub root: NodeId,
    pub range: SelectionRange,
}

impl DomSelection {
    pub fn new(root: NodeId, anchor: usize, active: usize) -> Self {
        Self {
            root,
            range: SelectionRange { anchor, active },
        }
    }
    #[inline]
    pub fn anchor(&self) -> usize {
        self.range.anchor
    }

    #[inline]
    pub fn active(&self) -> usize {
        self.range.active
    }

    pub fn set_cursor(&mut self, pos: usize) {
        self.range.set_cursor(pos);
    }

    pub fn start(&self) -> usize {
        self.range.anchor.min(self.range.active)
    }

    pub fn end(&self) -> usize {
        self.range.anchor.max(self.range.active)
    }

    pub fn is_collapsed(&self) -> bool {
        self.range.anchor == self.range.active
    }
}
