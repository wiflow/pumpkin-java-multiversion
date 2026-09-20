//! Remaps block state ids inside a `LEVEL_CHUNK_WITH_LIGHT` payload.
//!
//! For clients from 1.18 onwards the chunk wire format is unchanged, so a chunk
//! only needs its block state ids rewritten: the numeric ids shift between
//! versions, and a client throws `No value with id <n>` when a section palette
//! names a state its own registry does not have.
//!
//! Only the block palette is touched. Biome ids index the biome registry that
//! was negotiated during configuration, and the surrounding bytes (heightmaps,
//! block entities, light) are copied through untouched.

use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::ser::{NetworkReadExt, NetworkWriteExt};
use pumpkin_util::version::JavaMinecraftVersion;

use crate::remap::block_state_remap::remap_block_state_for_version;

/// Highest `bits_per_entry` that still uses an indirect (listed) block palette.
/// Above this the section uses the direct palette and stores raw ids.
const MAX_INDIRECT_BLOCK_BITS: u8 = 8;
/// Same threshold for biome containers, which hold 4x4x4 entries.
const MAX_INDIRECT_BIOME_BITS: u8 = 3;

/// Number of packed longs for `entry_count` entries at `bits_per_entry`.
const fn packed_long_count(entry_count: usize, bits_per_entry: u8) -> usize {
    if bits_per_entry == 0 {
        return 0;
    }
    let per_long = 64 / bits_per_entry as usize;
    entry_count.div_ceil(per_long)
}

/// Copies one palette container, remapping block state ids when `remap` is set.
///
/// Returns `None` for a direct palette, whose packed entries would need
/// unpacking and repacking at the client's own bit width. Chunks containing one
/// are left alone rather than corrupted.
fn copy_container(
    cursor: &mut &[u8],
    out: &mut Vec<u8>,
    entry_count: usize,
    max_indirect_bits: u8,
    remap: Option<JavaMinecraftVersion>,
) -> Option<()> {
    let bits_per_entry = cursor.get_u8().ok()?;
    out.write_u8(bits_per_entry).ok()?;

    if bits_per_entry == 0 {
        let id = cursor.get_var_int().ok()?.0;
        let id = match remap {
            Some(version) => i32::from(remap_block_state_for_version(
                u16::try_from(id).ok()?,
                version,
            )),
            None => id,
        };
        out.write_var_int(&VarInt(id)).ok()?;
    } else if bits_per_entry <= max_indirect_bits {
        let len = cursor.get_var_int().ok()?.0;
        out.write_var_int(&VarInt(len)).ok()?;
        for _ in 0..len {
            let id = cursor.get_var_int().ok()?.0;
            let id = match remap {
                Some(version) => i32::from(remap_block_state_for_version(
                    u16::try_from(id).ok()?,
                    version,
                )),
                None => id,
            };
            out.write_var_int(&VarInt(id)).ok()?;
        }
    } else if remap.is_some() {
        // Direct palette: ids live in the packed data at a width derived from
        // the sender's registry size, so they cannot simply be copied across.
        return None;
    }

    let longs = packed_long_count(entry_count, bits_per_entry);
    for _ in 0..longs {
        let packed = cursor.get_i64_be().ok()?;
        out.write_i64_be(packed).ok()?;
    }

    Some(())
}

/// Rewrites the section blob of a chunk packet for `version`.
///
/// Returns `None` when the payload cannot be handled, in which case the caller
/// should leave the packet untouched.
#[must_use]
pub fn remap_chunk_payload(payload: &[u8], version: JavaMinecraftVersion) -> Option<Vec<u8>> {
    let mut cursor = payload;
    let mut out = Vec::with_capacity(payload.len());

    // Chunk position.
    let chunk_x = cursor.get_i32_be().ok()?;
    let chunk_z = cursor.get_i32_be().ok()?;
    out.write_i32_be(chunk_x).ok()?;
    out.write_i32_be(chunk_z).ok()?;

    // Heightmaps: count, then (index, length, longs) per entry.
    let map_count = cursor.get_var_int().ok()?.0;
    out.write_var_int(&VarInt(map_count)).ok()?;
    for _ in 0..map_count {
        let index = cursor.get_var_int().ok()?.0;
        let len = cursor.get_var_int().ok()?.0;
        out.write_var_int(&VarInt(index)).ok()?;
        out.write_var_int(&VarInt(len)).ok()?;
        for _ in 0..len {
            let val = cursor.get_i64_be().ok()?;
            out.write_i64_be(val).ok()?;
        }
    }

    // Section blob, length prefixed.
    let data_len = usize::try_from(cursor.get_var_int().ok()?.0).ok()?;
    if data_len > cursor.len() {
        return None;
    }
    let (sections, rest) = cursor.split_at(data_len);

    let mut section_cursor = sections;
    let mut sections_out = Vec::with_capacity(sections.len());
    while !section_cursor.is_empty() {
        // Core pads 1.21.5 chunk data with zero bytes after the last section
        // (that client sizes the buffer as if the packed arrays still carried
        // length prefixes). Zeros parse as empty sections until the tail is
        // too short for one, which would fail the whole chunk; the padding is
        // not section data, so it is copied through untouched.
        if section_cursor.iter().all(|&b| b == 0) {
            sections_out.extend_from_slice(section_cursor);
            break;
        }
        let block_count = section_cursor.get_i16_be().ok()?;
        sections_out.write_i16_be(block_count).ok()?;

        if version >= JavaMinecraftVersion::V_26_1 {
            // Fluid count, added in 26.1.
            let liquid_count = section_cursor.get_i16_be().ok()?;
            sections_out.write_i16_be(liquid_count).ok()?;
        }

        copy_container(
            &mut section_cursor,
            &mut sections_out,
            4096,
            MAX_INDIRECT_BLOCK_BITS,
            Some(version),
        )?;
        copy_container(
            &mut section_cursor,
            &mut sections_out,
            64,
            MAX_INDIRECT_BIOME_BITS,
            None,
        )?;
    }

    out.write_var_int(&VarInt(i32::try_from(sections_out.len()).ok()?))
        .ok()?;
    out.extend_from_slice(&sections_out);
    // Block entities and light data are version independent here.
    out.extend_from_slice(rest);

    Some(out)
}
