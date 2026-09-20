//! Rewrites `UPDATE_TAGS` for the client's registries and id numbering.
//!
//! The packet carries one group per registry, each tag listing its members by
//! numeric id. Several things go wrong for an older client: a group for a
//! registry it lacks (`dialog` before 1.21.6, `timeline` and `potion` before
//! 26.x) is fatal rather than skipped; the member ids are 26.3's, so
//! `mineable` and the item tags would point at the wrong blocks and items; and
//! tags its own datapack had but 26.3 dropped (`enchantable/sword` on 1.21.5)
//! are still referenced by that version's registry entries, and an unresolved
//! tag fails the whole registry load. Groups the client cannot resolve are
//! dropped, block, item and entity type members are renumbered with members
//! the client does not have left out, and the missing tags are added back.

use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::ser::{NetworkReadExt, NetworkWriteExt};
use pumpkin_util::version::JavaMinecraftVersion;

use crate::remap::tag_id_remap::{
    block_id_for_version, entity_type_id_for_version, item_id_for_version,
};

struct Tag {
    name: String,
    ids: Vec<i32>,
}

struct Group {
    registry: String,
    tags: Vec<Tag>,
}

fn read_groups(payload: &[u8]) -> Option<Vec<Group>> {
    let mut cursor: &[u8] = payload;
    let count = usize::try_from(cursor.get_var_int().ok()?.0).ok()?;
    let mut groups = Vec::with_capacity(count);
    for _ in 0..count {
        let registry: String = cursor.get_str().ok()?.into();
        let tag_count = usize::try_from(cursor.get_var_int().ok()?.0).ok()?;
        let mut tags = Vec::with_capacity(tag_count);
        for _ in 0..tag_count {
            let name: String = cursor.get_str().ok()?.into();
            let id_count = usize::try_from(cursor.get_var_int().ok()?.0).ok()?;
            let mut ids = Vec::with_capacity(id_count);
            for _ in 0..id_count {
                ids.push(cursor.get_var_int().ok()?.0);
            }
            tags.push(Tag { name, ids });
        }
        groups.push(Group { registry, tags });
    }
    Some(groups)
}

/// The id mapper for a registry, or `None` for registries whose ids are the
/// same on every version we translate for (positional dynamic registries and
/// static ones Via has no mapping for).
fn mapper(registry: &str) -> Option<fn(u16, JavaMinecraftVersion) -> Option<u16>> {
    match registry {
        "block" => Some(block_id_for_version),
        "item" => Some(item_id_for_version),
        "entity_type" => Some(entity_type_id_for_version),
        _ => None,
    }
}

/// Tags of `registry` that `version`'s datapack has and 26.3 does not, minus
/// any the server sent anyway. Members are 26.3 ids, renumbered like the rest.
fn extra_tags(registry: &str, group: &Group, version: JavaMinecraftVersion) -> Vec<Tag> {
    crate::registry::generated::get_extra_tags(version)
        .unwrap_or(&[])
        .iter()
        .filter(|extra| extra.registry == registry)
        .map(|extra| Tag {
            name: format!("minecraft:{}", extra.tag),
            ids: extra.members.iter().copied().map(i32::from).collect(),
        })
        .filter(|extra| !group.tags.iter().any(|tag| tag.name == extra.name))
        .collect()
}

/// Rewrites an `UPDATE_TAGS` payload for `version`. Returns `None` when there
/// is no tag data for the version or the payload could not be walked, in
/// which case it should go out unchanged.
#[must_use]
pub fn rewrite_update_tags(payload: &[u8], version: JavaMinecraftVersion) -> Option<Vec<u8>> {
    let known = crate::registry::generated::get_tag_registries(version)?;
    let groups = read_groups(payload)?;

    let kept: Vec<&Group> = groups
        .iter()
        .filter(|group| {
            let bare = group
                .registry
                .strip_prefix("minecraft:")
                .unwrap_or(&group.registry);
            known.contains(&bare)
        })
        .collect();

    let mut out = Vec::with_capacity(payload.len());
    out.write_var_int(&VarInt(i32::try_from(kept.len()).ok()?))
        .ok()?;
    for group in kept {
        let bare = group
            .registry
            .strip_prefix("minecraft:")
            .unwrap_or(&group.registry);
        let map = mapper(bare);
        let extras = extra_tags(bare, group, version);

        out.write_string(&group.registry).ok()?;
        out.write_var_int(&VarInt(
            i32::try_from(group.tags.len() + extras.len()).ok()?,
        ))
        .ok()?;
        for tag in group.tags.iter().chain(extras.iter()) {
            let ids: Vec<i32> = match map {
                Some(map) => tag
                    .ids
                    .iter()
                    .filter_map(|&id| u16::try_from(id).ok())
                    .filter_map(|id| map(id, version))
                    .map(i32::from)
                    .collect(),
                None => tag.ids.clone(),
            };
            out.write_string(&tag.name).ok()?;
            out.write_var_int(&VarInt(i32::try_from(ids.len()).ok()?))
                .ok()?;
            for id in ids {
                out.write_var_int(&VarInt(id)).ok()?;
            }
        }
    }
    Some(out)
}
