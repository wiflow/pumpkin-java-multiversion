//! Remaps the entity type id in `ADD_ENTITY`.
//!
//! The translator already has a branch that decodes the whole packet into
//! `CSpawnEntity` to handle older clients that need paintings and falling blocks
//! turned into their own packets. That branch is guarded by the decode
//! succeeding, and when it does not, the packet falls through with a 26.3 entity
//! type id still in it. The client then builds the wrong entity class: a dropped
//! item arrives as an `ItemDisplay`, and the first metadata update kills the
//! connection because the fields do not line up.
//!
//! This rewrites only the type field, which needs no knowledge of the rest of
//! the packet and therefore cannot fail the same way.

use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::ser::{NetworkReadExt, NetworkWriteExt};
use pumpkin_util::version::JavaMinecraftVersion;

use crate::remap::entity_id_remap::remap_entity_id_for_version;

/// Number of bytes in the entity UUID that follows the entity id.
const UUID_LEN: usize = 16;

/// Rewrites the entity type in an `ADD_ENTITY` payload for `version`.
///
/// Layout is `varint entity_id`, `uuid`, `varint type`, then fields this does
/// not need to understand.
#[must_use]
pub fn remap_spawn_entity(payload: &[u8], version: JavaMinecraftVersion) -> Option<Vec<u8>> {
    let mut cursor = payload;
    let entity_id = cursor.get_var_int().ok()?;

    if cursor.len() < UUID_LEN {
        return None;
    }
    let (uuid_bytes, after_uuid) = cursor.split_at(UUID_LEN);
    cursor = after_uuid;

    let type_id = cursor.get_var_int().ok()?.0;
    let remapped = remap_entity_id_for_version(u16::try_from(type_id).ok()?, version);

    let mut out = Vec::with_capacity(payload.len());
    out.write_var_int(&entity_id).ok()?;
    out.extend_from_slice(uuid_bytes);
    out.write_var_int(&VarInt(i32::from(remapped))).ok()?;
    out.extend_from_slice(cursor);
    Some(out)
}
