//! The cross-layer universe: semantic families, per-layer definitions and
//! shared wire codecs, with the same validation as `td/gen/semantic/universe.go`.

use std::collections::BTreeMap;

use crate::model::{Definition, SchemaModel, Shape, TypeRef, semantic_stable_id};
use crate::parse::Category;

/// A reusable payload codec, unique by wire identifier.
#[derive(Debug, Clone)]
pub(crate) struct WireCodec {
    /// Wire identifier.
    pub(crate) wire_id: u32,
    /// Semantic key every layer agrees on.
    pub(crate) key: crate::model::SemanticKey,
    /// Payload shape.
    pub(crate) shape: Shape,
    /// Layers that define this wire identifier.
    pub(crate) layers: Vec<i32>,
}

/// A group of identical signatures across layers.
#[derive(Debug, Clone)]
pub(crate) struct SignatureVariant {
    /// Wire identifier.
    pub(crate) wire_id: u32,
    /// Body shape.
    pub(crate) body_shape: Shape,
    /// Body + result shape.
    pub(crate) signature_shape: Shape,
    /// Layers sharing this signature.
    pub(crate) layers: Vec<i32>,
}

/// The same category-qualified definition across layers.
#[derive(Debug, Clone)]
pub(crate) struct Family {
    /// Semantic key.
    pub(crate) key: crate::model::SemanticKey,
    /// Definition per layer.
    pub(crate) by_layer: BTreeMap<i32, Definition>,
    /// Distinct signature variants.
    pub(crate) variants: Vec<SignatureVariant>,
}

impl Family {
    /// Semantic identity, stable across layers.
    pub(crate) fn stable_id(&self) -> u64 {
        semantic_stable_id(&self.key)
    }

    /// Whether the family is a method.
    pub(crate) fn is_method(&self) -> bool {
        self.key.category == Category::Function
    }
}

/// The validated collection of all supported layer schemas.
#[derive(Debug, Clone)]
pub(crate) struct Universe {
    /// The canonical layer number.
    pub(crate) canonical_layer: i32,
    /// Layer schemas, ascending.
    pub(crate) schemas: BTreeMap<i32, SchemaModel>,
    /// Families by semantic key, in deterministic order.
    pub(crate) families: BTreeMap<crate::model::SemanticKey, Family>,
    /// Wire codecs by identifier.
    pub(crate) wire_codecs: BTreeMap<u32, WireCodec>,
    /// Digest of the whole universe.
    pub(crate) semantic_digest: Shape,
}

impl Universe {
    /// Loaded layers, ascending.
    pub(crate) fn layers(&self) -> Vec<i32> {
        self.schemas.keys().copied().collect()
    }

    /// The canonical schema.
    pub(crate) fn canonical(&self) -> &SchemaModel {
        &self.schemas[&self.canonical_layer]
    }

    /// Builds a universe from layer schemas.
    ///
    /// # Errors
    ///
    /// Returns a message when a layer is duplicated, the canonical layer is
    /// missing, or a shared wire identifier disagrees across layers.
    pub(crate) fn new(canonical_layer: i32, schemas: Vec<SchemaModel>) -> Result<Universe, String> {
        if canonical_layer <= 0 {
            return Err(format!("invalid canonical layer {canonical_layer}"));
        }
        let mut map: BTreeMap<i32, SchemaModel> = BTreeMap::new();
        for schema in schemas {
            if schema.layer <= 0 {
                return Err(format!("invalid schema layer {}", schema.layer));
            }
            if map.contains_key(&schema.layer) {
                return Err(format!("duplicate schema layer {}", schema.layer));
            }
            map.insert(schema.layer, schema);
        }
        if !map.contains_key(&canonical_layer) {
            return Err(format!("canonical layer {canonical_layer} is not loaded"));
        }
        let layers: Vec<i32> = map.keys().copied().collect();
        let mut families: BTreeMap<crate::model::SemanticKey, Family> = BTreeMap::new();
        let mut wire_codecs: BTreeMap<u32, WireCodec> = BTreeMap::new();

        for layer in &layers {
            let schema = &map[layer];
            for definition in &schema.definitions {
                match wire_codecs.get_mut(&definition.wire_id) {
                    None => {
                        wire_codecs.insert(
                            definition.wire_id,
                            WireCodec {
                                wire_id: definition.wire_id,
                                key: definition.key.clone(),
                                shape: definition.wire_shape,
                                layers: vec![*layer],
                            },
                        );
                    }
                    Some(codec) => {
                        if codec.key != definition.key {
                            return Err(format!(
                                "wire id {:#010x} maps to {} in one layer and {} in layer {layer}",
                                definition.wire_id,
                                codec.key.as_string(),
                                definition.key.as_string()
                            ));
                        }
                        if codec.shape != definition.wire_shape {
                            return Err(format!(
                                "wire id {:#010x} for {} has conflicting payload shapes (layer {layer})",
                                definition.wire_id,
                                definition.key.as_string()
                            ));
                        }
                        codec.layers.push(*layer);
                    }
                }

                let family = families
                    .entry(definition.key.clone())
                    .or_insert_with(|| Family {
                        key: definition.key.clone(),
                        by_layer: BTreeMap::new(),
                        variants: Vec::new(),
                    });
                family.by_layer.insert(*layer, definition.clone());
                match family.variants.iter_mut().find(|variant| {
                    variant.wire_id == definition.wire_id
                        && variant.signature_shape == definition.signature_shape
                }) {
                    Some(variant) => variant.layers.push(*layer),
                    None => family.variants.push(SignatureVariant {
                        wire_id: definition.wire_id,
                        body_shape: definition.body_shape,
                        signature_shape: definition.signature_shape,
                        layers: vec![*layer],
                    }),
                }
            }
        }
        let mut universe = Universe {
            canonical_layer,
            schemas: map,
            families,
            wire_codecs,
            semantic_digest: [0; 32],
        };
        universe.semantic_digest = universe_digest(&universe);
        Ok(universe)
    }

    /// The definition of `key` in `layer`, if present.
    pub(crate) fn definition(
        &self,
        layer: i32,
        key: &crate::model::SemanticKey,
    ) -> Option<&Definition> {
        self.families.get(key)?.by_layer.get(&layer)
    }

    /// Reports whether a field type references the dynamic `Object` or a
    /// generic slot anywhere in its graph.
    pub(crate) fn type_ref_dynamic(reference: &TypeRef) -> bool {
        if reference.kind == crate::model::TypeKind::GenericRef
            || (reference.kind == crate::model::TypeKind::Primitive && reference.qname == "Object")
        {
            return true;
        }
        reference
            .arg
            .as_deref()
            .is_some_and(Universe::type_ref_dynamic)
    }
}

fn universe_digest(universe: &Universe) -> Shape {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let domain = "gotd.tl.semantic.universe.v1";
    hasher.update((domain.len() as u64).to_le_bytes());
    hasher.update(domain.as_bytes());
    hasher.update((universe.canonical_layer as u64).to_le_bytes());
    let layers = universe.layers();
    hasher.update((layers.len() as u64).to_le_bytes());
    for layer in layers {
        let schema = &universe.schemas[&layer];
        hasher.update((layer as u64).to_le_bytes());
        hasher.update(schema.source.sha256);
        hasher.update((schema.definitions.len() as u64).to_le_bytes());
        for definition in &schema.definitions {
            hasher.update([definition.key.category as u8]);
            hasher.update((definition.key.qname.len() as u64).to_le_bytes());
            hasher.update(definition.key.qname.as_bytes());
            hasher.update((definition.wire_id as u64).to_le_bytes());
            hasher.update(definition.signature_shape);
        }
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}
