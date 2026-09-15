//! Panel identity: the [`PanelKey`] bound every reducer, projection, and
//! persistence API accepts, the interned [`SpecPanelId`] index for
//! spec-resolved panels, and the [`PanelCatalog`] that maps keys to the
//! stable string IDs layouts persist.
//!
//! [`PanelKind`] remains the source-compatible convenience for hand-written
//! fieldless enums; the blanket impl below keeps every existing enum working
//! wherever a [`PanelKey`] is required. Workspaces authored as data (Nix or
//! otherwise) cannot use a `&'static str` title enum, so their panel IDs are
//! validated and interned once into ordered [`SpecPanelId`] indices.

use std::collections::HashMap;
use std::fmt;
use serde::ser::{Error as SerdeError, Impossible, Serializer};
use serde::Serialize;

use crate::{kind_slug, PanelKind, PanelWin};

/// The panel identity every generic panel-kit API accepts.
///
/// Deliberately narrower than [`PanelKind`]: no serde bound, no static title
/// — a spec-resolved panel owns its title at runtime. Every `PanelKind` enum
/// satisfies this trait through a blanket impl, so existing closed-enum
/// consumers keep compiling unchanged wherever the bound is required.
pub trait PanelKey: Copy + Eq + std::hash::Hash + 'static {}

impl<T: PanelKind> PanelKey for T {}

/// An interned panel index, assigned once when an authored workspace spec is
/// resolved.
///
/// The wrapped index is a runtime identity only: it is never serialized or
/// persisted. Layouts persist stable string IDs (mapped through a
/// [`PanelCatalog`]) so reordering the authored panel list cannot invalidate
/// saved files. Interning happens inside this crate during spec resolution;
/// consumers receive ids from the resolved catalog and snapshot rather than
/// constructing their own.
///
/// ```compile_fail
/// use panel_kit_core::panel::SpecPanelId;
///
/// fn persisted<T: serde::Serialize>(_: &T) -> String {
///     String::new()
/// }
///
/// fn restored<T: serde::de::DeserializeOwned>(_: &str) -> T {
///     unimplemented!()
/// }
///
/// // Interned indices must stay runtime-only: neither serde bound
/// // persistence would need may be satisfied.
/// let _: fn(&SpecPanelId) -> String = persisted;
/// let _: fn(&str) -> SpecPanelId = restored;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpecPanelId(u32);

impl PanelKey for SpecPanelId {}
impl SpecPanelId {
    /// Intern a validated authored panel index.
    pub(crate) fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// Zero-based authored panel order assigned during spec resolution.
    pub const fn index(self) -> u32 {
        self.0
    }
}


/// Identity and presentation metadata for one panel in a [`PanelCatalog`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelMeta<K: PanelKey> {
    /// The panel's key: an app enum or a resolved [`SpecPanelId`].
    pub key: K,
    /// Stable string ID layouts persist. For app enums this must equal the
    /// enum's serde variant name (e.g. `"Workspace"`) so existing saves keep
    /// decoding.
    pub stable_id: Box<str>,
    /// Human-readable panel title, shown in the panel header.
    pub title: Box<str>,
    /// Stable slug derived from the title, used for CSS classes.
    pub slug: Box<str>,
}

/// Error returned when [`PanelCatalog::try_new`] receives entries that do not
/// form an unambiguous catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogError {
    /// A panel kind did not serialize as the stable string layouts persist.
    InvalidStableId,
    /// Two entries declared the same stable ID, so saved layouts could not
    /// tell those panels apart.
    DuplicateStableId(Box<str>),
    /// Two entries declared the same key, so key-to-stable-ID lookup would
    /// otherwise depend on insertion order.
    DuplicateKey,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStableId => f.write_str("panel kind did not serialize as a stable string"),
            Self::DuplicateStableId(id) => write!(f, "duplicate stable panel id `{id}`"),
            Self::DuplicateKey => f.write_str("duplicate panel key"),
        }
    }
}

impl std::error::Error for CatalogError {}

/// Ordered panel metadata with lookup by key and by stable persisted ID.
///
/// Built once — from app-enum entries, or by workspace-spec resolution — and
/// then shared immutably. Persistence maps keys to stable IDs through the
/// catalog when saving and back again when restoring, which is why layout
/// APIs never require `PanelKey: Serialize`.
#[derive(Debug)]
pub struct PanelCatalog<K: PanelKey> {
    entries: Box<[PanelMeta<K>]>,
    by_key: HashMap<K, usize>,
    by_stable_id: HashMap<Box<str>, usize>,
}

impl<K: PanelKey> PanelCatalog<K> {
    /// Build a catalog from entries in authored panel order, rejecting
    /// duplicate keys and stable IDs.
    pub fn try_new(entries: Vec<PanelMeta<K>>) -> Result<Self, CatalogError> {
        let entries = entries.into_boxed_slice();
        let mut by_key = HashMap::with_capacity(entries.len());
        let mut by_stable_id = HashMap::with_capacity(entries.len());

        for (index, meta) in entries.iter().enumerate() {
            if by_stable_id.insert(meta.stable_id.clone(), index).is_some() {
                return Err(CatalogError::DuplicateStableId(meta.stable_id.clone()));
            }
            if by_key.insert(meta.key, index).is_some() {
                return Err(CatalogError::DuplicateKey);
            }
        }

        Ok(Self {
            entries,
            by_key,
            by_stable_id,
        })
    }

    /// Metadata for a key, if the catalog declares it.
    pub fn get(&self, key: K) -> Option<&PanelMeta<K>> {
        self.by_key.get(&key).map(|&index| &self.entries[index])
    }

    /// Metadata for a stable persisted ID, if the catalog declares it.
    pub fn get_by_stable_id(&self, id: &str) -> Option<&PanelMeta<K>> {
        self.by_stable_id.get(id).map(|&index| &self.entries[index])
    }

    /// The stable ID a layout persists for `key`.
    pub fn stable_id(&self, key: K) -> Option<&str> {
        self.get(key).map(|meta| &*meta.stable_id)
    }

    /// Number of declared panels.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no panels are declared.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<K: PanelKind> PanelCatalog<K> {
    /// Build a catalog for a closed-enum panel layout.
    ///
    /// The stable IDs are serde's string variant names, preserving V1/V2 saved
    /// layout compatibility for existing `PanelKind` consumers. Titles come
    /// from [`PanelKind::title`], and slugs use the core [`kind_slug`] helper so
    /// web and TUI adapters cannot drift in their persistence catalog bridge.
    pub fn from_panel_kind_layout(panels: &[PanelWin<K>]) -> Result<Self, CatalogError> {
        let entries = panels
            .iter()
            .map(|panel| panel_kind_meta(panel.kind))
            .collect::<Result<Vec<_>, _>>()?;

        Self::try_new(entries)
    }
}

fn panel_kind_meta<K: PanelKind>(kind: K) -> Result<PanelMeta<K>, CatalogError> {
    let stable_id = kind
        .serialize(UnitVariantNameSerializer)
        .map_err(|_| CatalogError::InvalidStableId)?
        .into_boxed_str();

    Ok(PanelMeta {
        key: kind,
        stable_id,
        title: kind.title().into(),
        slug: kind_slug(kind.title()).into_boxed_str(),
    })
}

#[derive(Debug)]
struct StableIdSerializeError;

impl fmt::Display for StableIdSerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("panel kind did not serialize as a unit variant")
    }
}

impl std::error::Error for StableIdSerializeError {}

impl SerdeError for StableIdSerializeError {
    fn custom<T: fmt::Display>(_msg: T) -> Self {
        Self
    }
}

struct UnitVariantNameSerializer;

macro_rules! reject_stable_id_value {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(
            fn $name(self, $($arg: $ty),*) -> Result<$ret, Self::Error> {
                Err(StableIdSerializeError)
            }
        )*
    };
}

#[allow(unused_variables)]
impl Serializer for UnitVariantNameSerializer {
    type Ok = String;
    type Error = StableIdSerializeError;
    type SerializeSeq = Impossible<String, StableIdSerializeError>;
    type SerializeTuple = Impossible<String, StableIdSerializeError>;
    type SerializeTupleStruct = Impossible<String, StableIdSerializeError>;
    type SerializeTupleVariant = Impossible<String, StableIdSerializeError>;
    type SerializeMap = Impossible<String, StableIdSerializeError>;
    type SerializeStruct = Impossible<String, StableIdSerializeError>;
    type SerializeStructVariant = Impossible<String, StableIdSerializeError>;

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(variant.to_string())
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(StableIdSerializeError)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(StableIdSerializeError)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<Self::Ok, Self::Error> {
        Err(StableIdSerializeError)
    }

    reject_stable_id_value! {
        serialize_bool(value: bool) -> Self::Ok;
        serialize_i8(value: i8) -> Self::Ok;
        serialize_i16(value: i16) -> Self::Ok;
        serialize_i32(value: i32) -> Self::Ok;
        serialize_i64(value: i64) -> Self::Ok;
        serialize_u8(value: u8) -> Self::Ok;
        serialize_u16(value: u16) -> Self::Ok;
        serialize_u32(value: u32) -> Self::Ok;
        serialize_u64(value: u64) -> Self::Ok;
        serialize_f32(value: f32) -> Self::Ok;
        serialize_f64(value: f64) -> Self::Ok;
        serialize_char(value: char) -> Self::Ok;
        serialize_str(value: &str) -> Self::Ok;
        serialize_bytes(value: &[u8]) -> Self::Ok;
        serialize_none() -> Self::Ok;
        serialize_unit() -> Self::Ok;
        serialize_unit_struct(name: &'static str) -> Self::Ok;
        serialize_seq(len: Option<usize>) -> Self::SerializeSeq;
        serialize_tuple(len: usize) -> Self::SerializeTuple;
        serialize_tuple_struct(name: &'static str, len: usize) -> Self::SerializeTupleStruct;
        serialize_tuple_variant(name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Self::SerializeTupleVariant;
        serialize_map(len: Option<usize>) -> Self::SerializeMap;
        serialize_struct(name: &'static str, len: usize) -> Self::SerializeStruct;
        serialize_struct_variant(name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Self::SerializeStructVariant;
    }
}

#[cfg(test)]
#[path = "panel/tests.rs"]
mod tests;
