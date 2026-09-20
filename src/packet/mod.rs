use pumpkin_util::version::JavaMinecraftVersion;

pub mod block_update;
pub mod chunk_remap;
pub mod entity;
pub mod join;
pub mod legacy;
pub mod mappings;
pub mod score;
pub mod status;
pub mod translator;
pub mod update_tags;

/// Oldest client version this plugin admits.
///
/// Support is decided per tier, newest first, and a tier is only added here
/// once its probes and a real client pass are green. A client below the floor
/// is refused at login with a clear message rather than let in on packets
/// that are only partly translated, which is how clients crash.
///
/// Tier 4 (protocols 751 to 763, the 1.16.2 to 1.20.1 clients) is the floor.
/// This is the first tier whose clients differ structurally from 26.3 rather
/// than only in packet numbering, so the split between "byte identical" and
/// "rewritten" matters here:
///
/// Byte identical to 26.3, only the packet id changes:
/// * `BLOCK_UPDATE` (position, state varint) and `SECTION_BLOCKS_UPDATE`; the
///   latter grew a "suppress light updates" bool that exists up to 1.19.4 and
///   is written by core, not by this plugin.
/// * `SET_SCORE` (owner string, action varint, objective string, value varint
///   unless the action is 1) is the same container on 1.16.5, 1.17.1, 1.18.2,
///   1.19.4, 1.20.1 and 1.20.2 in minecraft-data, so `score::rewrite_reset_score`
///   covers the whole tier with no branch.
/// * The status response is a single JSON string on every version down to
///   1.16.2 (`packet_server_info.response` is typed `string` throughout), so
///   `status::rewrite_status_response` needs no branch either.
/// * `ADD_ENTITY`'s first three fields (varint entity id, uuid, varint type)
///   have been in that order since 1.14, which is all `entity::remap_spawn_entity`
///   reads.
///
/// Rewritten for this tier:
/// * The join codec. There is no configuration state below 1.20.2: after
///   `LOGIN_SUCCESS` the connection goes straight to play and every registry
///   travels as the NBT "dimension codec" inside the play `LOGIN` packet
///   (minecraft-data types `packet_login.dimensionCodec` as `nbt`, a
///   named-root compound, on 1.16.2 through 1.20.1). That lives in
///   `crate::registry`.
/// * Tag shapes. `UPDATE_TAGS` is four fixed lists on 1.16.2 to 1.16.5
///   (`blockTags`, `itemTags`, `fluidTags`, `entityTags`, no registry names on
///   the wire) and a registry-keyed array from 1.17. That lives in
///   `crate::packet::update_tags`.
/// * Chunk layouts. 1.16.2 to 1.16.5 send a full-chunk flag and a varint
///   primary bit mask, 1.17 and 1.17.1 a `BitSet` of `i64`s, and both carry
///   chunk-wide biomes as a varint array and block entities as full named NBT;
///   sections have no biome palette before 1.18. Light is always its own
///   `LIGHT_UPDATE` packet here. That lives in `crate::packet::chunk_remap`.
///   World height is 0 to 255 up to and including 1.17.1: sections outside it
///   are cut, never shifted.
/// * Spawn wrappers below 1.19. `ADD_ENTITY` only covers every entity from
///   1.19; before that living mobs use `SPAWN_LIVING_ENTITY` and paintings
///   `SPAWN_PAINTING`, and `ADD_ENTITY` itself has no head yaw and writes its
///   data field as a big-endian `i32` rather than a varint (minecraft-data
///   `packet_spawn_entity` on 1.16.2, 1.16.5, 1.17.1 and 1.18.2 against 1.19).
///   Core's `CSpawnEntity` branches at 1.19 for exactly that;
///   `crate::packet::legacy` holds the two wrapper packets.
///
/// The packet id table matches minecraft-data exactly for every version in the
/// tier (`tools/probes/audit_packet_ids.py 1.16.2 1.16.5 1.17.1 1.18.2 1.19
/// 1.19.2 1.19.3 1.19.4 1.20.1`: no differences in any state or direction), and
/// a row with no id for the version is dropped rather than sent.
pub const LOWEST_SUPPORTED: JavaMinecraftVersion = JavaMinecraftVersion::V_1_16_2;

/// Newest client version, the server's own.
pub const HIGHEST_SUPPORTED: JavaMinecraftVersion = JavaMinecraftVersion::V_26_3;

/// Returns whether a given Java edition version is supported by this multiversion plugin.
#[must_use]
pub fn is_version_supported(version: JavaMinecraftVersion) -> bool {
    version != JavaMinecraftVersion::Unknown
        && version >= LOWEST_SUPPORTED
        && version <= HIGHEST_SUPPORTED
}
