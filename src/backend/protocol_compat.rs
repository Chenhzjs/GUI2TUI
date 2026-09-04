//! AT-SPI wire compatibility adapters.
//!
//! This module converts protocol variants into toolkit-independent records. It
//! never chooses behavior from an application or toolkit name.

use std::time::{Duration, Instant};

use atspi::{
    CacheItem, InterfaceSet, LegacyCacheItem, Role, StateSet, events::EventBodyQtOwned,
    proxy::cache::CacheProxy,
};
use thiserror::Error;

use crate::semantic::{BackendLocator, SemanticAction};

/// Decode the historical `siiv(so)` PropertyChange body still emitted by
/// implementations that predate the current typed AT-SPI event signature.
///
/// This compatibility decision is made solely from the wire interface/member
/// and body signature. Application and toolkit names are deliberately absent.
pub fn normalize_legacy_property_event(
    message: &zbus::Message,
) -> Option<crate::events::NormalizedEvent> {
    let header = message.header();
    if header.interface()?.as_str() != "org.a11y.atspi.Event.Object"
        || header.member()?.as_str() != "PropertyChange"
    {
        return None;
    }
    let sender = header.sender()?;
    let path = header.path()?;
    let body = message.body().deserialize::<EventBodyQtOwned>().ok()?;
    Some(crate::events::NormalizedEvent::NodePropertyChanged {
        locator: BackendLocator::new(sender.as_str(), path.as_str()),
        property: body.kind,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheWireFormat {
    Modern,
    Legacy,
}

impl std::fmt::Display for CacheWireFormat {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str(match self {
            Self::Modern => "modern",
            Self::Legacy => "legacy",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkAccessibleRecord {
    pub locator: BackendLocator,
    pub application: Option<BackendLocator>,
    pub parent: Option<BackendLocator>,
    pub index_in_parent: Option<usize>,
    pub child_count: Option<usize>,
    /// Legacy records provide explicit children; modern records provide count/index.
    pub explicit_children: Option<Vec<BackendLocator>>,
    pub interfaces: InterfaceSet,
    pub name: Option<String>,
    pub role: Role,
    pub description: Option<String>,
    pub states: StateSet,
    pub actions: Vec<SemanticAction>,
    pub value: Option<String>,
    /// Selective live enrichment proved a finite bounded Value with a
    /// positive advertised increment. Cache interface exposure alone is not
    /// sufficient to set this flag.
    pub adjustable_value: bool,
}

#[derive(Clone, Debug)]
pub struct CacheFetch {
    pub format: CacheWireFormat,
    pub records: Vec<BulkAccessibleRecord>,
    pub rpc_duration: Duration,
    pub modern_error: Option<String>,
}

#[derive(Debug, Error)]
pub enum ProtocolCompatError {
    #[error("could not build AT-SPI Cache proxy: {0}")]
    Proxy(zbus::Error),
    #[error("AT-SPI Cache.GetItems failed as modern ({modern}) and legacy ({legacy})")]
    CacheUnavailable {
        modern: Box<zbus::Error>,
        legacy: Box<zbus::Error>,
    },
    #[error("cache record contains a null or malformed accessible object")]
    MalformedObject,
    #[error("cache record contains duplicate explicit child {0}")]
    DuplicateExplicitChild(BackendLocator),
}

impl TryFrom<CacheItem> for BulkAccessibleRecord {
    type Error = ProtocolCompatError;

    fn try_from(item: CacheItem) -> Result<Self, Self::Error> {
        Ok(Self {
            locator: required_locator(&item.object)?,
            application: locator(&item.app),
            parent: locator(&item.parent),
            index_in_parent: usize::try_from(item.index).ok(),
            child_count: usize::try_from(item.children).ok(),
            explicit_children: None,
            interfaces: item.ifaces,
            name: nonempty(item.short_name),
            role: item.role,
            description: nonempty(item.name),
            states: item.states,
            actions: Vec::new(),
            value: None,
            adjustable_value: false,
        })
    }
}

impl TryFrom<LegacyCacheItem> for BulkAccessibleRecord {
    type Error = ProtocolCompatError;

    fn try_from(item: LegacyCacheItem) -> Result<Self, Self::Error> {
        let mut seen = std::collections::HashSet::new();
        let children = item
            .children
            .iter()
            .filter_map(locator)
            .map(|child| {
                if seen.insert(child.clone()) {
                    Ok(child)
                } else {
                    Err(ProtocolCompatError::DuplicateExplicitChild(child))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            locator: required_locator(&item.object)?,
            application: locator(&item.app),
            parent: locator(&item.parent),
            index_in_parent: None,
            child_count: Some(children.len()),
            explicit_children: Some(children),
            interfaces: item.ifaces,
            name: nonempty(item.short_name),
            role: item.role,
            description: nonempty(item.name),
            states: item.states,
            actions: Vec::new(),
            value: None,
            adjustable_value: false,
        })
    }
}

pub async fn fetch_cache(
    connection: &zbus::Connection,
    destination: &str,
) -> Result<CacheFetch, ProtocolCompatError> {
    let proxy = CacheProxy::builder(connection)
        .destination(destination)
        .map_err(ProtocolCompatError::Proxy)?
        .build()
        .await
        .map_err(ProtocolCompatError::Proxy)?;
    let started = Instant::now();
    match proxy.get_items().await {
        Ok(items) => Ok(CacheFetch {
            format: CacheWireFormat::Modern,
            records: items
                .into_iter()
                .map(BulkAccessibleRecord::try_from)
                .collect::<Result<_, _>>()?,
            rpc_duration: started.elapsed(),
            modern_error: None,
        }),
        Err(modern) => {
            let legacy_started = Instant::now();
            match proxy.get_legacy_items().await {
                Ok(items) => Ok(CacheFetch {
                    format: CacheWireFormat::Legacy,
                    records: items
                        .into_iter()
                        .map(BulkAccessibleRecord::try_from)
                        .collect::<Result<_, _>>()?,
                    rpc_duration: legacy_started.elapsed(),
                    modern_error: Some(modern.to_string()),
                }),
                Err(legacy) => Err(ProtocolCompatError::CacheUnavailable {
                    modern: Box::new(modern),
                    legacy: Box::new(legacy),
                }),
            }
        }
    }
}

fn required_locator(object: &atspi::ObjectRefOwned) -> Result<BackendLocator, ProtocolCompatError> {
    locator(object).ok_or(ProtocolCompatError::MalformedObject)
}

fn locator(object: &atspi::ObjectRefOwned) -> Option<BackendLocator> {
    if object.is_null() {
        return None;
    }
    Some(BackendLocator::new(
        object.name_as_str()?,
        object.path_as_str(),
    ))
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use atspi::{Interface, ObjectRef, ObjectRefOwned, State};

    use super::*;

    fn object(path: &'static str) -> ObjectRefOwned {
        ObjectRefOwned::from_static_str_unchecked(":1.2", path)
    }

    #[test]
    fn modern_cache_item_maps_to_bulk_record_and_handles_negative_index() {
        let item = CacheItem {
            object: object("/button"),
            app: object("/root"),
            parent: object("/window"),
            index: -1,
            children: 0,
            ifaces: InterfaceSet::new(Interface::Accessible | Interface::Action),
            short_name: "Save".to_owned(),
            role: Role::Button,
            name: "Save document".to_owned(),
            states: [State::Enabled].into_iter().collect(),
        };
        let record = BulkAccessibleRecord::try_from(item).unwrap();
        assert_eq!(record.locator, BackendLocator::new(":1.2", "/button"));
        assert_eq!(record.index_in_parent, None);
        assert_eq!(record.child_count, Some(0));
        assert!(record.interfaces.contains(Interface::Action));
    }

    #[test]
    fn legacy_cache_item_maps_explicit_children() {
        let item = LegacyCacheItem {
            object: object("/list"),
            app: object("/root"),
            parent: object("/window"),
            children: vec![object("/a"), object("/b")],
            ifaces: InterfaceSet::new(Interface::Accessible | Interface::Selection),
            short_name: "Items".to_owned(),
            role: Role::List,
            name: String::new(),
            states: StateSet::empty(),
        };
        let record = BulkAccessibleRecord::try_from(item).unwrap();
        assert_eq!(record.explicit_children.as_ref().unwrap().len(), 2);
        assert_eq!(record.child_count, Some(2));
    }

    #[test]
    fn malformed_and_duplicate_legacy_records_are_rejected() {
        let malformed = CacheItem {
            object: ObjectRefOwned::new(ObjectRef::Null),
            ..CacheItem::default()
        };
        assert!(BulkAccessibleRecord::try_from(malformed).is_err());

        let legacy = LegacyCacheItem {
            object: object("/list"),
            children: vec![object("/same"), object("/same")],
            ..LegacyCacheItem::default()
        };
        assert!(matches!(
            BulkAccessibleRecord::try_from(legacy),
            Err(ProtocolCompatError::DuplicateExplicitChild(_))
        ));
    }

    #[test]
    fn modern_and_legacy_password_cache_records_never_carry_a_value() {
        let modern = BulkAccessibleRecord::try_from(CacheItem {
            object: object("/modern-password"),
            role: Role::PasswordText,
            short_name: "Password".to_owned(),
            ..CacheItem::default()
        })
        .unwrap();
        let legacy = BulkAccessibleRecord::try_from(LegacyCacheItem {
            object: object("/legacy-password"),
            role: Role::PasswordText,
            short_name: "Password".to_owned(),
            ..LegacyCacheItem::default()
        })
        .unwrap();
        assert_eq!(modern.value, None);
        assert_eq!(legacy.value, None);
    }
}
