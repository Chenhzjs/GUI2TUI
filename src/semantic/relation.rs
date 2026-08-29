use std::fmt;

use atspi::RelationType;

use super::{BackendLocator, RuntimeNodeId};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SemanticRelationKind {
    LabelFor,
    LabelledBy,
    DescriptionFor,
    DescribedBy,
    ErrorMessage,
    ErrorFor,
    MemberOf,
    ControllerFor,
    ControlledBy,
    FlowsTo,
    FlowsFrom,
    PopupFor,
    SubwindowOf,
    ParentWindowOf,
    Embeds,
    EmbeddedBy,
    TooltipFor,
    NodeChildOf,
    NodeParentOf,
    Details,
    DetailsFor,
    Other(String),
}

impl From<RelationType> for SemanticRelationKind {
    fn from(value: RelationType) -> Self {
        match value {
            RelationType::LabelFor => Self::LabelFor,
            RelationType::LabelledBy => Self::LabelledBy,
            RelationType::DescriptionFor => Self::DescriptionFor,
            RelationType::DescribedBy => Self::DescribedBy,
            RelationType::ErrorMessage => Self::ErrorMessage,
            RelationType::ErrorFor => Self::ErrorFor,
            RelationType::MemberOf => Self::MemberOf,
            RelationType::ControllerFor => Self::ControllerFor,
            RelationType::ControlledBy => Self::ControlledBy,
            RelationType::FlowsTo => Self::FlowsTo,
            RelationType::FlowsFrom => Self::FlowsFrom,
            RelationType::PopupFor => Self::PopupFor,
            RelationType::SubwindowOf => Self::SubwindowOf,
            RelationType::ParentWindowOf => Self::ParentWindowOf,
            RelationType::Embeds => Self::Embeds,
            RelationType::EmbeddedBy => Self::EmbeddedBy,
            RelationType::TooltipFor => Self::TooltipFor,
            RelationType::NodeChildOf => Self::NodeChildOf,
            RelationType::NodeParentOf => Self::NodeParentOf,
            RelationType::Details => Self::Details,
            RelationType::DetailsFor => Self::DetailsFor,
            RelationType::Null => Self::Other("null".to_owned()),
            RelationType::Extended => Self::Other("extended".to_owned()),
        }
    }
}

impl fmt::Display for SemanticRelationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Other(value) => write!(f, "Other({value})"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticRelationTarget {
    pub locator: BackendLocator,
    pub runtime_id: Option<RuntimeNodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticRelation {
    pub kind: SemanticRelationKind,
    pub targets: Vec<SemanticRelationTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum RelationState {
    #[default]
    Unknown,
    Known(Vec<SemanticRelation>),
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendRelation {
    pub kind: SemanticRelationKind,
    pub targets: Vec<BackendLocator>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_mapping_preserves_standard_and_extended_values() {
        assert_eq!(
            SemanticRelationKind::from(RelationType::LabelledBy),
            SemanticRelationKind::LabelledBy
        );
        assert_eq!(
            SemanticRelationKind::from(RelationType::Extended),
            SemanticRelationKind::Other("extended".to_owned())
        );
    }
}
