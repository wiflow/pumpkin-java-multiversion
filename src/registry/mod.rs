//! Version specific registry data.
//!
//! A 26.3 server sends registry entries whose NBT an older client cannot decode,
//! and the client rejects the entire registry load rather than the offending
//! entry. [`generated`] holds the registry contents for each supported version,
//! produced from that version's datapack by `tools/registry-codegen`.

pub mod generated;

use std::io::Cursor;

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
