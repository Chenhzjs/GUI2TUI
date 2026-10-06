//! Runtime capability reconstruction from public Accessibility evidence.
//!
//! This module is intentionally below presentation. It records what exists in
//! the current GUI, which operations are publicly declared, and which physical
//! affordances were verified in this runtime session. Learned knowledge never
//! grants operation authority; callers must still use the normal runtime,
//! generation, scope, ticket, fresh-target and readback chain.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt,
    time::Instant,
};

use serde::Serialize;

pub mod grounding;
pub mod self_description;
use self_description::SelfDescriptionCatalog;

use crate::{
    runtime::{ApplicationGenerationId, RuntimeSessionId},
    semantic::{
        BackendLocator, Geometry, RelationState, RuntimeNodeId, SemanticAction, SemanticCache,
        SemanticCapability, SemanticRole, SemanticState, TextInputKind,
    },
    transcompile::{InteractionScopeId, InteractionScopes},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyIdentity {
    Enter,
    Space,
    Character(char),
}

impl fmt::Display for KeyIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Enter => formatter.write_str("Enter"),
            Self::Space => formatter.write_str("Space"),
            Self::Character(character) => character.fmt(formatter),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct ModifierSet {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl ModifierSet {
    pub const CONTROL: Self = Self {
        control: true,
        alt: false,
        shift: false,
        meta: false,
    };

    pub const fn is_empty(self) -> bool {
        !self.control && !self.alt && !self.shift && !self.meta
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct KeyboardPrimitive {
    pub key: KeyIdentity,
    pub modifiers: ModifierSet,
}

impl KeyboardPrimitive {
    pub const ENTER: Self = Self {
        key: KeyIdentity::Enter,
        modifiers: ModifierSet {
            control: false,
            alt: false,
            shift: false,
            meta: false,
        },
    };

    pub const SPACE: Self = Self {
        key: KeyIdentity::Space,
        modifiers: ModifierSet {
            control: false,
            alt: false,
            shift: false,
            meta: false,
        },
    };

    pub const fn control(character: char) -> Self {
        Self {
            key: KeyIdentity::Character(character),
            modifiers: ModifierSet::CONTROL,
        }
    }
}

impl fmt::Display for KeyboardPrimitive {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.control {
            formatter.write_str("Ctrl+")?;
        }
        if self.modifiers.alt {
            formatter.write_str("Alt+")?;
        }
        if self.modifiers.shift {
            formatter.write_str("Shift+")?;
        }
        if self.modifiers.meta {
            formatter.write_str("Meta+")?;
        }
        self.key.fmt(formatter)
    }
}

/// Parse one publicly advertised key binding. This is deliberately a small,
/// platform-neutral parser, not a widget-to-key table or a keyboard fuzzing
/// vocabulary.
pub fn parse_advertised_keybinding(binding: &str) -> Option<KeyboardPrimitive> {
    let normalized = binding.trim().replace('<', "").replace(['>', '-'], "+");
    if normalized.is_empty() {
        return None;
    }
    let mut modifiers = ModifierSet::default();
    let mut key = None;
    for token in normalized
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "primary" => modifiers.control = true,
            "alt" | "mod1" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            "meta" | "super" | "mod4" => modifiers.meta = true,
            "enter" | "return" => key = Some(KeyIdentity::Enter),
            "space" | "spacebar" => key = Some(KeyIdentity::Space),
            _ if token.chars().count() == 1 => {
                if key.is_some() {
                    return None;
                }
                key = token
                    .chars()
                    .next()
                    .map(|character| KeyIdentity::Character(character.to_ascii_lowercase()))
            }
            _ => return None,
        }
    }
    Some(KeyboardPrimitive {
        key: key?,
        modifiers,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryClass {
    Unavailable,
    PointLike,
    Compact,
    Wide,
    Tall,
    Large,
}

impl GeometryClass {
    fn from_geometry(geometry: Option<&Geometry>) -> Self {
        let Some(geometry) = geometry else {
            return Self::Unavailable;
        };
        if geometry.width <= 1 || geometry.height <= 1 {
            Self::PointLike
        } else if geometry.width >= geometry.height.saturating_mul(4) {
            Self::Wide
        } else if geometry.height >= geometry.width.saturating_mul(4) {
            Self::Tall
        } else if i64::from(geometry.width) * i64::from(geometry.height) >= 160_000 {
            Self::Large
        } else {
            Self::Compact
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextMetadata {
    pub kind: TextInputKind,
    pub value_available: bool,
    pub character_count: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionMetadata {
    pub selected: bool,
    pub selectable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeObject {
    pub runtime_id: RuntimeNodeId,
    pub backend_locator: BackendLocator,
    pub application_generation: ApplicationGenerationId,
    pub interaction_scope: InteractionScopeId,
    pub role: SemanticRole,
    pub name: Option<String>,
    pub description: Option<String>,
    pub states: Vec<SemanticState>,
    pub interfaces: Vec<String>,
    pub relations: RelationState,
    pub advertised_actions: Vec<SemanticAction>,
    pub advertised_keybindings: Vec<(String, KeyboardPrimitive)>,
    pub semantic_capabilities: Vec<SemanticCapability>,
    pub text: Option<TextMetadata>,
    pub selection: SelectionMetadata,
    pub geometry: Option<Geometry>,
    pub parent: Option<RuntimeNodeId>,
    pub children: Vec<RuntimeNodeId>,
    pub semantic_region: Option<RuntimeNodeId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RuntimeObjectMetrics {
    pub object_count: usize,
    pub graph_build_micros: u64,
    pub estimated_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct RuntimeObjectModel {
    generation: ApplicationGenerationId,
    objects: BTreeMap<RuntimeNodeId, RuntimeObject>,
    by_locator: HashMap<BackendLocator, RuntimeNodeId>,
    pub metrics: RuntimeObjectMetrics,
}

impl RuntimeObjectModel {
    pub fn from_cache(
        cache: &SemanticCache,
        scopes: &InteractionScopes,
        generation: ApplicationGenerationId,
    ) -> Self {
        let started = Instant::now();
        let mut objects = BTreeMap::new();
        let mut by_locator = HashMap::new();
        for node in cache.nodes() {
            let Some(interaction_scope) = scopes.scope_for_node(node.runtime_id) else {
                continue;
            };
            let text = node.text_input_kind.map(|kind| TextMetadata {
                kind,
                value_available: kind != TextInputKind::Password && node.value.is_some(),
                character_count: (kind != TextInputKind::Password)
                    .then(|| node.value.as_ref().map(|value| value.chars().count()))
                    .flatten(),
            });
            let advertised_keybindings = node
                .actions
                .iter()
                .flat_map(|action| {
                    action
                        .keybinding
                        .as_deref()
                        .unwrap_or("")
                        .split(';')
                        .filter_map(|raw| {
                            parse_advertised_keybinding(raw).map(|parsed| (raw.to_owned(), parsed))
                        })
                })
                .collect();
            let object = RuntimeObject {
                runtime_id: node.runtime_id,
                backend_locator: node.backend_locator.clone(),
                application_generation: generation,
                interaction_scope,
                role: node.role.clone(),
                name: node.name.clone(),
                description: node.description.clone(),
                states: node.states.clone(),
                interfaces: sorted_unique(node.debug.interfaces.clone()),
                relations: cache
                    .relation_state(node.runtime_id)
                    .cloned()
                    .unwrap_or_default(),
                advertised_actions: node.actions.clone(),
                advertised_keybindings,
                semantic_capabilities: node.capabilities.clone(),
                text,
                selection: SelectionMetadata {
                    selected: node.states.contains(&SemanticState::Selected),
                    selectable: has_state(&node.states, "selectable")
                        || node.capabilities.iter().any(|capability| {
                            matches!(
                                capability,
                                SemanticCapability::SelectChildren
                                    | SemanticCapability::SelectCurrentChild
                                    | SemanticCapability::SelectCurrentTableRow
                            )
                        }),
                },
                geometry: node.debug.geometry.clone(),
                parent: node.parent,
                children: node.children.clone(),
                semantic_region: nearest_semantic_region(cache, node.runtime_id),
            };
            by_locator.insert(object.backend_locator.clone(), node.runtime_id);
            objects.insert(node.runtime_id, object);
        }
        let estimated_bytes = objects.values().map(estimate_object_bytes).sum();
        Self {
            generation,
            metrics: RuntimeObjectMetrics {
                object_count: objects.len(),
                graph_build_micros: micros(started.elapsed().as_micros()),
                estimated_bytes,
            },
            objects,
            by_locator,
        }
    }

    pub fn generation(&self) -> ApplicationGenerationId {
        self.generation
    }

    pub fn object(&self, id: RuntimeNodeId) -> Option<&RuntimeObject> {
        self.objects.get(&id)
    }

    pub fn object_by_locator(&self, locator: &BackendLocator) -> Option<&RuntimeObject> {
        self.by_locator.get(locator).and_then(|id| self.object(*id))
    }

    pub fn objects(&self) -> impl Iterator<Item = &RuntimeObject> {
        self.objects.values()
    }

    pub fn context_signature(&self, target: RuntimeNodeId) -> Option<ContextSignature> {
        let object = self.object(target)?;
        let mut ancestor_roles = Vec::new();
        let mut current = object.parent;
        for _ in 0..3 {
            let Some(parent) = current.and_then(|id| self.object(id)) else {
                break;
            };
            ancestor_roles.push(parent.role.to_string());
            current = parent.parent;
        }
        let sibling_roles = object
            .parent
            .and_then(|id| self.object(id))
            .map(|parent| {
                parent
                    .children
                    .iter()
                    .filter(|id| **id != target)
                    .filter_map(|id| self.object(*id))
                    .map(|sibling| sibling.role.to_string())
                    .collect::<Vec<_>>()
            })
            .map(sorted_unique)
            .unwrap_or_default();
        let relation_kinds = match &object.relations {
            RelationState::Known(relations) => sorted_unique(
                relations
                    .iter()
                    .map(|relation| relation.kind.to_string())
                    .collect(),
            ),
            RelationState::Unknown | RelationState::Unavailable => Vec::new(),
        };
        let advertised = sorted_unique(
            object
                .advertised_actions
                .iter()
                .map(|action| action.name.to_ascii_lowercase())
                .chain(
                    object
                        .semantic_capabilities
                        .iter()
                        .map(|capability| format!("{capability:?}")),
                )
                .collect(),
        );
        Some(ContextSignature {
            role: object.role.to_string(),
            interfaces: object.interfaces.clone(),
            important_states: important_states(&object.states),
            relation_kinds,
            ancestor_roles,
            sibling_roles,
            region_role: object
                .semantic_region
                .and_then(|id| self.object(id))
                .map(|region| region.role.to_string()),
            advertised,
            editable: object
                .semantic_capabilities
                .contains(&SemanticCapability::EditText)
                || object.states.contains(&SemanticState::Editable),
            focusable: has_state(&object.states, "focusable"),
            scrollable: object
                .interfaces
                .iter()
                .any(|interface| interface.to_ascii_lowercase().contains("component"))
                && has_state(&object.states, "scrollable"),
            geometry_class: GeometryClass::from_geometry(object.geometry.as_ref()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct ContextSignature {
    pub role: String,
    pub interfaces: Vec<String>,
    pub important_states: Vec<String>,
    pub relation_kinds: Vec<String>,
    pub ancestor_roles: Vec<String>,
    pub sibling_roles: Vec<String>,
    pub region_role: Option<String>,
    pub advertised: Vec<String>,
    pub editable: bool,
    pub focusable: bool,
    pub scrollable: bool,
    pub geometry_class: GeometryClass,
}

impl ContextSignature {
    /// Deterministic structural similarity in the inclusive range 0..=1000.
    /// Role alone contributes only 200 and can never synthesize a candidate.
    pub fn similarity(&self, other: &Self) -> u16 {
        let mut score = 0_u16;
        if self.role == other.role {
            score += 200;
        }
        score += scaled_jaccard(&self.interfaces, &other.interfaces, 240);
        score += scaled_jaccard(&self.important_states, &other.important_states, 130);
        score += scaled_jaccard(&self.relation_kinds, &other.relation_kinds, 80);
        score += sequence_similarity(&self.ancestor_roles, &other.ancestor_roles, 130);
        score += scaled_jaccard(&self.sibling_roles, &other.sibling_roles, 70);
        if self.region_role == other.region_role {
            score += 40;
        }
        score += scaled_jaccard(&self.advertised, &other.advertised, 50);
        if self.editable == other.editable {
            score += 20;
        }
        if self.focusable == other.focusable {
            score += 20;
        }
        if self.scrollable == other.scrollable {
            score += 10;
        }
        if self.geometry_class == other.geometry_class {
            score += 50;
        }
        if materially_different_nonempty_sets(&self.interfaces, &other.interfaces) {
            score = score.saturating_sub(200);
        }
        score.min(1000)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CapabilityOperation {
    AccessibilityInvoke(String),
    SetText,
    SetValue,
    SetSelection,
    Focus,
    Keyboard(KeyboardPrimitive),
    TextInput,
}

impl fmt::Display for CapabilityOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessibilityInvoke(action) => write!(formatter, "AccessibilityInvoke({action})"),
            Self::SetText => formatter.write_str("SetText"),
            Self::SetValue => formatter.write_str("SetValue"),
            Self::SetSelection => formatter.write_str("SetSelection"),
            Self::Focus => formatter.write_str("Focus"),
            Self::Keyboard(primitive) => write!(formatter, "Keyboard({primitive})"),
            Self::TextInput => formatter.write_str("TextInput"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Declared,
    Structural,
    Observed,
    Hypothesized,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub detail: String,
}

/// Machine-readable explanation for why one candidate exists.  This is kept
/// separate from the human-oriented evidence strings so research tooling does
/// not have to parse prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateSource {
    AdvertisedAction,
    AdvertisedKeybinding,
    StructuralCapability,
    ExactContextHistory,
    SimilarContextHistory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CandidateProvenance {
    pub source: CandidateSource,
    pub supporting_observations: u32,
    pub context_similarity_milli: Option<u16>,
    pub history_successes: u32,
    pub history_failures: u32,
    pub direct_successes: u32,
    pub direct_failures: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateLifecycle {
    Declared,
    Hypothesized,
    Observed,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    FocusChanged,
    TextChanged,
    ValueChanged,
    SelectionChanged,
    ExpandedChanged,
    StateChanged,
    StructureChanged,
    DocumentChanged,
    WindowChanged,
    DialogOpened,
    DialogClosed,
    TargetBecameStale,
    ExternalEffectObserved,
    NoRelevantEffect,
    NoDetectableChange,
    Timeout,
    UnknownEffect,
}

impl Effect {
    pub fn is_verified_relevant(self) -> bool {
        !matches!(
            self,
            Self::NoRelevantEffect | Self::NoDetectableChange | Self::Timeout | Self::UnknownEffect
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    Observational,
    ReversibleNavigation,
    LocalMutation,
    ExternalEffect,
    PotentiallyDestructive,
    Unknown,
}

impl RiskClass {
    /// Unknown effect is not itself proof of danger. Explicitly dangerous
    /// classes remain hard-blocked by autonomous exploration.
    pub fn is_explicitly_dangerous(self) -> bool {
        matches!(self, Self::ExternalEffect | Self::PotentiallyDestructive)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryRoute {
    Accessibility,
    NativeKeyboard,
    NativeText,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationPlan {
    FocusReadback,
    TextReadback,
    ValueReadback,
    SelectionReadback,
    RelevantSurfaceObservation,
    ExternalObserver,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidityScope {
    pub session: RuntimeSessionId,
    pub generation: ApplicationGenerationId,
    pub interaction_scope: InteractionScopeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredCapability {
    pub target: RuntimeNodeId,
    pub operation: CapabilityOperation,
    pub preconditions: Vec<String>,
    pub delivery_route: DeliveryRoute,
    pub evidence: Vec<Evidence>,
    pub expected_effects: Vec<Effect>,
    pub verification: VerificationPlan,
    pub confidence_milli: u16,
    pub risk: RiskClass,
    pub validity: ValidityScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Affordance {
    pub target: RuntimeNodeId,
    pub target_locator: BackendLocator,
    pub context: ContextSignature,
    pub operation: CapabilityOperation,
    pub preconditions: Vec<String>,
    pub delivery_route: DeliveryRoute,
    pub evidence: Vec<Evidence>,
    pub provenance: CandidateProvenance,
    pub lifecycle: CandidateLifecycle,
    pub effect: Effect,
    pub verification: VerificationPlan,
    pub confidence_milli: u16,
    pub risk: RiskClass,
    pub validity: ValidityScope,
    pub success_count: u32,
    pub failure_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectedCandidate {
    pub target_locator: BackendLocator,
    pub context: ContextSignature,
    pub operation: CapabilityOperation,
    pub effect: Effect,
    pub failure_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectRecord {
    pub target_locator: BackendLocator,
    pub operation: CapabilityOperation,
    pub effect: Effect,
    pub verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LearnedAffordance {
    context: ContextSignature,
    operation: CapabilityOperation,
    effect: Effect,
    success_count: u32,
    failure_count: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DirectHistory {
    successes: u32,
    failures: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CandidateMetrics {
    pub runtime_objects: usize,
    pub raw_possible_primitives: usize,
    pub generated_candidates: usize,
    pub retained_candidates: usize,
    pub synthesis_micros: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HumanCapability {
    Activate,
    Toggle,
    Select,
    Expand,
    Collapse,
    SwitchPage,
    Choose,
    OpenMenu,
    EditText,
    AdjustValue,
    BrowseContent,
}

#[derive(Clone, Debug)]
pub struct CapabilityGraph {
    session: RuntimeSessionId,
    generation: ApplicationGenerationId,
    objects: RuntimeObjectModel,
    declared: Vec<DeclaredCapability>,
    observed: Vec<Affordance>,
    hypothesized: Vec<Affordance>,
    rejected: Vec<RejectedCandidate>,
    effects: Vec<EffectRecord>,
    learned: Vec<LearnedAffordance>,
    direct_history: HashMap<(BackendLocator, CapabilityOperation), DirectHistory>,
    human_projection: HashMap<RuntimeNodeId, BTreeSet<HumanCapability>>,
    self_description: SelfDescriptionCatalog,
    self_description_enabled: bool,
    pub last_candidate_metrics: CandidateMetrics,
}

impl CapabilityGraph {
    pub fn new(
        session: RuntimeSessionId,
        generation: ApplicationGenerationId,
        cache: &SemanticCache,
        scopes: &InteractionScopes,
    ) -> Self {
        let objects = RuntimeObjectModel::from_cache(cache, scopes, generation);
        let declared = build_declared_capabilities(&session, &objects);
        Self {
            session,
            generation,
            objects,
            declared,
            observed: Vec::new(),
            hypothesized: Vec::new(),
            rejected: Vec::new(),
            effects: Vec::new(),
            learned: Vec::new(),
            direct_history: HashMap::new(),
            human_projection: HashMap::new(),
            self_description: SelfDescriptionCatalog::default(),
            self_description_enabled: false,
            last_candidate_metrics: CandidateMetrics::default(),
        }
    }

    /// Refresh current objects while retaining only session/generation-scoped
    /// learning. A generation change discards learned target knowledge.
    pub fn refresh(
        &mut self,
        generation: ApplicationGenerationId,
        cache: &SemanticCache,
        scopes: &InteractionScopes,
    ) {
        if generation != self.generation {
            self.learned.clear();
            self.direct_history.clear();
            self.effects.clear();
            self.observed.clear();
            self.hypothesized.clear();
            self.rejected.clear();
            self.human_projection.clear();
            self.generation = generation;
        }
        self.objects = RuntimeObjectModel::from_cache(cache, scopes, generation);
        self.declared = build_declared_capabilities(&self.session, &self.objects);
        self.human_projection
            .retain(|target, _| self.objects.object(*target).is_some());
        // Full public snapshots allocate new RuntimeNodeIds even when the
        // backend object survives. Rebind historical evidence by exact locator,
        // never by the old snapshot-local ID. This does not authorize delivery.
        self.observed.retain_mut(|affordance| {
            let Some(object) = self.objects.object_by_locator(&affordance.target_locator) else {
                return false;
            };
            affordance.target = object.runtime_id;
            affordance.validity.interaction_scope = object.interaction_scope;
            true
        });
        self.hypothesized.clear();
        if self.self_description_enabled {
            self.self_description = SelfDescriptionCatalog::mine(&self.objects);
        }
    }

    pub fn session(&self) -> &RuntimeSessionId {
        &self.session
    }

    pub fn generation(&self) -> ApplicationGenerationId {
        self.generation
    }

    pub fn objects(&self) -> &RuntimeObjectModel {
        &self.objects
    }

    /// Explicit research acquisition. Described claims stay separate from
    /// executable target-bound affordances, inside this one capability graph.
    pub fn acquire_self_descriptions(&mut self) -> &SelfDescriptionCatalog {
        self.self_description_enabled = true;
        self.self_description = SelfDescriptionCatalog::mine(&self.objects);
        &self.self_description
    }

    pub fn self_descriptions(&self) -> &SelfDescriptionCatalog {
        &self.self_description
    }

    pub fn declared_capabilities(&self) -> &[DeclaredCapability] {
        &self.declared
    }

    pub fn observed_affordances(&self) -> &[Affordance] {
        &self.observed
    }

    pub fn hypothesized_affordances(&self) -> &[Affordance] {
        &self.hypothesized
    }

    pub fn rejected_candidates(&self) -> &[RejectedCandidate] {
        &self.rejected
    }

    pub fn effects(&self) -> &[EffectRecord] {
        &self.effects
    }

    pub fn context_signature(&self, target: RuntimeNodeId) -> Option<ContextSignature> {
        self.objects.context_signature(target)
    }

    /// Register a capability already selected by the conservative semantic
    /// projection. The graph becomes the final source consulted by the scene;
    /// physical hypotheses are never registered here.
    pub fn register_human_capability(
        &mut self,
        target: RuntimeNodeId,
        capability: HumanCapability,
    ) -> bool {
        let Some(object) = self.objects.object(target) else {
            return false;
        };
        let parent = object.parent.and_then(|id| self.objects.object(id));
        let supported = match capability {
            HumanCapability::EditText => object
                .semantic_capabilities
                .contains(&SemanticCapability::EditText),
            HumanCapability::AdjustValue => object
                .semantic_capabilities
                .contains(&SemanticCapability::Value),
            HumanCapability::BrowseContent => object.role == SemanticRole::Table,
            HumanCapability::Choose => {
                object.selection.selectable
                    || object.role == SemanticRole::ComboBox
                    || object.semantic_capabilities.iter().any(|capability| {
                        matches!(
                            capability,
                            SemanticCapability::SelectChildren
                                | SemanticCapability::SelectCurrentChild
                        )
                    })
            }
            HumanCapability::Select => {
                !object.advertised_actions.is_empty()
                    || object.selection.selectable
                    || parent.is_some_and(|parent| {
                        parent.semantic_capabilities.iter().any(|capability| {
                            matches!(
                                capability,
                                SemanticCapability::SelectChildren
                                    | SemanticCapability::SelectCurrentChild
                                    | SemanticCapability::SelectCurrentTableRow
                            )
                        })
                    })
            }
            HumanCapability::SwitchPage => {
                !object.advertised_actions.is_empty()
                    || parent.is_some_and(|parent| {
                        parent
                            .semantic_capabilities
                            .contains(&SemanticCapability::SelectChildren)
                    })
            }
            HumanCapability::Activate
            | HumanCapability::Toggle
            | HumanCapability::Expand
            | HumanCapability::Collapse
            | HumanCapability::OpenMenu => {
                !object.advertised_actions.is_empty() || object.selection.selectable
            }
        };
        if supported {
            self.human_projection
                .entry(target)
                .or_default()
                .insert(capability);
        }
        supported
    }

    pub fn has_human_capability(&self, target: RuntimeNodeId, capability: HumanCapability) -> bool {
        self.human_projection
            .get(&target)
            .is_some_and(|capabilities| capabilities.contains(&capability))
    }

    /// Record an operation-scoped observation. NoRelevantEffect, timeout and
    /// unknown results lower local confidence but never create an observed
    /// affordance.
    pub fn record_observation(
        &mut self,
        target: RuntimeNodeId,
        operation: CapabilityOperation,
        effect: Effect,
    ) -> Option<&Affordance> {
        let object = self.objects.object(target)?.clone();
        let context = self.objects.context_signature(target)?;
        self.record_observation_with_context(
            object.backend_locator,
            object.interaction_scope,
            target,
            context,
            operation,
            effect,
        )
    }

    /// Used when observation replaced the original RuntimeNodeId. The context
    /// must have been captured before delivery; it is evidence, never target
    /// authority for a later operation.
    pub fn record_observation_with_context(
        &mut self,
        target_locator: BackendLocator,
        interaction_scope: InteractionScopeId,
        target: RuntimeNodeId,
        context: ContextSignature,
        operation: CapabilityOperation,
        effect: Effect,
    ) -> Option<&Affordance> {
        let verified = effect.is_verified_relevant();
        let direct = self
            .direct_history
            .entry((target_locator.clone(), operation.clone()))
            .or_default();
        if verified {
            direct.successes = direct.successes.saturating_add(1);
        } else {
            direct.failures = direct.failures.saturating_add(1);
        }
        self.effects.push(EffectRecord {
            target_locator: target_locator.clone(),
            operation: operation.clone(),
            effect,
            verified,
        });
        if let Some(learned) = self
            .learned
            .iter_mut()
            .find(|learned| learned.context == context && learned.operation == operation)
        {
            if verified {
                learned.success_count = learned.success_count.saturating_add(1);
                learned.effect = effect;
            } else {
                learned.failure_count = learned.failure_count.saturating_add(1);
            }
        } else {
            self.learned.push(LearnedAffordance {
                context: context.clone(),
                operation: operation.clone(),
                effect,
                success_count: u32::from(verified),
                failure_count: u32::from(!verified),
            });
        }
        if !verified {
            self.observed.retain(|affordance| {
                affordance.target_locator != target_locator || affordance.operation != operation
            });
            self.rejected.retain(|candidate| {
                candidate.target_locator != target_locator || candidate.operation != operation
            });
            self.rejected.push(RejectedCandidate {
                target_locator,
                context,
                operation,
                effect,
                failure_count: direct.failures,
            });
            return None;
        }
        let (route, verification) = operation_contract(&operation);
        let success_count = direct.successes;
        let failure_count = direct.failures;
        let (history_successes, history_failures) = self
            .learned
            .iter()
            .find(|learned| learned.context == context && learned.operation == operation)
            .map(|learned| (learned.success_count, learned.failure_count))
            .unwrap_or((success_count, failure_count));
        let confidence_milli = observed_confidence(history_successes, history_failures);
        let affordance = Affordance {
            target,
            target_locator: target_locator.clone(),
            context,
            operation: operation.clone(),
            preconditions: physical_preconditions(&operation),
            delivery_route: route,
            evidence: vec![Evidence {
                kind: EvidenceKind::Observed,
                detail: format!(
                    "operation-scoped effect {effect:?}; successes={success_count} failures={failure_count}"
                ),
            }],
            provenance: CandidateProvenance {
                source: CandidateSource::ExactContextHistory,
                supporting_observations: history_successes,
                context_similarity_milli: Some(1000),
                history_successes,
                history_failures,
                direct_successes: success_count,
                direct_failures: failure_count,
            },
            lifecycle: CandidateLifecycle::Observed,
            effect,
            verification,
            confidence_milli,
            risk: risk_for(&operation, Some(effect)),
            validity: ValidityScope {
                session: self.session.clone(),
                generation: self.generation,
                interaction_scope,
            },
            success_count,
            failure_count,
        };
        self.observed.retain(|candidate| {
            candidate.target_locator != target_locator || candidate.operation != operation
        });
        self.rejected.retain(|candidate| {
            candidate.target_locator != target_locator || candidate.operation != operation
        });
        self.observed.push(affordance);
        self.observed.last()
    }

    pub fn synthesize_candidates(&mut self, target: RuntimeNodeId) -> Vec<Affordance> {
        self.synthesize_candidates_with_history(target, true)
    }

    /// Research ablation: current declarations only, without transferring
    /// evidence even from operations earlier in this same bootstrap session.
    pub fn synthesize_without_history(&mut self, target: RuntimeNodeId) -> Vec<Affordance> {
        self.synthesize_candidates_with_history(target, false)
    }

    fn synthesize_candidates_with_history(
        &mut self,
        target: RuntimeNodeId,
        include_history: bool,
    ) -> Vec<Affordance> {
        let started = Instant::now();
        let Some(object) = self.objects.object(target).cloned() else {
            return Vec::new();
        };
        let Some(context) = self.objects.context_signature(target) else {
            return Vec::new();
        };
        let mut candidates: BTreeMap<CapabilityOperation, Affordance> = BTreeMap::new();
        for declared in self
            .declared
            .iter()
            .filter(|capability| capability.target == target)
        {
            candidates.insert(
                declared.operation.clone(),
                Affordance {
                    target,
                    target_locator: object.backend_locator.clone(),
                    context: context.clone(),
                    operation: declared.operation.clone(),
                    preconditions: declared.preconditions.clone(),
                    delivery_route: declared.delivery_route,
                    evidence: declared.evidence.clone(),
                    provenance: CandidateProvenance {
                        source: match &declared.operation {
                            CapabilityOperation::Keyboard(_) => {
                                CandidateSource::AdvertisedKeybinding
                            }
                            CapabilityOperation::AccessibilityInvoke(_) => {
                                CandidateSource::AdvertisedAction
                            }
                            _ => CandidateSource::StructuralCapability,
                        },
                        supporting_observations: 0,
                        context_similarity_milli: None,
                        history_successes: 0,
                        history_failures: 0,
                        direct_successes: 0,
                        direct_failures: 0,
                    },
                    lifecycle: CandidateLifecycle::Declared,
                    effect: declared
                        .expected_effects
                        .first()
                        .copied()
                        .unwrap_or(Effect::UnknownEffect),
                    verification: declared.verification,
                    confidence_milli: declared.confidence_milli,
                    risk: declared.risk,
                    validity: declared.validity.clone(),
                    success_count: 0,
                    failure_count: 0,
                },
            );
        }
        let raw_possible_primitives = candidates.len().saturating_add(
            self.learned
                .iter()
                .filter(|learned| include_history && learned.success_count > 0)
                .count(),
        );
        let mut generated = candidates.len();
        for learned in self.learned.iter().filter(|_| include_history) {
            if learned.success_count == 0 {
                continue;
            }
            let similarity = context.similarity(&learned.context);
            if similarity < 700 {
                continue;
            }
            generated = generated.saturating_add(1);
            let direct = self
                .direct_history
                .get(&(object.backend_locator.clone(), learned.operation.clone()))
                .cloned()
                .unwrap_or_default();
            let confidence = hypothesized_confidence(
                similarity,
                learned.success_count,
                learned.failure_count,
                direct.successes,
                direct.failures,
            );
            if confidence < 400 || direct.failures > direct.successes {
                continue;
            }
            let (route, verification) = operation_contract(&learned.operation);
            let affordance = Affordance {
                target,
                target_locator: object.backend_locator.clone(),
                context: context.clone(),
                operation: learned.operation.clone(),
                preconditions: physical_preconditions(&learned.operation),
                delivery_route: route,
                evidence: vec![Evidence {
                    kind: if direct.successes > 0 {
                        EvidenceKind::Observed
                    } else {
                        EvidenceKind::Hypothesized
                    },
                    detail: format!(
                        "context_similarity={similarity} history={}/{} direct={}/{}",
                        learned.success_count,
                        learned.failure_count,
                        direct.successes,
                        direct.failures
                    ),
                }],
                provenance: CandidateProvenance {
                    source: if context == learned.context {
                        CandidateSource::ExactContextHistory
                    } else {
                        CandidateSource::SimilarContextHistory
                    },
                    supporting_observations: learned.success_count,
                    context_similarity_milli: Some(similarity),
                    history_successes: learned.success_count,
                    history_failures: learned.failure_count,
                    direct_successes: direct.successes,
                    direct_failures: direct.failures,
                },
                lifecycle: if direct.successes > 0 {
                    CandidateLifecycle::Observed
                } else {
                    CandidateLifecycle::Hypothesized
                },
                effect: learned.effect,
                verification,
                confidence_milli: confidence,
                risk: risk_for(&learned.operation, Some(learned.effect)),
                validity: ValidityScope {
                    session: self.session.clone(),
                    generation: self.generation,
                    interaction_scope: object.interaction_scope,
                },
                success_count: learned.success_count,
                failure_count: learned.failure_count,
            };
            candidates
                .entry(learned.operation.clone())
                .and_modify(|existing| {
                    if affordance.confidence_milli > existing.confidence_milli {
                        *existing = affordance.clone();
                    }
                })
                .or_insert(affordance);
        }
        let mut retained: Vec<_> = candidates
            .into_values()
            .filter(|candidate| {
                !self.rejected.iter().any(|rejected| {
                    rejected.target_locator == object.backend_locator
                        && rejected.context == context
                        && rejected.operation == candidate.operation
                })
            })
            .collect();
        retained.sort_by_key(candidate_rank_key);
        self.hypothesized
            .retain(|candidate| candidate.target != target);
        self.hypothesized.extend(
            retained
                .iter()
                .filter(|candidate| {
                    candidate
                        .evidence
                        .iter()
                        .any(|evidence| evidence.kind == EvidenceKind::Hypothesized)
                })
                .cloned(),
        );
        self.last_candidate_metrics = CandidateMetrics {
            runtime_objects: self.objects.metrics.object_count,
            raw_possible_primitives,
            generated_candidates: generated,
            retained_candidates: retained.len(),
            synthesis_micros: micros(started.elapsed().as_micros()),
        };
        retained
    }

    pub fn snapshot(&self) -> CapabilitySnapshot {
        CapabilitySnapshot {
            generation: self.generation.0,
            object_count: self.objects.metrics.object_count,
            declared_capability_count: self.declared.len(),
            observed_affordance_count: self.observed.len(),
            hypothesized_affordance_count: self.hypothesized.len(),
            rejected_candidate_count: self.rejected.len(),
            effect_count: self.effects.len(),
            human_capability_count: self.human_projection.values().map(BTreeSet::len).sum(),
            graph_build_micros: self.objects.metrics.graph_build_micros,
            estimated_bytes: self.objects.metrics.estimated_bytes,
            candidate_metrics: self.last_candidate_metrics,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CapabilitySnapshot {
    pub generation: u64,
    pub object_count: usize,
    pub declared_capability_count: usize,
    pub observed_affordance_count: usize,
    pub hypothesized_affordance_count: usize,
    pub rejected_candidate_count: usize,
    pub effect_count: usize,
    pub human_capability_count: usize,
    pub graph_build_micros: u64,
    pub estimated_bytes: usize,
    pub candidate_metrics: CandidateMetrics,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityGap {
    ObservationGap,
    AddressabilityGap,
    AffordanceDiscoveryGap,
    ActuationGap,
    VerificationGap,
    UnsafeExplorationGap,
    OpaqueRegionGap,
    UnknownGap,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GapEvidence {
    pub visible_in_gui: bool,
    pub accessibility_object: bool,
    pub addressable: bool,
    pub delivery_available: bool,
    pub verification_available: bool,
    pub candidate_available: bool,
    pub exploration_safe: bool,
    pub opaque_region: bool,
}

pub fn classify_gap(evidence: GapEvidence) -> CapabilityGap {
    if evidence.opaque_region {
        CapabilityGap::OpaqueRegionGap
    } else if evidence.visible_in_gui && !evidence.accessibility_object {
        CapabilityGap::ObservationGap
    } else if evidence.accessibility_object && !evidence.addressable {
        CapabilityGap::AddressabilityGap
    } else if evidence.addressable && !evidence.delivery_available {
        CapabilityGap::ActuationGap
    } else if evidence.delivery_available && !evidence.verification_available {
        CapabilityGap::VerificationGap
    } else if evidence.verification_available && !evidence.candidate_available {
        CapabilityGap::AffordanceDiscoveryGap
    } else if evidence.verification_available && !evidence.exploration_safe {
        CapabilityGap::UnsafeExplorationGap
    } else {
        CapabilityGap::UnknownGap
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplorationEnvironment {
    Ordinary,
    Fixture,
    DisposableSession,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExplorationPolicy {
    pub enabled: bool,
    pub environment: ExplorationEnvironment,
    pub operation_budget: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ExplorationBudget {
    pub max_hypotheses_per_state: u32,
    pub max_repeated_failures: u32,
    pub max_retries_per_candidate: u32,
}

impl ExplorationBudget {
    /// Derive the state budget from the already sparse candidate set.  This
    /// never expands the primitive vocabulary and caps even a noisy provider.
    pub fn for_candidate_count(candidate_count: usize) -> Self {
        Self {
            max_hypotheses_per_state: candidate_count.min(8) as u32,
            max_repeated_failures: 2,
            max_retries_per_candidate: 1,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExplorationLedger {
    attempted_by_context: HashMap<(BackendLocator, ContextSignature), u32>,
    failures_by_context: HashMap<(BackendLocator, ContextSignature), u32>,
    attempts_by_candidate: HashMap<(BackendLocator, ContextSignature, CapabilityOperation), u32>,
}

impl ExplorationLedger {
    pub fn allows(&self, candidate: &Affordance, budget: ExplorationBudget) -> bool {
        self.attempted_by_context
            .get(&(candidate.target_locator.clone(), candidate.context.clone()))
            .copied()
            .unwrap_or(0)
            < budget.max_hypotheses_per_state
            && self
                .failures_by_context
                .get(&(candidate.target_locator.clone(), candidate.context.clone()))
                .copied()
                .unwrap_or(0)
                < budget.max_repeated_failures
            && self
                .attempts_by_candidate
                .get(&(
                    candidate.target_locator.clone(),
                    candidate.context.clone(),
                    candidate.operation.clone(),
                ))
                .copied()
                .unwrap_or(0)
                < budget.max_retries_per_candidate
    }

    pub fn record(&mut self, candidate: &Affordance, effect: Effect) {
        *self
            .attempted_by_context
            .entry((candidate.target_locator.clone(), candidate.context.clone()))
            .or_default() += 1;
        *self
            .attempts_by_candidate
            .entry((
                candidate.target_locator.clone(),
                candidate.context.clone(),
                candidate.operation.clone(),
            ))
            .or_default() += 1;
        if !effect.is_verified_relevant() {
            *self
                .failures_by_context
                .entry((candidate.target_locator.clone(), candidate.context.clone()))
                .or_default() += 1;
        }
    }
}

impl Default for ExplorationPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            environment: ExplorationEnvironment::Ordinary,
            operation_budget: 0,
        }
    }
}

impl ExplorationPolicy {
    pub fn allows(
        self,
        candidate: &Affordance,
        fresh_authority: bool,
        strong_verifier: bool,
        operations_used: u32,
    ) -> bool {
        self.enabled
            && matches!(
                self.environment,
                ExplorationEnvironment::Fixture | ExplorationEnvironment::DisposableSession
            )
            && operations_used < self.operation_budget
            && fresh_authority
            && strong_verifier
            && matches!(
                candidate.risk,
                RiskClass::Observational | RiskClass::ReversibleNavigation
            )
    }
}

pub fn format_capability_graph(graph: &CapabilityGraph) -> String {
    let mut output = format!(
        "CapabilityGraph generation={} objects={} declared={} observed={} hypothesized={} rejected={} effects={} build_us={} estimated_bytes={}\n",
        graph.generation.0,
        graph.objects.metrics.object_count,
        graph.declared.len(),
        graph.observed.len(),
        graph.hypothesized.len(),
        graph.rejected.len(),
        graph.effects.len(),
        graph.objects.metrics.graph_build_micros,
        graph.objects.metrics.estimated_bytes,
    );
    for object in graph.objects.objects() {
        output.push_str(&format!(
            "Object runtime={} locator={} generation={} scope={} role={} states={:?} interfaces={:?} parent={:?} children={} region={:?}\n",
            object.runtime_id,
            object.backend_locator,
            object.application_generation.0,
            object.interaction_scope,
            object.role,
            object.states,
            object.interfaces,
            object.parent.map(RuntimeNodeId::get),
            object.children.len(),
            object.semantic_region.map(RuntimeNodeId::get),
        ));
        for capability in graph
            .declared
            .iter()
            .filter(|capability| capability.target == object.runtime_id)
        {
            output.push_str(&format!(
                "  Declared operation={} evidence={:?} confidence={:.3} risk={:?} verification={:?}\n",
                capability.operation,
                capability.evidence,
                f32::from(capability.confidence_milli) / 1000.0,
                capability.risk,
                capability.verification,
            ));
        }
        for affordance in graph
            .observed
            .iter()
            .filter(|affordance| affordance.target == object.runtime_id)
        {
            output.push_str(&format_affordance("Observed", affordance));
        }
        for affordance in graph
            .hypothesized
            .iter()
            .filter(|affordance| affordance.target == object.runtime_id)
        {
            output.push_str(&format_affordance("Hypothesized", affordance));
        }
        for rejected in graph
            .rejected
            .iter()
            .filter(|candidate| candidate.target_locator == object.backend_locator)
        {
            output.push_str(&format!(
                "  Rejected operation={} effect={:?} failures={}\n",
                rejected.operation, rejected.effect, rejected.failure_count
            ));
        }
    }
    for claim in &graph.self_description.claims {
        output.push_str(&format!(
            "Described source={} origin={:?} status={:?} description={:?} primitive={:?} enabled={} target_grounded={} risk={:?} verifier={:?} confidence_basis={:?}\n",
            claim.source, claim.origin, claim.status, claim.description, claim.shortcut,
            claim.enabled, claim.target_grounded, claim.risk, claim.verification, claim.confidence_basis,
        ));
    }
    output
}

fn format_affordance(label: &str, affordance: &Affordance) -> String {
    format!(
        "  {label} operation={} lifecycle={:?} effect={:?} provenance={:?} evidence={:?} confidence={:.3} risk={:?} verification={:?} history={}/{}\n",
        affordance.operation,
        affordance.lifecycle,
        affordance.effect,
        affordance.provenance,
        affordance.evidence,
        f32::from(affordance.confidence_milli) / 1000.0,
        affordance.risk,
        affordance.verification,
        affordance.success_count,
        affordance.failure_count,
    )
}

fn build_declared_capabilities(
    session: &RuntimeSessionId,
    objects: &RuntimeObjectModel,
) -> Vec<DeclaredCapability> {
    let mut capabilities = Vec::new();
    for object in objects.objects() {
        let validity = ValidityScope {
            session: session.clone(),
            generation: object.application_generation,
            interaction_scope: object.interaction_scope,
        };
        for action in &object.advertised_actions {
            if action.name.trim().is_empty() {
                continue;
            }
            capabilities.push(DeclaredCapability {
                target: object.runtime_id,
                operation: CapabilityOperation::AccessibilityInvoke(action.name.clone()),
                preconditions: vec!["fresh advertised action remains present".to_owned()],
                delivery_route: DeliveryRoute::Accessibility,
                evidence: vec![Evidence {
                    kind: EvidenceKind::Declared,
                    detail: format!("advertised Accessibility action {:?}", action.name),
                }],
                expected_effects: vec![Effect::UnknownEffect],
                verification: VerificationPlan::RelevantSurfaceObservation,
                confidence_milli: 1000,
                risk: RiskClass::Unknown,
                validity: validity.clone(),
            });
        }
        for (raw, primitive) in &object.advertised_keybindings {
            capabilities.push(DeclaredCapability {
                target: object.runtime_id,
                operation: CapabilityOperation::Keyboard(*primitive),
                preconditions: physical_preconditions(&CapabilityOperation::Keyboard(*primitive)),
                delivery_route: DeliveryRoute::NativeKeyboard,
                evidence: vec![Evidence {
                    kind: EvidenceKind::Declared,
                    detail: format!("advertised keybinding {raw:?}"),
                }],
                expected_effects: vec![Effect::UnknownEffect],
                verification: VerificationPlan::RelevantSurfaceObservation,
                confidence_milli: 950,
                risk: RiskClass::Unknown,
                validity: validity.clone(),
            });
        }
        if has_state(&object.states, "focusable") {
            capabilities.push(DeclaredCapability {
                target: object.runtime_id,
                operation: CapabilityOperation::Focus,
                preconditions: vec!["target remains focusable in current scope".to_owned()],
                delivery_route: DeliveryRoute::Accessibility,
                evidence: vec![Evidence {
                    kind: EvidenceKind::Structural,
                    detail: "current public focusable state".to_owned(),
                }],
                expected_effects: vec![Effect::FocusChanged],
                verification: VerificationPlan::FocusReadback,
                confidence_milli: 850,
                risk: RiskClass::ReversibleNavigation,
                validity: validity.clone(),
            });
        }
        for capability in &object.semantic_capabilities {
            let (operation, verification, effect, risk) = match capability {
                SemanticCapability::Activate => continue,
                SemanticCapability::SelectChildren
                | SemanticCapability::SelectCurrentChild
                | SemanticCapability::SelectCurrentTableRow => (
                    CapabilityOperation::SetSelection,
                    VerificationPlan::SelectionReadback,
                    Effect::SelectionChanged,
                    RiskClass::LocalMutation,
                ),
                SemanticCapability::EditText | SemanticCapability::EditComplexText => (
                    CapabilityOperation::SetText,
                    VerificationPlan::TextReadback,
                    Effect::TextChanged,
                    RiskClass::LocalMutation,
                ),
                SemanticCapability::Value => (
                    CapabilityOperation::SetValue,
                    VerificationPlan::ValueReadback,
                    Effect::ValueChanged,
                    RiskClass::LocalMutation,
                ),
            };
            if capabilities
                .iter()
                .any(|known| known.target == object.runtime_id && known.operation == operation)
            {
                continue;
            }
            capabilities.push(DeclaredCapability {
                target: object.runtime_id,
                operation,
                preconditions: vec![format!("{capability:?} remains publicly exposed")],
                delivery_route: DeliveryRoute::Accessibility,
                evidence: vec![Evidence {
                    kind: EvidenceKind::Declared,
                    detail: format!("public semantic capability {capability:?}"),
                }],
                expected_effects: vec![effect],
                verification,
                confidence_milli: 1000,
                risk,
                validity: validity.clone(),
            });
        }
    }
    capabilities
}

fn operation_contract(operation: &CapabilityOperation) -> (DeliveryRoute, VerificationPlan) {
    match operation {
        CapabilityOperation::AccessibilityInvoke(_) => (
            DeliveryRoute::Accessibility,
            VerificationPlan::RelevantSurfaceObservation,
        ),
        CapabilityOperation::SetText => {
            (DeliveryRoute::Accessibility, VerificationPlan::TextReadback)
        }
        CapabilityOperation::SetValue => (
            DeliveryRoute::Accessibility,
            VerificationPlan::ValueReadback,
        ),
        CapabilityOperation::SetSelection => (
            DeliveryRoute::Accessibility,
            VerificationPlan::SelectionReadback,
        ),
        CapabilityOperation::Focus => (
            DeliveryRoute::Accessibility,
            VerificationPlan::FocusReadback,
        ),
        CapabilityOperation::Keyboard(_) => (
            DeliveryRoute::NativeKeyboard,
            VerificationPlan::RelevantSurfaceObservation,
        ),
        CapabilityOperation::TextInput => {
            (DeliveryRoute::NativeText, VerificationPlan::TextReadback)
        }
    }
}

fn risk_for(operation: &CapabilityOperation, effect: Option<Effect>) -> RiskClass {
    if matches!(effect, Some(Effect::ExternalEffectObserved)) {
        return RiskClass::ExternalEffect;
    }
    match operation {
        CapabilityOperation::Focus => RiskClass::ReversibleNavigation,
        CapabilityOperation::SetText
        | CapabilityOperation::SetValue
        | CapabilityOperation::SetSelection
        | CapabilityOperation::TextInput => RiskClass::LocalMutation,
        CapabilityOperation::AccessibilityInvoke(_) => RiskClass::Unknown,
        CapabilityOperation::Keyboard(_) => match effect {
            Some(Effect::FocusChanged | Effect::ExpandedChanged | Effect::SelectionChanged) => {
                // One visible navigation effect cannot rule out hidden side
                // effects of an arbitrary key in a new context.
                RiskClass::Unknown
            }
            Some(
                Effect::TextChanged
                | Effect::ValueChanged
                | Effect::StateChanged
                | Effect::StructureChanged
                | Effect::DocumentChanged
                | Effect::WindowChanged
                | Effect::DialogOpened
                | Effect::DialogClosed
                | Effect::TargetBecameStale,
            ) => RiskClass::LocalMutation,
            _ => RiskClass::Unknown,
        },
    }
}

fn candidate_rank_key(
    candidate: &Affordance,
) -> (u8, u8, std::cmp::Reverse<u16>, CapabilityOperation) {
    let risk = match candidate.risk {
        RiskClass::Observational => 0,
        RiskClass::ReversibleNavigation => 1,
        RiskClass::LocalMutation => 2,
        RiskClass::ExternalEffect => 3,
        RiskClass::PotentiallyDestructive => 4,
        RiskClass::Unknown => 5,
    };
    let verifier = match candidate.verification {
        VerificationPlan::FocusReadback
        | VerificationPlan::TextReadback
        | VerificationPlan::ValueReadback
        | VerificationPlan::SelectionReadback => 0,
        VerificationPlan::RelevantSurfaceObservation => 1,
        VerificationPlan::ExternalObserver => 2,
        VerificationPlan::Unavailable => 3,
    };
    (
        risk,
        verifier,
        std::cmp::Reverse(candidate.confidence_milli),
        candidate.operation.clone(),
    )
}

fn physical_preconditions(operation: &CapabilityOperation) -> Vec<String> {
    if matches!(
        operation,
        CapabilityOperation::Keyboard(_) | CapabilityOperation::TextInput
    ) {
        vec![
            "current runtime session and application generation".to_owned(),
            "current interaction scope".to_owned(),
            "fresh exact target and owning window".to_owned(),
            "exact Accessibility focus verified immediately before delivery".to_owned(),
            "current operation ticket".to_owned(),
        ]
    } else {
        Vec::new()
    }
}

fn observed_confidence(successes: u32, failures: u32) -> u16 {
    let total = successes.saturating_add(failures).max(1);
    let ratio = successes.saturating_mul(1000) / total;
    (600_u32
        .saturating_add(successes.min(5).saturating_mul(70))
        .saturating_mul(ratio)
        / 1000)
        .min(980) as u16
}

fn hypothesized_confidence(
    similarity: u16,
    successes: u32,
    failures: u32,
    direct_successes: u32,
    direct_failures: u32,
) -> u16 {
    let history_total = successes.saturating_add(failures).max(1);
    let reliability = successes.saturating_mul(1000) / history_total;
    let base = u32::from(similarity)
        .saturating_mul(reliability)
        .saturating_mul(3)
        / 4000;
    let direct_bonus = direct_successes.min(3).saturating_mul(80);
    let direct_penalty = direct_failures.min(3).saturating_mul(220);
    base.saturating_add(direct_bonus)
        .saturating_sub(direct_penalty)
        .min(950) as u16
}

fn has_state(states: &[SemanticState], expected: &str) -> bool {
    states.iter().any(
        |state| matches!(state, SemanticState::Other(value) if value.eq_ignore_ascii_case(expected)),
    )
}

fn important_states(states: &[SemanticState]) -> Vec<String> {
    const IMPORTANT: &[&str] = &[
        "active",
        "enabled",
        "expandable",
        "expanded",
        "focusable",
        "focused",
        "modal",
        "multiselectable",
        "selectable",
        "showing",
        "visible",
    ];
    sorted_unique(
        states
            .iter()
            .filter_map(|state| {
                let value = state.to_string().to_ascii_lowercase();
                IMPORTANT.contains(&value.as_str()).then_some(value)
            })
            .collect(),
    )
}

fn nearest_semantic_region(cache: &SemanticCache, mut id: RuntimeNodeId) -> Option<RuntimeNodeId> {
    loop {
        let node = cache.node(id)?;
        if matches!(
            node.role,
            SemanticRole::Window
                | SemanticRole::Dialog
                | SemanticRole::Toolbar
                | SemanticRole::Navigation
                | SemanticRole::Search
                | SemanticRole::Main
                | SemanticRole::Section
                | SemanticRole::Article
                | SemanticRole::Sidebar
                | SemanticRole::Footer
                | SemanticRole::Form
                | SemanticRole::Document
        ) {
            return Some(id);
        }
        id = node.parent?;
    }
}

fn sorted_unique<T: Ord>(mut values: Vec<T>) -> Vec<T> {
    values.sort();
    values.dedup();
    values
}

fn scaled_jaccard(left: &[String], right: &[String], weight: u16) -> u16 {
    if left.is_empty() && right.is_empty() {
        return 0;
    }
    let left: BTreeSet<_> = left.iter().collect();
    let right: BTreeSet<_> = right.iter().collect();
    let union = left.union(&right).count();
    if union == 0 {
        return weight;
    }
    let intersection = left.intersection(&right).count();
    (intersection * usize::from(weight) / union) as u16
}

fn materially_different_nonempty_sets(left: &[String], right: &[String]) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let left: BTreeSet<_> = left.iter().collect();
    let right: BTreeSet<_> = right.iter().collect();
    let union = left.union(&right).count();
    let intersection = left.intersection(&right).count();
    intersection.saturating_mul(2) < union
}

fn sequence_similarity(left: &[String], right: &[String], weight: u16) -> u16 {
    if left.is_empty() && right.is_empty() {
        return 0;
    }
    let length = left.len().max(right.len());
    if length == 0 {
        return weight;
    }
    let matches = left
        .iter()
        .zip(right)
        .filter(|(left, right)| left == right)
        .count();
    (matches * usize::from(weight) / length) as u16
}

fn estimate_object_bytes(object: &RuntimeObject) -> usize {
    std::mem::size_of::<RuntimeObject>()
        + object.name.as_ref().map_or(0, String::len)
        + object.description.as_ref().map_or(0, String::len)
        + object.interfaces.iter().map(String::len).sum::<usize>()
        + object.children.len() * std::mem::size_of::<RuntimeNodeId>()
        + object
            .advertised_actions
            .iter()
            .map(|action| action.name.len() + action.description.as_ref().map_or(0, String::len))
            .sum::<usize>()
}

fn micros(value: u128) -> u64 {
    value.min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::{DebugInfo, RelationalSemanticGraph, SemanticNode};

    fn node(path: &str, role: SemanticRole) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.9", path),
            index_in_parent: None,
            role,
            name: Some(path.to_owned()),
            description: None,
            value: None,
            text_input_kind: None,
            states: vec![
                SemanticState::Enabled,
                SemanticState::Other("showing".to_owned()),
                SemanticState::Other("visible".to_owned()),
            ],
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        }
    }

    fn graph(root: SemanticNode) -> CapabilityGraph {
        let cache = SemanticCache::from_snapshot(root).unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        CapabilityGraph::new(
            RuntimeSessionId::default(),
            ApplicationGenerationId(1),
            &cache,
            &scopes,
        )
    }

    fn target_named(graph: &CapabilityGraph, name: &str) -> RuntimeNodeId {
        graph
            .objects()
            .objects()
            .find(|object| object.name.as_deref() == Some(name))
            .unwrap()
            .runtime_id
    }

    #[test]
    fn advertised_keybindings_are_strong_sparse_candidates() {
        let mut root = node("/app", SemanticRole::Application);
        let mut window = node("/window", SemanticRole::Window);
        let mut control = node("/control", SemanticRole::Unknown("generic".to_owned()));
        control
            .states
            .push(SemanticState::Other("focusable".to_owned()));
        control.actions.push(SemanticAction {
            index: 0,
            name: "perform".to_owned(),
            description: None,
            keybinding: Some("Ctrl+K".to_owned()),
        });
        window.children.push(control);
        root.children.push(window);
        let mut graph = graph(root);
        let target = target_named(&graph, "/control");
        let candidates = graph.synthesize_candidates(target);
        assert!(candidates.iter().any(|candidate| {
            candidate.operation == CapabilityOperation::Keyboard(KeyboardPrimitive::control('k'))
                && candidate
                    .evidence
                    .iter()
                    .any(|evidence| evidence.kind == EvidenceKind::Declared)
        }));
        assert!(!candidates.iter().any(|candidate| {
            candidate.operation == CapabilityOperation::Keyboard(KeyboardPrimitive::SPACE)
        }));
        assert!(graph.last_candidate_metrics.retained_candidates < 8);
    }

    #[test]
    fn password_metadata_is_redacted_in_runtime_object_model() {
        let mut root = node("/app", SemanticRole::Application);
        let mut password = node("/password", SemanticRole::TextInput);
        password.text_input_kind = Some(TextInputKind::Password);
        password.value = Some("do-not-retain".to_owned());
        root.children.push(password);
        let graph = graph(root);
        let target = target_named(&graph, "/password");
        let metadata = graph
            .objects()
            .object(target)
            .unwrap()
            .text
            .as_ref()
            .unwrap();
        assert!(!metadata.value_available);
        assert_eq!(metadata.character_count, None);
        assert!(!format_capability_graph(&graph).contains("do-not-retain"));
    }

    #[test]
    fn same_role_alone_cannot_generalize_a_keyboard_affordance() {
        let mut root = node("/app", SemanticRole::Application);
        let mut window = node("/window", SemanticRole::Window);
        let mut a = node("/a", SemanticRole::ComboBox);
        a.states.push(SemanticState::Other("focusable".to_owned()));
        a.debug.interfaces = vec!["EditableText".to_owned(), "Text".to_owned()];
        let mut b = node("/b", SemanticRole::ComboBox);
        b.states.push(SemanticState::Other("focusable".to_owned()));
        b.debug.interfaces = vec!["Selection".to_owned()];
        window.children.extend([a, b]);
        root.children.push(window);
        let mut graph = graph(root);
        let a = target_named(&graph, "/a");
        let b = target_named(&graph, "/b");
        assert!(
            graph
                .record_observation(
                    a,
                    CapabilityOperation::Keyboard(KeyboardPrimitive::ENTER),
                    Effect::DocumentChanged,
                )
                .is_some()
        );
        let candidates = graph.synthesize_candidates(b);
        assert!(!candidates.iter().any(|candidate| {
            candidate.operation == CapabilityOperation::Keyboard(KeyboardPrimitive::ENTER)
        }));
    }

    #[test]
    fn same_role_distinct_key_fixture_uses_structural_context_not_a_role_table() {
        let mut root = node("/app", SemanticRole::Application);
        let mut window = node("/window", SemanticRole::Window);
        for (path, interface) in [
            ("/enter", "EnterContract"),
            ("/space", "SpaceContract"),
            ("/ctrl-k", "ChordContract"),
        ] {
            let mut control = node(path, SemanticRole::Unknown("generic".to_owned()));
            control
                .states
                .push(SemanticState::Other("focusable".to_owned()));
            control.debug.interfaces = vec!["Component".to_owned(), interface.to_owned()];
            window.children.push(control);
        }
        root.children.push(window);
        let mut graph = graph(root);
        let fixtures = [
            ("/enter", KeyboardPrimitive::ENTER),
            ("/space", KeyboardPrimitive::SPACE),
            ("/ctrl-k", KeyboardPrimitive::control('k')),
        ];
        for (path, primitive) in fixtures {
            let target = target_named(&graph, path);
            graph.record_observation(
                target,
                CapabilityOperation::Keyboard(primitive),
                Effect::StateChanged,
            );
        }
        for (path, expected) in fixtures {
            let target = target_named(&graph, path);
            let physical: Vec<_> = graph
                .synthesize_candidates(target)
                .into_iter()
                .filter_map(|candidate| match candidate.operation {
                    CapabilityOperation::Keyboard(primitive) => Some(primitive),
                    _ => None,
                })
                .collect();
            assert!(physical.contains(&expected));
            assert_eq!(physical.len(), 1);
        }
    }

    #[test]
    fn different_roles_can_share_one_observed_physical_affordance() {
        let mut root = node("/app", SemanticRole::Application);
        let mut window = node("/window", SemanticRole::Window);
        let mut entry = node("/entry", SemanticRole::TextInput);
        entry
            .states
            .push(SemanticState::Other("focusable".to_owned()));
        entry.debug.interfaces = vec!["Component".to_owned()];
        let mut button = node("/button", SemanticRole::Button);
        button
            .states
            .push(SemanticState::Other("focusable".to_owned()));
        button.debug.interfaces = vec!["Component".to_owned()];
        window.children.extend([entry, button]);
        root.children.push(window);
        let mut graph = graph(root);
        let operation = CapabilityOperation::Keyboard(KeyboardPrimitive::control('k'));
        for path in ["/entry", "/button"] {
            let target = target_named(&graph, path);
            assert!(
                graph
                    .record_observation(target, operation.clone(), Effect::StateChanged)
                    .is_some()
            );
        }
        assert_eq!(
            graph
                .observed_affordances()
                .iter()
                .filter(|affordance| affordance.operation == operation)
                .count(),
            2
        );
    }

    #[test]
    fn unseen_affordance_is_recovered_across_runtime_identity_change() {
        let make_root = |path: &str| {
            let mut root = node("/app", SemanticRole::Application);
            let mut window = node("/window", SemanticRole::Window);
            let mut control = node(path, SemanticRole::Unknown("generic-focusable".to_owned()));
            control
                .states
                .push(SemanticState::Other("focusable".to_owned()));
            control.debug.interfaces = vec!["Component".to_owned(), "Action".to_owned()];
            window.children.push(control);
            root.children.push(window);
            root
        };
        let first_cache = SemanticCache::from_snapshot(make_root("/first")).unwrap();
        let first_scopes =
            InteractionScopes::analyze(&first_cache, &RelationalSemanticGraph::new(&first_cache));
        let mut graph = CapabilityGraph::new(
            RuntimeSessionId::default(),
            ApplicationGenerationId(1),
            &first_cache,
            &first_scopes,
        );
        let first = target_named(&graph, "/first");
        assert!(!graph.synthesize_candidates(first).iter().any(|candidate| {
            candidate.operation == CapabilityOperation::Keyboard(KeyboardPrimitive::control('K'))
        }));
        let observed = graph
            .record_observation(
                first,
                CapabilityOperation::Keyboard(KeyboardPrimitive::control('K')),
                Effect::DocumentChanged,
            )
            .unwrap()
            .confidence_milli;

        let second_cache = SemanticCache::from_snapshot(make_root("/second")).unwrap();
        let second_scopes =
            InteractionScopes::analyze(&second_cache, &RelationalSemanticGraph::new(&second_cache));
        graph.refresh(ApplicationGenerationId(1), &second_cache, &second_scopes);
        let second = target_named(&graph, "/second");
        assert_ne!(first, second);
        let hypothesis = graph
            .synthesize_candidates(second)
            .into_iter()
            .find(|candidate| {
                candidate.operation
                    == CapabilityOperation::Keyboard(KeyboardPrimitive::control('K'))
            })
            .expect("similar context should receive a bounded hypothesis");
        assert!(
            hypothesis
                .evidence
                .iter()
                .any(|evidence| evidence.kind == EvidenceKind::Hypothesized)
        );
        let after = graph
            .record_observation(second, hypothesis.operation, Effect::DocumentChanged)
            .unwrap()
            .confidence_milli;
        assert!(after > observed);
    }

    #[test]
    fn background_churn_or_no_relevant_effect_is_not_learned() {
        let mut root = node("/app", SemanticRole::Application);
        let mut control = node("/control", SemanticRole::Button);
        control
            .states
            .push(SemanticState::Other("focusable".to_owned()));
        root.children.push(control);
        let mut graph = graph(root);
        let target = target_named(&graph, "/control");
        assert!(
            graph
                .record_observation(
                    target,
                    CapabilityOperation::Keyboard(KeyboardPrimitive::ENTER),
                    Effect::NoRelevantEffect,
                )
                .is_none()
        );
        assert!(graph.observed_affordances().is_empty());
        assert!(
            !graph.synthesize_candidates(target).iter().any(|candidate| {
                candidate.operation == CapabilityOperation::Keyboard(KeyboardPrimitive::ENTER)
            })
        );
    }

    #[test]
    fn direct_failure_localizes_confidence_without_global_role_mapping() {
        let mut root = node("/app", SemanticRole::Application);
        let mut window = node("/window", SemanticRole::Window);
        for path in ["/a", "/b"] {
            let mut control = node(path, SemanticRole::ComboBox);
            control
                .states
                .push(SemanticState::Other("focusable".to_owned()));
            control.debug.interfaces = vec!["Component".to_owned(), "Text".to_owned()];
            window.children.push(control);
        }
        root.children.push(window);
        let mut graph = graph(root);
        let a = target_named(&graph, "/a");
        let b = target_named(&graph, "/b");
        let operation = CapabilityOperation::Keyboard(KeyboardPrimitive::ENTER);
        graph.record_observation(a, operation.clone(), Effect::DocumentChanged);
        assert!(
            graph
                .synthesize_candidates(b)
                .iter()
                .any(|candidate| candidate.operation == operation)
        );
        graph.record_observation(b, operation.clone(), Effect::NoRelevantEffect);
        assert!(
            !graph
                .synthesize_candidates(b)
                .iter()
                .any(|candidate| candidate.operation == operation)
        );
        assert!(
            graph
                .synthesize_candidates(a)
                .iter()
                .any(|candidate| candidate.operation == operation)
        );
    }

    #[test]
    fn self_description_is_unverified_bounded_and_excludes_passwords() {
        use self_description::{DescriptionStatus, MAX_SHORTCUTS_PER_TEXT};
        let mut root = node("/app", SemanticRole::Application);
        let mut help = node("/help", SemanticRole::Label);
        help.name =
            Some("Example only Ctrl+Q; Export Ctrl+Shift+E; requires an image Alt+I".to_owned());
        let mut disabled = node("/disabled", SemanticRole::MenuItem);
        disabled.name = Some("Unavailable Ctrl+D".to_owned());
        disabled.states.retain(|s| *s != SemanticState::Enabled);
        let mut secret = node("/password", SemanticRole::TextInput);
        secret.text_input_kind = Some(TextInputKind::Password);
        secret.description = Some("private Ctrl+X".to_owned());
        root.children.extend([help, disabled, secret]);
        let mut graph = graph(root);
        assert!(graph.self_descriptions().claims.is_empty());
        let catalog = graph.acquire_self_descriptions();
        assert_eq!(catalog.claims.len(), 4);
        assert_eq!(
            catalog
                .claims
                .iter()
                .filter(|c| c.status == DescriptionStatus::Rejected)
                .count(),
            1
        );
        assert!(
            catalog
                .claims
                .iter()
                .all(|c| !c.target_grounded && c.verification == VerificationPlan::Unavailable)
        );
        assert!(
            catalog
                .claims
                .iter()
                .any(|c| c.risk == RiskClass::ExternalEffect)
        );
        assert!(graph.observed_affordances().is_empty());
        assert!(!format_capability_graph(&graph).contains("private Ctrl+X"));
        let dense = (0..100)
            .map(|i| format!("Ctrl+{} ", char::from(b'a' + (i % 26) as u8)))
            .collect::<String>();
        assert!(self_description::extract_shortcuts(&dense).len() <= MAX_SHORTCUTS_PER_TEXT);
    }

    #[test]
    fn observation_survives_snapshot_id_change_only_for_exact_locator() {
        let make_root = |path: &str| {
            let mut root = node("/app", SemanticRole::Application);
            root.children.push(node(path, SemanticRole::TextInput));
            root
        };
        let mut graph = graph(make_root("/target"));
        let old = target_named(&graph, "/target");
        graph.record_observation(old, CapabilityOperation::SetText, Effect::TextChanged);
        let cache = SemanticCache::from_snapshot(make_root("/target")).unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        graph.refresh(ApplicationGenerationId(1), &cache, &scopes);
        let current = target_named(&graph, "/target");
        assert_ne!(old, current);
        assert_eq!(graph.observed_affordances().len(), 1);
        assert_eq!(graph.observed_affordances()[0].target, current);
        let cache = SemanticCache::from_snapshot(make_root("/different")).unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        graph.refresh(ApplicationGenerationId(1), &cache, &scopes);
        assert!(graph.observed_affordances().is_empty());
    }

    #[test]
    fn generation_change_discards_learned_affordance_and_stale_targets() {
        let make_root = |path: &str| {
            let mut root = node("/app", SemanticRole::Application);
            let mut target = node(path, SemanticRole::Unknown("generic".to_owned()));
            target
                .states
                .push(SemanticState::Other("focusable".to_owned()));
            target.debug.interfaces = vec!["Component".to_owned(), "Action".to_owned()];
            root.children.push(target);
            root
        };
        let (first_cache, first_scopes) = {
            let cache = SemanticCache::from_snapshot(make_root("/old")).unwrap();
            let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
            (cache, scopes)
        };
        let mut graph = CapabilityGraph::new(
            RuntimeSessionId::default(),
            ApplicationGenerationId(1),
            &first_cache,
            &first_scopes,
        );
        let old = target_named(&graph, "/old");
        graph.record_observation(
            old,
            CapabilityOperation::Keyboard(KeyboardPrimitive::ENTER),
            Effect::DocumentChanged,
        );
        let next_cache = SemanticCache::from_snapshot(make_root("/new")).unwrap();
        let next_scopes =
            InteractionScopes::analyze(&next_cache, &RelationalSemanticGraph::new(&next_cache));
        graph.refresh(ApplicationGenerationId(2), &next_cache, &next_scopes);
        let new = target_named(&graph, "/new");
        assert!(graph.observed_affordances().is_empty());
        assert!(!graph.synthesize_candidates(new).iter().any(|candidate| {
            candidate.operation == CapabilityOperation::Keyboard(KeyboardPrimitive::ENTER)
        }));
    }

    #[test]
    fn human_projection_never_exposes_learned_physical_affordances() {
        let mut root = node("/app", SemanticRole::Application);
        let mut input = node("/input", SemanticRole::TextInput);
        input.capabilities.push(SemanticCapability::EditText);
        input
            .states
            .push(SemanticState::Other("focusable".to_owned()));
        root.children.push(input);
        let mut graph = graph(root);
        let target = target_named(&graph, "/input");
        graph.record_observation(
            target,
            CapabilityOperation::Keyboard(KeyboardPrimitive::ENTER),
            Effect::DocumentChanged,
        );
        assert!(graph.register_human_capability(target, HumanCapability::EditText));
        assert!(graph.has_human_capability(target, HumanCapability::EditText));
        assert!(graph.observed_affordances().iter().any(|affordance| {
            affordance.operation == CapabilityOperation::Keyboard(KeyboardPrimitive::ENTER)
        }));
    }

    #[test]
    fn active_exploration_is_off_by_default_and_risk_bounded() {
        let mut root = node("/app", SemanticRole::Application);
        let mut control = node("/control", SemanticRole::Button);
        control
            .states
            .push(SemanticState::Other("focusable".to_owned()));
        root.children.push(control);
        let mut graph = graph(root);
        let target = target_named(&graph, "/control");
        graph.record_observation(target, CapabilityOperation::Focus, Effect::FocusChanged);
        let candidate = graph
            .synthesize_candidates(target)
            .into_iter()
            .find(|candidate| candidate.operation == CapabilityOperation::Focus)
            .unwrap();
        assert!(!ExplorationPolicy::default().allows(&candidate, true, true, 0));
        let enabled = ExplorationPolicy {
            enabled: true,
            environment: ExplorationEnvironment::Fixture,
            operation_budget: 1,
        };
        assert!(enabled.allows(&candidate, true, true, 0));
        assert!(!enabled.allows(&candidate, true, true, 1));
    }

    #[test]
    fn gap_taxonomy_is_deterministic() {
        assert_eq!(
            classify_gap(GapEvidence {
                visible_in_gui: true,
                ..Default::default()
            }),
            CapabilityGap::ObservationGap
        );
        assert_eq!(
            classify_gap(GapEvidence {
                visible_in_gui: true,
                accessibility_object: true,
                addressable: true,
                ..Default::default()
            }),
            CapabilityGap::ActuationGap
        );
        assert_eq!(
            classify_gap(GapEvidence {
                opaque_region: true,
                ..Default::default()
            }),
            CapabilityGap::OpaqueRegionGap
        );
    }

    #[test]
    fn compound_public_bindings_are_parsed_without_guessing() {
        assert_eq!(
            parse_advertised_keybinding("<Control>k"),
            Some(KeyboardPrimitive::control('k'))
        );
        assert_eq!(
            parse_advertised_keybinding("<Return>"),
            Some(KeyboardPrimitive::ENTER)
        );
        assert!(parse_advertised_keybinding("Ctrl+k+q").is_none());
        let mut control = node("/control", SemanticRole::Button);
        control.actions.push(SemanticAction {
            index: 7,
            name: "activate".into(),
            description: None,
            keybinding: Some("<Alt>d;;<Control>k".into()),
        });
        let mut graph = graph(control);
        let target = target_named(&graph, "/control");
        let candidate = graph
            .synthesize_candidates(target)
            .into_iter()
            .find(|candidate| {
                candidate.operation
                    == CapabilityOperation::Keyboard(KeyboardPrimitive::control('k'))
            })
            .unwrap();
        assert_eq!(
            candidate.provenance.source,
            CandidateSource::AdvertisedKeybinding
        );
        graph.record_observation(
            target,
            candidate.operation.clone(),
            Effect::NoRelevantEffect,
        );
        assert!(
            !graph
                .synthesize_candidates(target)
                .iter()
                .any(|other| other.operation == candidate.operation)
        );
        assert_eq!(graph.rejected_candidates().len(), 1);
    }

    #[test]
    fn exploration_budget_is_target_local_and_blocks_repeat_attempts() {
        let mut control = node("/control", SemanticRole::Button);
        control
            .states
            .push(SemanticState::Other("focusable".into()));
        let mut graph = graph(control);
        let target = target_named(&graph, "/control");
        let candidate = graph
            .synthesize_candidates(target)
            .into_iter()
            .find(|candidate| candidate.operation == CapabilityOperation::Focus)
            .unwrap();
        let mut ledger = ExplorationLedger::default();
        let budget = ExplorationBudget::for_candidate_count(1);
        assert!(ledger.allows(&candidate, budget));
        ledger.record(&candidate, Effect::NoRelevantEffect);
        assert!(!ledger.allows(&candidate, budget));
        let mut different_target = candidate;
        different_target.target_locator = BackendLocator::new(":1.9", "/other");
        assert!(ledger.allows(&different_target, budget));
    }
}
