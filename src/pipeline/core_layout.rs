use std::collections::HashMap;
use std::sync::OnceLock;

use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::protocol::packet_key;
use crate::packet::mappings::{PacketId, clientbound};

/// Lowest version core writes a correct payload for. Packets with no entry
/// are written in 26.3 layout.
#[must_use]
pub fn core_layout_floor(packet: &'static PacketId) -> JavaMinecraftVersion {
    table()
        .get(&packet_key(packet))
        .copied()
        .unwrap_or(JavaMinecraftVersion::V_26_3)
}

fn table() -> &'static HashMap<usize, JavaMinecraftVersion> {
    static TABLE: OnceLock<HashMap<usize, JavaMinecraftVersion>> = OnceLock::new();
    TABLE.get_or_init(|| {
        use JavaMinecraftVersion as V;
        let mut table = HashMap::new();
        let mut put = |packet: &'static PacketId, floor: V| {
            table.insert(packet_key(packet), floor);
        };

        // net/java/chunk_data/mod.rs: v1_16 below 1.18, v1_18 from it.
        put(&clientbound::play::LEVEL_CHUNK_WITH_LIGHT, V::V_1_16_2);
        // java/client/play/block_update.rs: no branch beyond write_block_pos, which packs 1.14 style from 1.14.
        put(&clientbound::play::BLOCK_UPDATE, V::V_1_14);
        // java/client/play/multi_block_update.rs: branches at 1.16 and 1.7.6.
        put(&clientbound::play::SECTION_BLOCKS_UPDATE, V::V_1_7_2);
        // java/client/play/worldevent.rs: no branch beyond write_block_pos.
        put(&clientbound::play::LEVEL_EVENT, V::V_1_14);
        // java/client/play/spawn_entity.rs: branches at 1.9, 1.14, 1.19 and 1.21.9.
        put(&clientbound::play::ADD_ENTITY, V::V_1_7_2);
        // java/client/play/remove_entities.rs: the 1.17 single id and the 1.7.6 form.
        put(&clientbound::play::REMOVE_ENTITIES, V::V_1_7_2);
        // java/client/play/update_tags.rs: fixed lists below 1.17, nothing below 1.13.
        put(&clientbound::play::UPDATE_TAGS, V::V_1_13);
        // java/client/config/update_tags.rs: no branch; the configuration state starts at 1.20.2.
        put(&clientbound::config::UPDATE_TAGS, V::V_1_20_2);
        // net/java/config/known_packs.rs: the bundle below 1.20.5, one packet per registry from it.
        put(&clientbound::config::REGISTRY_DATA, V::V_1_20_2);
        // java/client/status/status_response.rs: one json string on every version.
        put(&clientbound::status::STATUS_RESPONSE, V::V_1_7_2);
        // java/client/play/login.rs: branches from 1.8 up.
        put(&clientbound::play::LOGIN, V::V_1_7_2);
        // java/client/play/respawn.rs: branches from 1.14 up, older arm below.
        put(&clientbound::play::RESPAWN, V::V_1_7_2);
        // java/client/play/reset_score.rs: no branch; the packet starts at 1.20.3.
        put(&clientbound::play::RESET_SCORE, V::V_1_20_3);
        // java/client/play/update_score.rs: branches at 1.20.3 and 1.7.6.
        put(&clientbound::play::SET_SCORE, V::V_1_7_2);
        // java/client/play/entity_metadata.rs: the 1.8 entity id, and Metadata::write branches at 1.9.
        put(&clientbound::play::SET_ENTITY_DATA, V::V_1_7_2);
        // java/client/play/update_attributes.rs: branches at 1.7.6, 1.16, 1.17, 1.20.5 and 1.21.
        put(&clientbound::play::UPDATE_ATTRIBUTES, V::V_1_7_2);
        // java/client/play/recipe_book_add.rs: no branch on the container.
        put(&clientbound::play::RECIPE_BOOK_ADD, V::V_26_3);

        table
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packet_with_no_entry_is_native() {
        assert_eq!(
            core_layout_floor(&clientbound::play::SET_TIME),
            JavaMinecraftVersion::V_26_3
        );
        assert_eq!(
            core_layout_floor(&clientbound::play::LEVEL_CHUNK_WITH_LIGHT),
            JavaMinecraftVersion::V_1_16_2
        );
    }
}
