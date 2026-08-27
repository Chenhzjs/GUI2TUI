use std::fmt;

use atspi::{Role, State};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use thiserror::Error;

const NODE_ID_PREFIX: &str = "atspi1_";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    bus_name: String,
    object_path: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NodeIdError {
    #[error("node ID must start with '{NODE_ID_PREFIX}'")]
    MissingPrefix,
    #[error("node ID payload is not valid URL-safe Base64")]
    InvalidBase64,
    #[error("node ID payload is not valid UTF-8")]
    InvalidUtf8,
    #[error("node ID payload does not contain a bus name and object path")]
    MissingSeparator,
    #[error("node ID contains an invalid AT-SPI unique bus name: {0}")]
    InvalidBusName(String),
    #[error("node ID contains an invalid D-Bus object path: {0}")]
    InvalidObjectPath(String),
}

impl NodeId {
    pub fn new(bus_name: impl Into<String>, object_path: impl Into<String>) -> Self {
        Self {
            bus_name: bus_name.into(),
            object_path: object_path.into(),
        }
    }

    pub fn bus_name(&self) -> &str {
        &self.bus_name
    }

    pub fn object_path(&self) -> &str {
        &self.object_path
    }

    pub fn encode(&self) -> String {
        let raw = format!("{}\0{}", self.bus_name, self.object_path);
        format!("{NODE_ID_PREFIX}{}", URL_SAFE_NO_PAD.encode(raw))
    }

    pub fn decode(encoded: &str) -> Result<Self, NodeIdError> {
        let payload = encoded
            .strip_prefix(NODE_ID_PREFIX)
            .ok_or(NodeIdError::MissingPrefix)?;
        let raw = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| NodeIdError::InvalidBase64)?;
        let raw = String::from_utf8(raw).map_err(|_| NodeIdError::InvalidUtf8)?;
        let (bus_name, object_path) = raw.split_once('\0').ok_or(NodeIdError::MissingSeparator)?;

        zbus::names::UniqueName::try_from(bus_name)
            .map_err(|_| NodeIdError::InvalidBusName(bus_name.to_owned()))?;
        zbus::zvariant::ObjectPath::try_from(object_path)
            .map_err(|_| NodeIdError::InvalidObjectPath(object_path.to_owned()))?;

        Ok(Self::new(bus_name, object_path))
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.encode())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticRole {
    Application,
    Window,
    Dialog,
    Container,
    Label,
    Button,
    ToggleButton,
    CheckBox,
    RadioButton,
    Text,
    TextInput,
    MenuBar,
    Menu,
    MenuItem,
    List,
    ListItem,
    Tab,
    TabList,
    Tree,
    TreeItem,
    Table,
    Row,
    Cell,
    Slider,
    ProgressBar,
    StatusBar,
    Unknown(String),
}

impl From<Role> for SemanticRole {
    fn from(role: Role) -> Self {
        match role {
            Role::Application => Self::Application,
            Role::Frame | Role::Window | Role::InternalFrame => Self::Window,
            Role::Dialog | Role::Alert | Role::FileChooser | Role::ColorChooser => Self::Dialog,
            Role::Panel
            | Role::Filler
            | Role::RootPane
            | Role::ScrollPane
            | Role::Viewport
            | Role::Grouping
            | Role::Form
            | Role::Section => Self::Container,
            Role::Label | Role::Static | Role::Caption | Role::Heading => Self::Label,
            Role::Button | Role::PushButtonMenu => Self::Button,
            Role::ToggleButton => Self::ToggleButton,
            Role::CheckBox | Role::CheckMenuItem => Self::CheckBox,
            Role::RadioButton | Role::RadioMenuItem => Self::RadioButton,
            Role::Text | Role::Paragraph | Role::Terminal => Self::Text,
            Role::Entry | Role::PasswordText | Role::DateEditor | Role::Editbar => Self::TextInput,
            Role::MenuBar => Self::MenuBar,
            Role::Menu | Role::PopupMenu => Self::Menu,
            Role::MenuItem | Role::TearoffMenuItem => Self::MenuItem,
            Role::List | Role::ListBox => Self::List,
            Role::ListItem => Self::ListItem,
            Role::PageTab => Self::Tab,
            Role::PageTabList => Self::TabList,
            Role::Tree | Role::TreeTable => Self::Tree,
            Role::TreeItem => Self::TreeItem,
            Role::Table => Self::Table,
            Role::TableRow => Self::Row,
            Role::TableCell | Role::ColumnHeader | Role::RowHeader => Self::Cell,
            Role::Slider | Role::ScrollBar | Role::SpinButton | Role::Dial => Self::Slider,
            Role::ProgressBar | Role::LevelBar => Self::ProgressBar,
            Role::StatusBar => Self::StatusBar,
            other => Self::Unknown(other.name().to_owned()),
        }
    }
}

impl SemanticRole {
    pub fn from_atspi(role: Role, editable: bool) -> Self {
        if role == Role::Text && editable {
            Self::TextInput
        } else {
            Self::from(role)
        }
    }
}

impl fmt::Display for SemanticRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application => f.write_str("Application"),
            Self::Window => f.write_str("Window"),
            Self::Dialog => f.write_str("Dialog"),
            Self::Container => f.write_str("Container"),
            Self::Label => f.write_str("Label"),
            Self::Button => f.write_str("Button"),
            Self::ToggleButton => f.write_str("ToggleButton"),
            Self::CheckBox => f.write_str("CheckBox"),
            Self::RadioButton => f.write_str("RadioButton"),
            Self::Text => f.write_str("Text"),
            Self::TextInput => f.write_str("TextInput"),
            Self::MenuBar => f.write_str("MenuBar"),
            Self::Menu => f.write_str("Menu"),
            Self::MenuItem => f.write_str("MenuItem"),
            Self::List => f.write_str("List"),
            Self::ListItem => f.write_str("ListItem"),
            Self::Tab => f.write_str("Tab"),
            Self::TabList => f.write_str("TabList"),
            Self::Tree => f.write_str("Tree"),
            Self::TreeItem => f.write_str("TreeItem"),
            Self::Table => f.write_str("Table"),
            Self::Row => f.write_str("Row"),
            Self::Cell => f.write_str("Cell"),
            Self::Slider => f.write_str("Slider"),
            Self::ProgressBar => f.write_str("ProgressBar"),
            Self::StatusBar => f.write_str("StatusBar"),
            Self::Unknown(original) => write!(f, "Unknown({original})"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticState {
    Checked,
    Selected,
    Expanded,
    Collapsed,
    Enabled,
    Focused,
    Editable,
    Pressed,
    Busy,
    Indeterminate,
    ReadOnly,
    Other(String),
}

impl From<State> for SemanticState {
    fn from(state: State) -> Self {
        match state {
            State::Checked => Self::Checked,
            State::Selected => Self::Selected,
            State::Expanded => Self::Expanded,
            State::Collapsed => Self::Collapsed,
            State::Enabled => Self::Enabled,
            State::Focused => Self::Focused,
            State::Editable => Self::Editable,
            State::Pressed => Self::Pressed,
            State::Busy => Self::Busy,
            State::Indeterminate => Self::Indeterminate,
            State::ReadOnly => Self::ReadOnly,
            other => Self::Other(other.to_string()),
        }
    }
}

impl fmt::Display for SemanticState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Checked => "checked",
            Self::Selected => "selected",
            Self::Expanded => "expanded",
            Self::Collapsed => "collapsed",
            Self::Enabled => "enabled",
            Self::Focused => "focused",
            Self::Editable => "editable",
            Self::Pressed => "pressed",
            Self::Busy => "busy",
            Self::Indeterminate => "indeterminate",
            Self::ReadOnly => "read-only",
            Self::Other(value) => value,
        };
        f.write_str(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticAction {
    pub index: i32,
    pub name: String,
    pub description: Option<String>,
    pub keybinding: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugInfo {
    pub atspi_role: String,
    pub bus_name: String,
    pub object_path: String,
    pub interfaces: Vec<String>,
    pub geometry: Option<Geometry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeTruncation {
    MaxDepth {
        limit: usize,
    },
    MaxNodes {
        limit: usize,
    },
    OperationTimeout {
        operation: &'static str,
        node_id: String,
    },
}

impl fmt::Display for TreeTruncation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxDepth { limit } => write!(f, "tree truncated: max depth={limit}"),
            Self::MaxNodes { limit } => write!(f, "tree truncated: max nodes={limit}"),
            Self::OperationTimeout { operation, node_id } => {
                write!(f, "tree incomplete: {operation} timed out on {node_id}")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticNode {
    pub id: NodeId,
    pub role: SemanticRole,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub states: Vec<SemanticState>,
    pub actions: Vec<SemanticAction>,
    pub children: Vec<SemanticNode>,
    pub truncations: Vec<TreeTruncation>,
    pub debug: DebugInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_round_trip() {
        let id = NodeId::new(":1.42", "/org/a11y/atspi/accessible/17");
        let encoded = id.encode();
        assert!(!encoded.contains('/'));
        assert_eq!(NodeId::decode(&encoded).unwrap(), id);
    }

    #[test]
    fn invalid_node_ids_are_rejected() {
        assert_eq!(NodeId::decode("abc"), Err(NodeIdError::MissingPrefix));
        assert!(matches!(
            NodeId::decode("atspi1_!!!"),
            Err(NodeIdError::InvalidBase64)
        ));
    }

    #[test]
    fn maps_representative_roles() {
        assert_eq!(
            SemanticRole::from(Role::Application),
            SemanticRole::Application
        );
        assert_eq!(SemanticRole::from(Role::Entry), SemanticRole::TextInput);
        assert_eq!(SemanticRole::from(Role::PageTab), SemanticRole::Tab);
        assert_eq!(
            SemanticRole::from_atspi(Role::Text, true),
            SemanticRole::TextInput
        );
        assert_eq!(
            SemanticRole::from_atspi(Role::Text, false),
            SemanticRole::Text
        );
        assert_eq!(
            SemanticRole::from(Role::Image),
            SemanticRole::Unknown("image".to_owned())
        );
    }

    #[test]
    fn maps_representative_states() {
        assert_eq!(SemanticState::from(State::Checked), SemanticState::Checked);
        assert_eq!(
            SemanticState::from(State::Focusable),
            SemanticState::Other("focusable".to_owned())
        );
    }
}
