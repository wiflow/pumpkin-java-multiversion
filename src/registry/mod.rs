//! Version specific registry data.
//!
//! A 26.3 server sends registry entries whose NBT an older client cannot decode,
//! and the client rejects the entire registry load rather than the offending
//! entry. [`generated`] holds the registry contents for each supported version,
//! produced from that version's datapack by `tools/registry-codegen`.
//!
//! 1.20.5 is the oldest version with this packet at all: below it the whole
//! registry set arrives as one NBT blob in a single packet (that is the tier 3
//! config stall). From 1.20.5 up the payload is the same shape -- registry id,
//! entry count, then per entry an id, a "has data" flag and network NBT with
//! no root name -- so the parser here needs no version branch for protocol
//! 766, and `REGISTRY_V_1_20_5` supplies that version's own NBT.

pub mod generated;

use std::io::Cursor;

use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::deserializer::{NbtReadHelper, NbtReadHelperJava};
use pumpkin_nbt::tag::NbtTag;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::ser::{NetworkReadExt, NetworkWriteExt};
use pumpkin_util::version::JavaMinecraftVersion;

/// One entry as it arrived from the server: its id and its raw NBT, if any.
struct IncomingEntry<'a> {
    entry_id: String,
    data: Option<&'a [u8]>,
}

/// Splits a `REGISTRY_DATA` payload into its registry id and the rest.
fn read_registry_id(payload: &[u8]) -> Option<(String, &[u8])> {
    let mut cursor: &[u8] = payload;
    let registry_id: String = cursor.get_str().ok()?.into();
    Some((registry_id, cursor))
}

/// Parses the entries that follow the registry id, skipping over each entry's
/// NBT only far enough to find where it ends. The root tag is not always a
/// compound (`block_transformer` entries are lists), so this walks the tag
/// rather than reading a document.
fn read_entries(mut cursor: &[u8]) -> Option<Vec<IncomingEntry<'_>>> {
    let count = usize::try_from(cursor.get_var_int().ok()?.0).ok()?;

    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let entry_id: String = cursor.get_str().ok()?.into();
        let has_data = cursor.get_bool().ok()?;
        let data = if has_data {
            let mut nbt_cursor = Cursor::new(cursor);
            let mut reader = NbtReadHelperJava::new(&mut nbt_cursor);
            let tag_id = reader.get_u8().ok()?;
            NbtTag::skip_data(&mut reader, tag_id).ok()?;
            let used = usize::try_from(nbt_cursor.position()).ok()?;
            let (data, rest) = cursor.split_at(used);
            cursor = rest;
            Some(data)
        } else {
            None
        };
        entries.push(IncomingEntry { entry_id, data });
    }
    Some(entries)
}

/// Rewrites a `REGISTRY_DATA` payload for `version`.
///
/// Entries the version's datapack knows are replaced with that version's NBT.
/// Entries it does not know (26.3 additions such as new biomes or songs) keep
/// their slot, since the client numbers entries by position and the server
/// refers to those numbers later, but carry a stand-in copied from a known
/// entry: the old client cannot parse the 26.3 NBT, and one bad entry fails
/// the whole registry load.
///
/// Returns `None` when this plugin has no registry data for `version`, and
/// `Some(None)` when the version has no such registry at all, in which case the
/// packet should be dropped rather than sent with entries the client cannot use.
#[must_use]
#[allow(clippy::option_option)]
pub fn build_registry_payload(
    version: JavaMinecraftVersion,
    payload: &[u8],
) -> Option<Option<Vec<u8>>> {
    let registries = generated::get_synced(version)?;
    let (registry_id, rest) = read_registry_id(payload)?;

    // Generated ids are bare ("worldgen/biome"); the wire form is namespaced.
    // Decide on the registry before touching its entries, so one the version
    // does not have is dropped even if its entries could not be walked.
    let wanted = registry_id
        .strip_prefix("minecraft:")
        .unwrap_or(&registry_id);

    let Some(registry) = registries.iter().find(|r| r.registry_id == wanted) else {
        return Some(None);
    };

    let incoming = read_entries(rest)?;

    // Stand-in for entries this version does not have. Plains is the least
    // surprising biome to render an unknown one as; elsewhere any entry does.
    let fallback = registry
        .entries
        .iter()
        .find(|e| e.name == "plains")
        .or_else(|| registry.entries.first())?;

    let mut buf = Vec::with_capacity(payload.len());
    buf.write_string(&registry_id).ok()?;
    buf.write_var_int(&VarInt(i32::try_from(incoming.len()).ok()?))
        .ok()?;

    for entry in &incoming {
        let bare = entry
            .entry_id
            .strip_prefix("minecraft:")
            .unwrap_or(&entry.entry_id);
        let known = registry.entries.iter().find(|e| e.name == bare);

        buf.write_string(&entry.entry_id).ok()?;
        match (known, entry.data) {
            (Some(known), _) => {
                buf.write_bool(true).ok()?;
                buf.extend_from_slice(known.data);
            }
            (None, Some(_)) => {
                buf.write_bool(true).ok()?;
                buf.extend_from_slice(fallback.data);
            }
            (None, None) => buf.write_bool(false).ok()?,
        }
    }

    Some(Some(buf))
}

/// Rewrites the single-compound `REGISTRY_DATA` payload that 1.20.2 to 1.20.4
/// clients receive.
///
/// Those versions get every synced registry in one packet, as one unnamed
/// network NBT compound in the vanilla layout
///
/// ```text
/// { "<registry id>": { "type": "<registry id>",
///                      "value": [ { "name": str, "id": int,
///                                   "element": <tag> }, ... ] }, ... }
/// ```
///
/// The rewrite does for that bundle what [`build_registry_payload`] does for
/// the per registry packets of 1.20.5 and up: a registry the version does not
/// have is left out of the compound entirely, and every entry of a kept
/// registry keeps the server's name, its position and its id but carries this
/// version's own NBT. A name the version does not know keeps its slot with a
/// stand-in element, since the server refers to entries by the id it sent.
///
/// Returns `None` when there is no registry data for `version` or the payload
/// is not a bundle this can parse in full; the caller drops the packet then,
/// because a partly rewritten bundle fails the client's whole registry load.
#[must_use]
pub fn build_registry_bundle_payload(
    version: JavaMinecraftVersion,
    payload: &[u8],
) -> Option<Vec<u8>> {
    let root = read_unnamed_compound(payload)?;
    let out = rewrite_registry_codec(version, &root)?;
    Some(pumpkin_nbt::Nbt::from(out).write_unnamed().to_vec())
}

/// The element NBT `version`'s datapack has for one `dimension_type` entry,
/// as a whole tag (type id then payload).
///
/// 1.16.2 to 1.18.2 repeat the current dimension's element inline in the join
/// and respawn packets instead of naming it, so it has to be replaced the same
/// way the codec is. `name` may be bare or namespaced; an unknown name falls
/// back to the overworld, which is what the server itself does when it cannot
/// find the dimension.
#[must_use]
pub fn dimension_type_element(version: JavaMinecraftVersion, name: &str) -> Option<&'static [u8]> {
    let registries = generated::get_synced(version)?;
    let registry = registries
        .iter()
        .find(|r| r.registry_id == "dimension_type")?;
    let bare = name.strip_prefix("minecraft:").unwrap_or(name);
    registry
        .entries
        .iter()
        .find(|e| e.name == bare)
        .or_else(|| registry.entries.iter().find(|e| e.name == "overworld"))
        .or_else(|| registry.entries.first())
        .map(|entry| entry.data)
}

/// The current dimension's `min_y` and `height`. 1.16.x carries neither and
/// is fixed at 0 to 255.
#[must_use]
pub fn dimension_bounds(version: JavaMinecraftVersion, name: &str) -> Option<(i32, i32)> {
    let element = read_unnamed_compound(dimension_type_element(version, name)?)?;
    Some((
        element.get_int("min_y").unwrap_or(0),
        element.get_int("height").unwrap_or(256),
    ))
}

/// Rewrites the registry codec compound in place: registries `version` does
/// not have are left out, and every kept entry keeps the server's name and id
/// but carries this version's own element NBT.
///
/// Shared by the configuration `REGISTRY_DATA` bundle of 1.20.2 to 1.20.4 and
/// by the dimension codec inside the play `LOGIN` packet of 1.16.2 to 1.20.1:
/// the two carry the same compound, only the NBT root differs (unnamed on
/// 1.20.2 and up, a named empty root below it).
#[must_use]
pub fn rewrite_registry_codec(
    version: JavaMinecraftVersion,
    root: &NbtCompound,
) -> Option<NbtCompound> {
    let registries = generated::get_synced(version)?;

    let mut out = NbtCompound::new();
    for (registry_key, registry_value) in &root.child_tags {
        // Generated ids are bare ("worldgen/biome"); the wire form is namespaced.
        let bare = registry_key
            .strip_prefix("minecraft:")
            .unwrap_or(registry_key);
        let Some(registry) = registries.iter().find(|r| r.registry_id == bare) else {
            // A registry this version does not have: leave it out rather than
            // hand the client entries it cannot decode.
            continue;
        };
        let body = registry_value.extract_compound()?;
        let incoming = body.get_list("value")?;

        // Stand-in for entries this version does not have. Plains is the least
        // surprising biome to render an unknown one as; elsewhere any entry does.
        let fallback = registry
            .entries
            .iter()
            .find(|e| e.name == "plains")
            .or_else(|| registry.entries.first())?;

        let mut entries = Vec::with_capacity(incoming.len());
        for entry in incoming {
            let entry = entry.extract_compound()?;
            let name = entry.get_string("name")?;
            let id = entry.get_int("id")?;
            let bare_name = name.strip_prefix("minecraft:").unwrap_or(name);
            let known = registry
                .entries
                .iter()
                .find(|e| e.name == bare_name)
                .unwrap_or(fallback);

            let mut rewritten = NbtCompound::new();
            rewritten.put_string("name", name.to_string());
            rewritten.put_int("id", id);
            rewritten.put("element", read_tag(known.data)?);
            entries.push(NbtTag::Compound(rewritten));
        }

        let mut rewritten_registry = NbtCompound::new();
        rewritten_registry.put_string(
            "type",
            body.get_string("type").unwrap_or(registry_key).to_string(),
        );
        rewritten_registry.put_list("value", entries);
        out.put(registry_key, NbtTag::Compound(rewritten_registry));
    }

    Some(out)
}

/// Reads `data` as one unnamed network NBT compound, rejecting trailing bytes.
fn read_unnamed_compound(data: &[u8]) -> Option<NbtCompound> {
    match read_tag(data)? {
        NbtTag::Compound(compound) => Some(compound),
        _ => None,
    }
}

/// Reads one whole NBT tag (type id then payload, no root name), rejecting
/// trailing bytes. Registry elements are compounds in practice, but the
/// generated data stores whatever tag that version's datapack held.
fn read_tag(data: &[u8]) -> Option<NbtTag> {
    let mut cursor = Cursor::new(data);
    let mut reader = NbtReadHelperJava::new(&mut cursor);
    let tag_id = reader.get_u8().ok()?;
    let tag = NbtTag::deserialize_data(&mut reader, tag_id).ok()?;
    if usize::try_from(cursor.position()).ok()? == data.len() {
        Some(tag)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_nbt::Nbt;

    /// One `{ name, id, element }` entry as the server writes it.
    fn entry(name: &str, id: i32) -> NbtTag {
        let mut element = NbtCompound::new();
        element.put_string("marker", "from-server".to_string());
        let mut entry = NbtCompound::new();
        entry.put_string("name", name.to_string());
        entry.put_int("id", id);
        entry.put("element", NbtTag::Compound(element));
        NbtTag::Compound(entry)
    }

    fn registry(id: &str, entries: Vec<NbtTag>) -> NbtTag {
        let mut body = NbtCompound::new();
        body.put_string("type", id.to_string());
        body.put_list("value", entries);
        NbtTag::Compound(body)
    }

    #[test]
    fn bundle_drops_unknown_registries_and_replaces_elements() {
        let mut root = NbtCompound::new();
        root.put(
            "minecraft:worldgen/biome",
            registry(
                "minecraft:worldgen/biome",
                vec![
                    entry("minecraft:plains", 0),
                    // 26.3 only: 1.20.3 has no pale garden.
                    entry("minecraft:pale_garden", 1),
                ],
            ),
        );
        // 1.20.3 has no wolf_variant registry at all.
        root.put(
            "minecraft:wolf_variant",
            registry("minecraft:wolf_variant", vec![entry("minecraft:pale", 0)]),
        );
        let payload = Nbt::from(root).write_unnamed().to_vec();

        let out = build_registry_bundle_payload(JavaMinecraftVersion::V_1_20_3, &payload)
            .expect("bundle rewritten");
        let rewritten = read_unnamed_compound(&out).expect("output parses");

        assert!(
            rewritten.get("minecraft:wolf_variant").is_none(),
            "a registry the version lacks is left out"
        );
        let biome = rewritten
            .get_compound("minecraft:worldgen/biome")
            .expect("biome registry kept");
        assert_eq!(biome.get_string("type"), Some("minecraft:worldgen/biome"));
        let values = biome.get_list("value").expect("value list");
        assert_eq!(values.len(), 2, "entry count and order kept");

        let generated = generated::get_synced(JavaMinecraftVersion::V_1_20_3)
            .expect("1.20.3 registry data")
            .iter()
            .find(|r| r.registry_id == "worldgen/biome")
            .expect("biome registry generated");
        let plains_nbt = read_tag(
            generated
                .entries
                .iter()
                .find(|e| e.name == "plains")
                .expect("plains generated")
                .data,
        )
        .expect("plains nbt parses");

        for (index, (name, id)) in [("minecraft:plains", 0), ("minecraft:pale_garden", 1)]
            .into_iter()
            .enumerate()
        {
            let value = values[index].extract_compound().expect("entry compound");
            assert_eq!(value.get_string("name"), Some(name), "name kept");
            assert_eq!(value.get_int("id"), Some(id), "id kept");
            let element = value.get("element").expect("element");
            assert!(
                element
                    .extract_compound()
                    .is_some_and(|c| c.get_string("marker").is_none()),
                "the server's element is replaced"
            );
            // Both the known name and the one 1.20.3 lacks end up as this
            // version's plains: the latter through the stand-in rule.
            assert_eq!(element, &plains_nbt, "this version's own NBT");
        }
    }

    #[test]
    fn bundle_with_trailing_bytes_is_dropped() {
        let mut root = NbtCompound::new();
        root.put(
            "minecraft:worldgen/biome",
            registry(
                "minecraft:worldgen/biome",
                vec![entry("minecraft:plains", 0)],
            ),
        );
        let mut payload = Nbt::from(root).write_unnamed().to_vec();
        payload.push(0);
        assert!(
            build_registry_bundle_payload(JavaMinecraftVersion::V_1_20_3, &payload).is_none(),
            "a bundle with bytes left over is not a bundle this understood"
        );
    }

    #[test]
    fn bundle_that_does_not_parse_is_dropped() {
        assert!(
            build_registry_bundle_payload(JavaMinecraftVersion::V_1_20_3, &[0x0a, 0x08]).is_none(),
            "a truncated bundle is dropped rather than half rewritten"
        );
    }
}
