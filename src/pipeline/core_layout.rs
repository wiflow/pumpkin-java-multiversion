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
        // java/client/play/sound_effect.rs: branches at 1.19.3, 1.19, 1.10 and 1.9.
        put(&clientbound::play::SOUND, V::V_1_7_2);
        // java/client/play/entity_sound_effect.rs: branches at 1.19.3, 1.19 and 1.10.
        put(&clientbound::play::SOUND_ENTITY, V::V_1_7_2);
        // java/client/play/explode.rs: branches at 1.21.9, 1.21.2, 1.20.5, 1.20.3, 1.19.3 and 1.17.
        put(&clientbound::play::EXPLODE, V::V_1_7_2);
        // java/client/play/particle.rs: branches at 26.3, 1.21.4, 1.20.5, 1.19, 1.15, 1.8 and 1.7.6.
        put(&clientbound::play::LEVEL_PARTICLES, V::V_1_7_2);
        // java/client/play/block_entity_data.rs: branches at 1.20.2, 1.18 and 1.8.
        put(&clientbound::play::BLOCK_ENTITY_DATA, V::V_1_7_2);
        // java/client/play/block_event.rs: no branch beyond write_block_pos.
        put(&clientbound::play::BLOCK_EVENT, V::V_1_14);
        // java/client/play/award_stats.rs: no branch; the container is the same from 1.13.
        put(&clientbound::play::AWARD_STATS, V::V_1_13);
        // java/client/play/commands.rs: the identifier form below 1.19; the packet starts at 1.13.
        put(&clientbound::play::COMMANDS, V::V_1_13);
        // java/client/play/open_screen.rs: no branch; the menu is an id from 1.14.
        put(&clientbound::play::OPEN_SCREEN, V::V_1_14);
        // java/client/play/map_item_data.rs: branches at 1.17, 1.14, 1.13 and 1.9.
        put(&clientbound::play::MAP_ITEM_DATA, V::V_1_7_2);
        // java/client/play/recipe_book_add.rs: no branch on the container.
        put(&clientbound::play::RECIPE_BOOK_ADD, V::V_26_3);
        // java/client/play/set_container_content.rs: branches at 1.17.1 and 1.21.2.
        put(&clientbound::play::CONTAINER_SET_CONTENT, V::V_1_7_2);
        // java/client/play/set_container_slot.rs: the same two branches.
        put(&clientbound::play::CONTAINER_SET_SLOT, V::V_1_7_2);
        // java/client/play/set_cursor_slot.rs: the stack alone, no branch.
        put(&clientbound::play::SET_CURSOR_ITEM, V::V_1_7_2);
        // java/client/play/set_player_inventory.rs: a slot and the stack, no branch.
        put(&clientbound::play::SET_PLAYER_INVENTORY, V::V_1_7_2);
        // java/client/play/set_equipment.rs: branches at 1.7.6, 1.9, 1.16 and 1.20.5.
        put(&clientbound::play::SET_EQUIPMENT, V::V_1_7_2);
        // java/client/play/merchant_offers.rs: branches at 1.19 and 1.20.5.
        put(&clientbound::play::MERCHANT_OFFERS, V::V_1_7_2);
        // java/client/play/item_cooldown.rs: an item id below 1.21.2, a group from it.
        put(&clientbound::play::COOLDOWN, V::V_1_7_2);
        // java/client/play/update_advancement.rs: branches at 1.20, 1.20.2, 1.21.5, 26.1 and 26.3.
        put(&clientbound::play::UPDATE_ADVANCEMENTS, V::V_1_7_2);

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
