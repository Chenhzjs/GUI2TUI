use std::fmt;

use crate::{semantic::RuntimeNodeId, tui::action::UiIntent};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionId(u64);

impl RegionId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for RegionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticRegionKind {
    Control,
    Field,
    Form,
    Selection,
    CommandSet,
    Navigation,
    Status,
    Content,
    OpaqueContent,
    Group,
    Unknown,
}

impl fmt::Display for SemanticRegionKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegionConfidence {
    Weak,
    Strong,
    Exact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalityPolicy {
    TerminalNative,
    FidelityPreferred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionInteraction {
    pub source: RuntimeNodeId,
    pub intent: UiIntent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticRegion {
    pub id: RegionId,
    pub kind: SemanticRegionKind,
    pub source_nodes: Vec<RuntimeNodeId>,
    pub label: Option<String>,
    pub descriptions: Vec<String>,
    pub errors: Vec<String>,
    pub logical_group: Vec<RuntimeNodeId>,
    pub children: Vec<SemanticRegion>,
    pub interactions: Vec<RegionInteraction>,
    pub confidence: RegionConfidence,
    pub modality: ModalityPolicy,
    pub command_path: Vec<String>,
}

impl SemanticRegion {
    pub fn terminal_native(
        id: RegionId,
        kind: SemanticRegionKind,
        source_nodes: Vec<RuntimeNodeId>,
    ) -> Self {
        Self {
            id,
            kind,
            source_nodes,
            label: None,
            descriptions: Vec::new(),
            errors: Vec::new(),
            logical_group: Vec::new(),
            children: Vec::new(),
            interactions: Vec::new(),
            confidence: RegionConfidence::Exact,
            modality: ModalityPolicy::TerminalNative,
            command_path: Vec::new(),
        }
    }
}
