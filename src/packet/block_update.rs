//! Remaps block state ids in the block-change packets.
//!
//! Chunk palettes are remapped when a chunk is sent, but every later change to
//! the world arrives through these packets carrying a raw state id. Only 27 of
//! the 35,723 states keep the same id between 26.3 and 26.2, so leaving these
//! alone makes the client render the wrong block for anything that changes
//! after load, and its view of the world drifts away from the server's.

use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::codec::var_long::VarLong;
use pumpkin_protocol::ser::{NetworkReadExt, NetworkWriteExt};
use pumpkin_util::version::JavaMinecraftVersion;

use crate::remap::block_state_remap::remap_block_state_for_version;

/// `BLOCK_UPDATE`: block position followed by the new state id.
#[must_use]
pub fn remap_block_update(payload: &[u8], version: JavaMinecraftVersion) -> Option<Vec<u8>> {
    let mut cursor = payload;
    let position = cursor.get_i64_be().ok()?;
    let state_id = cursor.get_var_int().ok()?.0;

    let remapped = remap_block_state_for_version(u16::try_from(state_id).ok()?, version);

    let mut out = Vec::with_capacity(payload.len());
    out.write_i64_be(position).ok()?;
    out.write_var_int(&VarInt(i32::from(remapped))).ok()?;
    Some(out)
}

/// `SECTION_BLOCKS_UPDATE`: section position, then one varlong per change
/// packing the state id in the high bits and the local position in the low 12.
#[must_use]
pub fn remap_section_blocks_update(
    payload: &[u8],
    version: JavaMinecraftVersion,
) -> Option<Vec<u8>> {
    let mut cursor = payload;
    let section = cursor.get_i64_be().ok()?;
    let count = cursor.get_var_int().ok()?.0;

    let mut out = Vec::with_capacity(payload.len());
    out.write_i64_be(section).ok()?;
    out.write_var_int(&VarInt(count)).ok()?;

    for _ in 0..count {
        let packed = cursor.get_var_long().ok()?.0 as u64;
        let local_pos = packed & 0xFFF;
        let state_id = u16::try_from(packed >> 12).ok()?;
        let remapped = remap_block_state_for_version(state_id, version);
        let repacked = (u64::from(remapped) << 12) | local_pos;
        out.write_var_long(&VarLong(repacked as i64)).ok()?;
    }

    Some(out)
}

/// Level event id of the block-break particle burst, whose data is a state id.
const PARTICLES_DESTROY_BLOCK: i32 = 2001;

/// `LEVEL_EVENT`: event id, block position, event data, global flag. Only the
/// destroy-block event carries a state id in its data; everything else is
/// passed through unchanged.
#[must_use]
pub fn remap_level_event(payload: &[u8], version: JavaMinecraftVersion) -> Option<Vec<u8>> {
    let mut cursor = payload;
    let event = cursor.get_i32_be().ok()?;
    if event != PARTICLES_DESTROY_BLOCK {
        return None;
    }
    let position = cursor.get_i64_be().ok()?;
    let data = cursor.get_i32_be().ok()?;
    let global = cursor.get_bool().ok()?;

    let remapped = remap_block_state_for_version(u16::try_from(data).ok()?, version);

    let mut out = Vec::with_capacity(payload.len());
    out.write_i32_be(event).ok()?;
    out.write_i64_be(position).ok()?;
    out.write_i32_be(i32::from(remapped)).ok()?;
    out.write_bool(global).ok()?;
    Some(out)
}
