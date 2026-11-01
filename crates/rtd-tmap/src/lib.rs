//! Combined wire-id to qualified-name registry.
#![warn(missing_docs)]
//!
//! This is the rtd equivalent of Go `td/tmap`: [`Map`] merges the generated
//! per-crate registries of [`rtd_tg`], [`rtd_mt`] and [`rtd_proto`] into a
//! single lookup from an MTProto wire identifier to its qualified TL type
//! name. Where the same identifier appears in more than one registry, the
//! later registry wins (`rtd-proto` over `rtd-mt` over `rtd-tg`), mirroring
//! the merge order of `td/telegram/typemap.go`.

use std::collections::BTreeMap;

/// The wire-id to qualified-name entries contributed by `rtd-proto`.
///
/// `rtd-proto` exposes its protocol type identifiers as named constants rather
/// than a registry function, so they are listed here, matching Go's
/// `proto.TypesMap`. The names are the canonical protocol names used by the
/// Go `proto` package.
const PROTO_TYPES: &[(u32, &str)] = &[
    (rtd_proto::MESSAGE_CONTAINER_TYPE_ID, "message_container"),
    (rtd_proto::RPC_RESULT_TYPE_ID, "rpc_result"),
    (rtd_proto::GZIP_TYPE_ID, "gzip"),
];

/// A map from MTProto wire identifiers to qualified TL type names.
///
/// Built by merging the `rtd-tg`, `rtd-mt` and `rtd-proto` registries; see the
/// module documentation for the merge order.
#[derive(Debug, Clone)]
pub struct Map {
    types: BTreeMap<u32, &'static str>,
}

impl Map {
    /// Creates the merged registry.
    #[must_use]
    pub fn new() -> Self {
        let mut types = BTreeMap::new();
        for (id, name) in rtd_tg::types_map()
            .iter()
            .chain(rtd_mt::types_map().iter())
            .chain(PROTO_TYPES.iter())
        {
            types.insert(*id, *name);
        }
        Self { types }
    }

    /// Returns the qualified name registered for `id`, or `None` when the
    /// identifier is unknown.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&str> {
        self.types.get(&id).copied()
    }
}

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Map;

    #[test]
    fn resolves_tg_ids() {
        let map = Map::new();
        // A tg-only type from the canonical model.
        assert_eq!(map.get(0x004a_8537), Some("securePasswordKdfAlgoUnknown"));
        assert_eq!(map.get(0x007e_fe0e), Some("storage.fileJpeg"));
    }

    #[test]
    fn resolves_mt_ids() {
        let map = Map::new();
        assert_eq!(map.get(0x7abe_77ec), Some("ping"));
        assert_eq!(map.get(0x6046_9778), Some("req_pq"));
        assert_eq!(map.get(0x9ec2_0908), Some("new_session_created"));
    }

    #[test]
    fn resolves_proto_ids() {
        let map = Map::new();
        assert_eq!(
            map.get(rtd_proto::MESSAGE_CONTAINER_TYPE_ID),
            Some("message_container")
        );
        assert_eq!(map.get(rtd_proto::RPC_RESULT_TYPE_ID), Some("rpc_result"));
        assert_eq!(map.get(rtd_proto::GZIP_TYPE_ID), Some("gzip"));
    }

    #[test]
    fn unknown_id_is_none() {
        let map = Map::new();
        assert_eq!(map.get(0xdead_beef), None);
        assert_eq!(Map::default().get(0), None);
    }
}
