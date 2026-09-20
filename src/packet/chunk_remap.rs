//! Remaps block state ids inside a `LEVEL_CHUNK_WITH_LIGHT` payload.
//!
//! Core writes the chunk in the client's own wire layout (NBT heightmaps and
//! length prefixed packed arrays before 1.21.5, the fluid count from 26.1), but
//! the block state ids inside the section palettes are always 26.3's. Those
//! shift between versions, and a client throws `No value with id <n>` when a
//! section palette names a state its own registry does not have, so the
//! palettes are renumbered here.
//!
//! Only the block palette is touched. Biome ids index the biome registry that
//! was negotiated during configuration, and the surrounding bytes (heightmaps,
//! block entities, light) are copied through untouched.
//!
//! Two shapes are parsed. From 1.18 a chunk is heightmaps plus a blob of
//! sections that each carry a block *and* a biome palette container, with no
//! bit mask. From 1.16.2 to 1.17.1 the blob holds block containers only, the
//! sections present are named by a primary bit mask before the heightmaps (a
//! varint on 1.16, a long array on 1.17), the biomes are a separate
//! varint-prefixed varint array, and block entities are full NBT compounds
//! rather than the compact form. Sources: minecraft-data
//! `pc/1.16.5` and `pc/1.17.1` `packet_map_chunk`, and prismarine-chunk
//! `src/pc/1.16/ChunkColumn.js` / `src/pc/1.17/ChunkColumn.js` for the section
//! body, which is identical on the two.
//!
//! The layout branches here are 1.20.2 (NBT root name), 1.21.5 (heightmap
//! list, no length prefixes) and 26.1 (fluid count). 1.20.2, 1.20.3, 1.20.5
//! and 1.21 all sit between the same pair of branches, so those eras parse
//! identically: unnamed NBT heightmaps and length prefixed packed arrays, no
//! fluid count. minecraft-data agrees (`map_chunk` is the same container on
//! 1.20.2, 1.20.3, 1.20.6 and 1.21.1, with `heightmaps` typed `anonymousNbt`
//! from 1.20.2 and `nbt` on 1.20.1 and below), and so does core:
//! `crates/pumpkin/src/net/java/chunk_data/v1_18.rs` branches at 1.21.4/1.21.5
//! and 26.1 and nowhere between 764 and 767, and
//! `NetworkWriteExt::write_nbt_with_version` writes the empty root name only
//! below `V_1_20_2`, which is the same boundary `copy_heightmaps` uses.

use std::io::Cursor;

use pumpkin_nbt::deserializer::NbtReadHelperJava;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::ser::{NetworkReadExt, NetworkWriteExt};
use pumpkin_util::version::JavaMinecraftVersion;

use crate::remap::block_state_remap::remap_block_state_for_version;

/// Oldest version whose chunk layout this parser understands. 1.16.2 is where
/// the biome array gained its varint length prefix; 1.16 and 1.16.1 send a bare
/// 1024 entries and are not handled here.
pub const OLDEST_LAYOUT: JavaMinecraftVersion = JavaMinecraftVersion::V_1_16_2;

/// First version whose sections carry biomes and drop the primary bit mask.
const FIRST_SECTION_BIOMES: JavaMinecraftVersion = JavaMinecraftVersion::V_1_18;

/// Highest `bits_per_entry` that still uses an indirect (listed) block palette.
/// Above this the section uses the direct palette and stores raw ids.
const MAX_INDIRECT_BLOCK_BITS: u8 = 8;
/// Same threshold for biome containers, which hold 4x4x4 entries.
const MAX_INDIRECT_BIOME_BITS: u8 = 3;
/// Block entries in a section.
const BLOCKS_PER_SECTION: usize = 16 * 16 * 16;
/// Biome entries in a section, from 1.18.
const BIOMES_PER_SECTION: usize = 4 * 4 * 4;

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
/// Before 1.21.5 the packed data array carries a `VarInt` length; from 1.21.5
/// the length is implied by the entry count and bit width.
///
/// The single-value palette (`bits_per_entry` 0 followed by one id and no
/// packed data) arrives with 1.18. Below that, vanilla's `PalettedContainer`
/// rounds any width of 4 or less up to 4 and reads a palette *list* there, so a
/// width of 0 is parsed as an indirect palette, exactly as the client does.
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
    version: JavaMinecraftVersion,
) -> Option<()> {
    let length_prefixed = version < JavaMinecraftVersion::V_1_21_5;
    let bits_per_entry = cursor.get_u8().ok()?;
    out.write_u8(bits_per_entry).ok()?;

    if bits_per_entry == 0 && version >= FIRST_SECTION_BIOMES {
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

    let longs = if length_prefixed {
        let len = cursor.get_var_int().ok()?.0;
        out.write_var_int(&VarInt(len)).ok()?;
        usize::try_from(len).ok()?
    } else {
        packed_long_count(entry_count, bits_per_entry)
    };
    for _ in 0..longs {
        let packed = cursor.get_i64_be().ok()?;
        out.write_i64_be(packed).ok()?;
    }

    Some(())
}

/// Copies the heightmaps as `version` frames them: a list of (type, long array)
/// from 1.21.5, a network NBT compound before that (with a root name before
/// 1.20.2, which is where core's `write_nbt_with_version` drops it too, so
/// 764 and 765 take the unnamed branch).
fn copy_heightmaps(
    cursor: &mut &[u8],
    out: &mut Vec<u8>,
    version: JavaMinecraftVersion,
) -> Option<()> {
    if version >= JavaMinecraftVersion::V_1_21_5 {
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
        return Some(());
    }

    let start = *cursor;
    let tag_id = cursor.get_u8().ok()?;
    if tag_id != 0 {
        if version < JavaMinecraftVersion::V_1_20_2 {
            // Named root: u16 length followed by the name bytes.
            let name_len = usize::try_from(cursor.get_i16_be().ok()?).ok()?;
            if cursor.len() < name_len {
                return None;
            }
            *cursor = &cursor[name_len..];
        }
        let mut nbt_cursor = Cursor::new(*cursor);
        let mut reader = NbtReadHelperJava::new(&mut nbt_cursor);
        NbtTag::skip_data(&mut reader, tag_id).ok()?;
        let used = usize::try_from(nbt_cursor.position()).ok()?;
        *cursor = &cursor[used..];
    }
    let consumed = start.len() - cursor.len();
    out.extend_from_slice(&start[..consumed]);
    Some(())
}

/// Rewrites the section blob of a chunk packet for `version`.
///
/// Returns `None` when the payload cannot be handled, in which case the caller
/// must drop the packet rather than send it with 26.3 state ids.
#[must_use]
pub fn remap_chunk_payload(payload: &[u8], version: JavaMinecraftVersion) -> Option<Vec<u8>> {
    if version < OLDEST_LAYOUT {
        return None;
    }
    if version < FIRST_SECTION_BIOMES {
        return remap_legacy_chunk_payload(payload, version);
    }

    let mut cursor = payload;
    let mut out = Vec::with_capacity(payload.len());

    // Chunk position.
    let chunk_x = cursor.get_i32_be().ok()?;
    let chunk_z = cursor.get_i32_be().ok()?;
    out.write_i32_be(chunk_x).ok()?;
    out.write_i32_be(chunk_z).ok()?;

    copy_heightmaps(&mut cursor, &mut out, version)?;

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
        // not section data, so it is copied through untouched. An all-zero
        // tail on any other version is a run of empty air sections, which the
        // copy preserves as well.
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
            BLOCKS_PER_SECTION,
            MAX_INDIRECT_BLOCK_BITS,
            Some(version),
            version,
        )?;
        copy_container(
            &mut section_cursor,
            &mut sections_out,
            BIOMES_PER_SECTION,
            MAX_INDIRECT_BIOME_BITS,
            None,
            version,
        )?;
    }

    out.write_var_int(&VarInt(i32::try_from(sections_out.len()).ok()?))
        .ok()?;
    out.extend_from_slice(&sections_out);
    // Block entities and light data are version independent here.
    out.extend_from_slice(rest);

    Some(out)
}

/// Copies the primary bit mask and returns how many sections follow it.
///
/// 1.16 sends a "full chunk" flag and the mask as a single varint;
/// 1.17 replaced both with a `BitSet` (varint length plus that many `i64`s),
/// which is what minecraft-data types as `["array", {"countType": "varint",
/// "type": "i64"}]` for `bitMap` on `pc/1.17.1`.
fn copy_section_mask(
    cursor: &mut &[u8],
    out: &mut Vec<u8>,
    version: JavaMinecraftVersion,
) -> Option<usize> {
    if version < JavaMinecraftVersion::V_1_17 {
        let full_chunk = cursor.get_bool().ok()?;
        out.write_bool(full_chunk).ok()?;
        // A partial chunk carries neither biomes nor a full section set; core
        // never sends one and this parser does not model it.
        if !full_chunk {
            return None;
        }
        let mask = cursor.get_var_int().ok()?.0;
        out.write_var_int(&VarInt(mask)).ok()?;
        return Some((mask as u32).count_ones() as usize);
    }

    let words = cursor.get_var_int().ok()?.0;
    out.write_var_int(&VarInt(words)).ok()?;
    let mut sections = 0usize;
    for _ in 0..words {
        let word = cursor.get_i64_be().ok()?;
        out.write_i64_be(word).ok()?;
        sections += (word as u64).count_ones() as usize;
    }
    Some(sections)
}

/// Copies one full NBT tag (a type byte, a root name, then the body), which is
/// how 1.16 and 1.17 frame every block entity in the chunk packet.
fn copy_named_nbt(cursor: &mut &[u8], out: &mut Vec<u8>) -> Option<()> {
    let start = *cursor;
    let tag_id = cursor.get_u8().ok()?;
    if tag_id != 0 {
        let name_len = usize::try_from(cursor.get_i16_be().ok()?).ok()?;
        if cursor.len() < name_len {
            return None;
        }
        *cursor = &cursor[name_len..];
        let body = *cursor;
        let mut nbt_cursor = Cursor::new(body);
        let mut reader = NbtReadHelperJava::new(&mut nbt_cursor);
        NbtTag::skip_data(&mut reader, tag_id).ok()?;
        let used = usize::try_from(nbt_cursor.position()).ok()?;
        // `NbtCompound::skip_content` ends a compound on EOF as well as on
        // TAG_End, so a payload cut off inside the last block entity would skip
        // clean. A complete compound always ends on its TAG_End byte; if the
        // last byte consumed is not one, the tag was truncated.
        if tag_id == 10 && body.get(used.checked_sub(1)?) != Some(&0) {
            return None;
        }
        *cursor = &cursor[used..];
    }
    let consumed = start.len() - cursor.len();
    out.extend_from_slice(&start[..consumed]);
    Some(())
}

/// Rewrites a 1.16.2 to 1.17.1 chunk packet.
///
/// Layout: position, the primary bit mask, named-root heightmaps NBT, a
/// varint-prefixed array of varint biomes, the length-prefixed section blob and
/// a varint-prefixed array of full NBT block entities. Light rides in its own
/// `LIGHT_UPDATE` packet on every version here, so the block entity list is the
/// end of the payload and anything after it is rejected.
fn remap_legacy_chunk_payload(payload: &[u8], version: JavaMinecraftVersion) -> Option<Vec<u8>> {
    let mut cursor = payload;
    let mut out = Vec::with_capacity(payload.len());

    let chunk_x = cursor.get_i32_be().ok()?;
    let chunk_z = cursor.get_i32_be().ok()?;
    out.write_i32_be(chunk_x).ok()?;
    out.write_i32_be(chunk_z).ok()?;

    let section_count = copy_section_mask(&mut cursor, &mut out, version)?;
    copy_heightmaps(&mut cursor, &mut out, version)?;

    // Biomes index the registry the client was given in the join packet, so
    // they are copied through; only block state ids move between versions.
    let biome_count = cursor.get_var_int().ok()?.0;
    out.write_var_int(&VarInt(biome_count)).ok()?;
    for _ in 0..biome_count {
        let biome = cursor.get_var_int().ok()?.0;
        out.write_var_int(&VarInt(biome)).ok()?;
    }

    let data_len = usize::try_from(cursor.get_var_int().ok()?.0).ok()?;
    if data_len > cursor.len() {
        return None;
    }
    let (sections, rest) = cursor.split_at(data_len);

    let mut section_cursor = sections;
    let mut sections_out = Vec::with_capacity(sections.len());
    for _ in 0..section_count {
        let block_count = section_cursor.get_i16_be().ok()?;
        sections_out.write_i16_be(block_count).ok()?;
        // No biome container before 1.18: the block palette is the whole
        // section.
        copy_container(
            &mut section_cursor,
            &mut sections_out,
            BLOCKS_PER_SECTION,
            MAX_INDIRECT_BLOCK_BITS,
            Some(version),
            version,
        )?;
    }
    if !section_cursor.is_empty() {
        return None;
    }

    out.write_var_int(&VarInt(i32::try_from(sections_out.len()).ok()?))
        .ok()?;
    out.extend_from_slice(&sections_out);

    let mut cursor = rest;
    let block_entities = cursor.get_var_int().ok()?.0;
    out.write_var_int(&VarInt(block_entities)).ok()?;
    for _ in 0..block_entities {
        copy_named_nbt(&mut cursor, &mut out)?;
    }
    if !cursor.is_empty() {
        return None;
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two sections in the 1.21.4 layout (empty NBT heightmaps, length
    /// prefixed arrays): one single-value stone section, one indirect palette
    /// with air and stone.
    fn chunk_1_21_4(stone: i32) -> Vec<u8> {
        let mut sections = Vec::new();
        // Section 1: 4096 blocks, single palette.
        sections.write_i16_be(4096).unwrap();
        sections.write_u8(0).unwrap();
        sections.write_var_int(&VarInt(stone)).unwrap();
        sections.write_var_int(&VarInt(0)).unwrap(); // packed data length
        sections.write_u8(0).unwrap(); // biome single palette
        sections.write_var_int(&VarInt(1)).unwrap();
        sections.write_var_int(&VarInt(0)).unwrap();
        // Section 2: indirect palette [air, stone], 4 bits per entry.
        sections.write_i16_be(1).unwrap();
        sections.write_u8(4).unwrap();
        sections.write_var_int(&VarInt(2)).unwrap();
        sections.write_var_int(&VarInt(0)).unwrap();
        sections.write_var_int(&VarInt(stone)).unwrap();
        sections.write_var_int(&VarInt(256)).unwrap();
        for _ in 0..256 {
            sections.write_i64_be(0).unwrap();
        }
        sections.write_u8(0).unwrap();
        sections.write_var_int(&VarInt(1)).unwrap();
        sections.write_var_int(&VarInt(0)).unwrap();

        let mut payload = Vec::new();
        payload.write_i32_be(3).unwrap();
        payload.write_i32_be(-4).unwrap();
        // Empty unnamed compound.
        payload.write_u8(10).unwrap();
        payload.write_u8(0).unwrap();
        payload
            .write_var_int(&VarInt(i32::try_from(sections.len()).unwrap()))
            .unwrap();
        payload.extend_from_slice(&sections);
        // Block entities: none. Light: stand-in bytes for the test.
        payload.write_var_int(&VarInt(0)).unwrap();
        payload.extend_from_slice(&[7, 7, 7]);
        payload
    }

    #[test]
    fn remaps_palettes_in_the_1_21_4_layout() {
        let stone = i32::from(pumpkin_data::Block::STONE.default_state.id.as_u16());
        let payload = chunk_1_21_4(stone);
        let out = remap_chunk_payload(&payload, JavaMinecraftVersion::V_1_21_4)
            .expect("1.21.4 chunk must parse");
        assert_eq!(&out[out.len() - 3..], &[7, 7, 7], "tail copied through");

        let mapped = i32::from(remap_block_state_for_version(
            u16::try_from(stone).unwrap(),
            JavaMinecraftVersion::V_1_21_4,
        ));
        // Position, empty NBT (2 bytes), blob length, block count.
        let mut cursor = &out[8 + 2..];
        let _blob_len = cursor.get_var_int().unwrap();
        assert_eq!(cursor.get_i16_be().unwrap(), 4096);
        assert_eq!(cursor.get_u8().unwrap(), 0);
        assert_eq!(cursor.get_var_int().unwrap().0, mapped);
    }

    /// 1.20.5 shares the 1.21 layout exactly, so the same bytes parse and the
    /// palette is renumbered with that version's block state table.
    #[test]
    fn remaps_palettes_in_the_1_20_5_layout() {
        let stone = i32::from(pumpkin_data::Block::STONE.default_state.id.as_u16());
        let payload = chunk_1_21_4(stone);
        let out = remap_chunk_payload(&payload, JavaMinecraftVersion::V_1_20_5)
            .expect("1.20.5 chunk must parse");
        assert_eq!(&out[out.len() - 3..], &[7, 7, 7], "tail copied through");

        let mapped = i32::from(remap_block_state_for_version(
            u16::try_from(stone).unwrap(),
            JavaMinecraftVersion::V_1_20_5,
        ));
        let mut cursor = &out[8 + 2..];
        let _blob_len = cursor.get_var_int().unwrap();
        assert_eq!(cursor.get_i16_be().unwrap(), 4096);
        assert_eq!(cursor.get_u8().unwrap(), 0);
        assert_eq!(cursor.get_var_int().unwrap().0, mapped);
    }

    /// 1.20.2 and 1.20.3 share the 1.21 layout too: the heightmaps root is
    /// unnamed from 1.20.2 (minecraft-data types it `anonymousNbt` there and
    /// `nbt` on 1.20.1), the packed arrays still carry their length prefix and
    /// there is no fluid count.
    #[test]
    fn remaps_palettes_in_the_1_20_2_and_1_20_3_layouts() {
        let stone = i32::from(pumpkin_data::Block::STONE.default_state.id.as_u16());
        let payload = chunk_1_21_4(stone);
        for version in [
            JavaMinecraftVersion::V_1_20_2,
            JavaMinecraftVersion::V_1_20_3,
        ] {
            let out = remap_chunk_payload(&payload, version).expect("1.20.2/1.20.3 chunk parses");
            assert_eq!(&out[out.len() - 3..], &[7, 7, 7], "tail copied through");

            let mapped = i32::from(remap_block_state_for_version(
                u16::try_from(stone).unwrap(),
                version,
            ));
            let mut cursor = &out[8 + 2..];
            let _blob_len = cursor.get_var_int().unwrap();
            assert_eq!(cursor.get_i16_be().unwrap(), 4096);
            assert_eq!(cursor.get_u8().unwrap(), 0);
            assert_eq!(cursor.get_var_int().unwrap().0, mapped);
        }
    }

    /// The unnamed heightmaps root starts at 1.20.2: the very same bytes are
    /// not a valid chunk for 1.20, where the parser reads a root name first.
    /// This pins the `version < V_1_20_2` branch to the same boundary core's
    /// `write_nbt_with_version` uses.
    #[test]
    fn unnamed_heightmap_root_starts_at_1_20_2() {
        // Heightmaps as core writes them below 1.21.5: a compound with one
        // long array, root unnamed.
        let mut heightmaps = Vec::new();
        heightmaps.write_u8(10).unwrap(); // TAG_Compound
        heightmaps.write_u8(12).unwrap(); // TAG_Long_Array
        heightmaps.write_i16_be(15).unwrap();
        heightmaps.extend_from_slice(b"MOTION_BLOCKING");
        heightmaps.write_i32_be(2).unwrap();
        heightmaps.write_i64_be(0).unwrap();
        heightmaps.write_i64_be(0).unwrap();
        heightmaps.write_u8(0).unwrap(); // TAG_End

        let empty = chunk_1_21_4(1);
        let mut payload = Vec::new();
        payload.extend_from_slice(&empty[..8]);
        payload.extend_from_slice(&heightmaps);
        // Skip the empty compound (2 bytes) of the fixture.
        payload.extend_from_slice(&empty[8 + 2..]);

        for version in [
            JavaMinecraftVersion::V_1_20_2,
            JavaMinecraftVersion::V_1_20_3,
            JavaMinecraftVersion::V_1_20_5,
        ] {
            assert!(
                remap_chunk_payload(&payload, version).is_some(),
                "{version} reads an unnamed heightmaps root"
            );
        }
        assert!(
            remap_chunk_payload(&payload, JavaMinecraftVersion::V_1_20).is_none(),
            "1.20 expects a root name and must reject these bytes"
        );
    }

    /// One section holding an [air, stone] palette, in the 1.16.2 or 1.17
    /// layout: position, primary bit mask, named-root heightmaps, the 1024
    /// biome varints, the section blob and one full-NBT block entity.
    fn chunk_legacy(stone: i32, version: JavaMinecraftVersion) -> Vec<u8> {
        let mut sections = Vec::new();
        sections.write_i16_be(1).unwrap();
        sections.write_u8(4).unwrap();
        sections.write_var_int(&VarInt(2)).unwrap();
        sections.write_var_int(&VarInt(0)).unwrap();
        sections.write_var_int(&VarInt(stone)).unwrap();
        sections.write_var_int(&VarInt(256)).unwrap();
        for _ in 0..256 {
            sections.write_i64_be(0).unwrap();
        }

        let mut payload = Vec::new();
        payload.write_i32_be(3).unwrap();
        payload.write_i32_be(-4).unwrap();
        if version < JavaMinecraftVersion::V_1_17 {
            payload.write_bool(true).unwrap(); // full chunk
            payload.write_var_int(&VarInt(1)).unwrap(); // section 0 only
        } else {
            payload.write_var_int(&VarInt(1)).unwrap(); // one mask word
            payload.write_i64_be(1).unwrap();
        }
        // Heightmaps: an empty compound with a named (empty) root.
        payload.write_u8(10).unwrap();
        payload.write_i16_be(0).unwrap();
        payload.write_u8(0).unwrap();
        // Biomes.
        payload.write_var_int(&VarInt(1024)).unwrap();
        for _ in 0..1024 {
            payload.write_var_int(&VarInt(1)).unwrap();
        }
        payload
            .write_var_int(&VarInt(i32::try_from(sections.len()).unwrap()))
            .unwrap();
        payload.extend_from_slice(&sections);
        // One block entity as a full NBT compound: {id: "minecraft:chest"}.
        payload.write_var_int(&VarInt(1)).unwrap();
        payload.write_u8(10).unwrap();
        payload.write_i16_be(0).unwrap();
        payload.write_u8(8).unwrap(); // TAG_String
        payload.write_i16_be(2).unwrap();
        payload.extend_from_slice(b"id");
        payload.write_i16_be(15).unwrap();
        payload.extend_from_slice(b"minecraft:chest");
        payload.write_u8(0).unwrap(); // TAG_End
        payload
    }

    /// Walks a rewritten legacy chunk and returns the section palette.
    fn legacy_palette(out: &[u8], version: JavaMinecraftVersion) -> Vec<i32> {
        let mut cursor = out;
        cursor.get_i32_be().unwrap();
        cursor.get_i32_be().unwrap();
        if version < JavaMinecraftVersion::V_1_17 {
            assert!(cursor.get_bool().unwrap());
            assert_eq!(cursor.get_var_int().unwrap().0, 1);
        } else {
            assert_eq!(cursor.get_var_int().unwrap().0, 1);
            assert_eq!(cursor.get_i64_be().unwrap(), 1);
        }
        assert_eq!(cursor.get_u8().unwrap(), 10);
        assert_eq!(cursor.get_i16_be().unwrap(), 0);
        assert_eq!(cursor.get_u8().unwrap(), 0);
        assert_eq!(cursor.get_var_int().unwrap().0, 1024);
        for _ in 0..1024 {
            cursor.get_var_int().unwrap();
        }
        let blob_len = usize::try_from(cursor.get_var_int().unwrap().0).unwrap();
        let (blob, rest) = cursor.split_at(blob_len);
        let mut section = blob;
        assert_eq!(section.get_i16_be().unwrap(), 1);
        assert_eq!(section.get_u8().unwrap(), 4);
        let len = section.get_var_int().unwrap().0;
        let palette: Vec<i32> = (0..len).map(|_| section.get_var_int().unwrap().0).collect();
        assert_eq!(section.get_var_int().unwrap().0, 256);
        for _ in 0..256 {
            section.get_i64_be().unwrap();
        }
        assert!(section.is_empty(), "section blob fully consumed");

        // The block entity list is copied through byte for byte.
        let mut cursor = rest;
        assert_eq!(cursor.get_var_int().unwrap().0, 1);
        assert_eq!(cursor.get_u8().unwrap(), 10);
        palette
    }

    /// 1.16.2 through 1.16.5 and 1.17 through 1.17.1: every palette entry is
    /// renumbered with that version's block state table, everything else is
    /// copied through.
    #[test]
    fn remaps_palettes_in_the_1_16_and_1_17_layouts() {
        let stone = i32::from(pumpkin_data::Block::STONE.default_state.id.as_u16());
        for version in [
            JavaMinecraftVersion::V_1_16_2,
            JavaMinecraftVersion::V_1_16_3,
            JavaMinecraftVersion::V_1_16_4,
            JavaMinecraftVersion::V_1_17,
            JavaMinecraftVersion::V_1_17_1,
        ] {
            let payload = chunk_legacy(stone, version);
            let out = remap_chunk_payload(&payload, version)
                .unwrap_or_else(|| panic!("{version} chunk must parse"));

            let palette = legacy_palette(&out, version);
            let air = i32::from(remap_block_state_for_version(0, version));
            let mapped = i32::from(remap_block_state_for_version(
                u16::try_from(stone).unwrap(),
                version,
            ));
            assert_eq!(palette, vec![air, mapped], "{version} palette renumbered");
        }
    }

    /// The 1.16 full-chunk flag is real: the same bytes are not a 1.17 packet,
    /// where the first field after the position is the mask's word count.
    #[test]
    fn the_full_chunk_flag_ends_at_1_17() {
        let payload = chunk_legacy(1, JavaMinecraftVersion::V_1_16_2);
        assert!(remap_chunk_payload(&payload, JavaMinecraftVersion::V_1_17).is_none());

        let payload = chunk_legacy(1, JavaMinecraftVersion::V_1_17);
        assert!(remap_chunk_payload(&payload, JavaMinecraftVersion::V_1_16_2).is_none());
    }

    /// 1.18 moved biomes into the sections and dropped the bit mask, so a
    /// 1.18-shaped payload is not a 1.17 one.
    ///
    /// The reverse is not asserted: the 1.18 parser copies everything after the
    /// sections through untouched, so it cannot tell a 1.17 tail apart. Which
    /// branch runs is decided by the client's version, not by the bytes.
    #[test]
    fn the_section_biome_container_starts_at_1_18() {
        let payload = chunk_1_21_4(1);
        assert!(remap_chunk_payload(&payload, JavaMinecraftVersion::V_1_17_1).is_none());
    }

    #[test]
    fn rejects_layouts_older_than_1_16_2() {
        let payload = chunk_legacy(1, JavaMinecraftVersion::V_1_16_2);
        assert!(remap_chunk_payload(&payload, JavaMinecraftVersion::V_1_16_1).is_none());
        assert!(remap_chunk_payload(&payload, JavaMinecraftVersion::V_1_15_2).is_none());
    }

    #[test]
    fn truncated_or_padded_legacy_payload_is_rejected() {
        for version in [
            JavaMinecraftVersion::V_1_16_2,
            JavaMinecraftVersion::V_1_17_1,
        ] {
            let payload = chunk_legacy(1, version);
            for cut in [9, 14, 40, 1100, payload.len() - 1] {
                assert!(
                    remap_chunk_payload(&payload[..cut], version).is_none(),
                    "{version} cut at {cut}"
                );
            }
            let mut trailing = payload.clone();
            trailing.push(0xff);
            assert!(
                remap_chunk_payload(&trailing, version).is_none(),
                "{version} trailing byte"
            );
        }
    }

    #[test]
    fn truncated_payload_is_rejected_not_forwarded() {
        let payload = chunk_1_21_4(1);
        for cut in [9, 12, 20, payload.len() - 40] {
            assert!(
                remap_chunk_payload(&payload[..cut], JavaMinecraftVersion::V_1_21_4).is_none(),
                "cut at {cut}"
            );
        }
    }
}
