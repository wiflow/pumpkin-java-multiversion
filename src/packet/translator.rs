use pumpkin_data::entity::EntityType;
use pumpkin_protocol::{
    ClientPacket, MultiVersionJavaPacket, VarInt, java::client::play::CSpawnEntity,
};
use pumpkin_util::{math::position::BlockPos, version::JavaMinecraftVersion};

use crate::packet::legacy::{
    CSpawnExperienceOrb, CSpawnLivingEntity, CSpawnPainting, CSpawnPlayer, DEFAULT_VARIANT,
    direction_2d_from_3d_index,
};
use crate::packet::mappings::{self, PacketId};
use crate::remap::{
    self, block_state_remap::remap_block_state_for_version,
    entity_id_remap::remap_object_type_for_version,
};

/// Converts the WIT-generated `JavaMinecraftVersion` into the internal `pumpkin_util` version.
#[must_use]
pub const fn from_wasm_java_version(
    version: pumpkin_plugin_api::wit::pumpkin::plugin::player::JavaMinecraftVersion,
) -> JavaMinecraftVersion {
    use pumpkin_plugin_api::wit::pumpkin::plugin::player::JavaMinecraftVersion as W;
    match version {
        W::V172 => JavaMinecraftVersion::V_1_7_2,
        W::V176 => JavaMinecraftVersion::V_1_7_6,
        W::V18 => JavaMinecraftVersion::V_1_8,
        W::V19 => JavaMinecraftVersion::V_1_9,
        W::V191 => JavaMinecraftVersion::V_1_9_1,
        W::V192 => JavaMinecraftVersion::V_1_9_2,
        W::V193 => JavaMinecraftVersion::V_1_9_3,
        W::V110 => JavaMinecraftVersion::V_1_10,
        W::V111 => JavaMinecraftVersion::V_1_11,
        W::V1111 => JavaMinecraftVersion::V_1_11_1,
        W::V112 => JavaMinecraftVersion::V_1_12,
        W::V1121 => JavaMinecraftVersion::V_1_12_1,
        W::V1122 => JavaMinecraftVersion::V_1_12_2,
        W::V113 => JavaMinecraftVersion::V_1_13,
        W::V1131 => JavaMinecraftVersion::V_1_13_1,
        W::V1132 => JavaMinecraftVersion::V_1_13_2,
        W::V114 => JavaMinecraftVersion::V_1_14,
        W::V1141 => JavaMinecraftVersion::V_1_14_1,
        W::V1142 => JavaMinecraftVersion::V_1_14_2,
        W::V1143 => JavaMinecraftVersion::V_1_14_3,
        W::V1144 => JavaMinecraftVersion::V_1_14_4,
        W::V115 => JavaMinecraftVersion::V_1_15,
        W::V1151 => JavaMinecraftVersion::V_1_15_1,
        W::V1152 => JavaMinecraftVersion::V_1_15_2,
        W::V116 => JavaMinecraftVersion::V_1_16,
        W::V1161 => JavaMinecraftVersion::V_1_16_1,
        W::V1162 => JavaMinecraftVersion::V_1_16_2,
        W::V1163 => JavaMinecraftVersion::V_1_16_3,
        W::V1164 => JavaMinecraftVersion::V_1_16_4,
        W::V117 => JavaMinecraftVersion::V_1_17,
        W::V1171 => JavaMinecraftVersion::V_1_17_1,
        W::V118 => JavaMinecraftVersion::V_1_18,
        W::V1182 => JavaMinecraftVersion::V_1_18_2,
        W::V119 => JavaMinecraftVersion::V_1_19,
        W::V1191 => JavaMinecraftVersion::V_1_19_1,
        W::V1193 => JavaMinecraftVersion::V_1_19_3,
        W::V1194 => JavaMinecraftVersion::V_1_19_4,
        W::V120 => JavaMinecraftVersion::V_1_20,
        W::V1202 => JavaMinecraftVersion::V_1_20_2,
        W::V1203 => JavaMinecraftVersion::V_1_20_3,
        W::V1205 => JavaMinecraftVersion::V_1_20_5,
        W::V121 => JavaMinecraftVersion::V_1_21,
        W::V1212 => JavaMinecraftVersion::V_1_21_2,
        W::V1214 => JavaMinecraftVersion::V_1_21_4,
        W::V1215 => JavaMinecraftVersion::V_1_21_5,
        W::V1216 => JavaMinecraftVersion::V_1_21_6,
        W::V1217 => JavaMinecraftVersion::V_1_21_7,
        W::V1219 => JavaMinecraftVersion::V_1_21_9,
        W::V12111 => JavaMinecraftVersion::V_1_21_11,
        W::V261 => JavaMinecraftVersion::V_26_1,
        W::V262 => JavaMinecraftVersion::V_26_2,
        W::V263 => JavaMinecraftVersion::V_26_3,
        W::Unknown => JavaMinecraftVersion::Unknown,
    }
}

pub static SERVERBOUND_HANDSHAKE: &[&PacketId] = &[&mappings::serverbound::handshake::INTENTION];

pub static SERVERBOUND_STATUS: &[&PacketId] = &[
    &mappings::serverbound::status::PING_REQUEST,
    &mappings::serverbound::status::STATUS_REQUEST,
];

pub static SERVERBOUND_LOGIN: &[&PacketId] = &[
    &mappings::serverbound::login::COOKIE_RESPONSE,
    &mappings::serverbound::login::CUSTOM_QUERY_ANSWER,
    &mappings::serverbound::login::HELLO,
    &mappings::serverbound::login::KEY,
    &mappings::serverbound::login::LOGIN_ACKNOWLEDGED,
];

pub static SERVERBOUND_CONFIG: &[&PacketId] = &[
    &mappings::serverbound::config::ACCEPT_CODE_OF_CONDUCT,
    &mappings::serverbound::config::CLIENT_INFORMATION,
    &mappings::serverbound::config::COOKIE_RESPONSE,
    &mappings::serverbound::config::CUSTOM_CLICK_ACTION,
    &mappings::serverbound::config::CUSTOM_PAYLOAD,
    &mappings::serverbound::config::FINISH_CONFIGURATION,
    &mappings::serverbound::config::KEEP_ALIVE,
    &mappings::serverbound::config::PONG,
    &mappings::serverbound::config::RESOURCE_PACK,
    &mappings::serverbound::config::SELECT_KNOWN_PACKS,
];

pub static SERVERBOUND_PLAY: &[&PacketId] = &[
    &mappings::serverbound::play::ACCEPT_TELEPORTATION,
    &mappings::serverbound::play::ATTACK,
    &mappings::serverbound::play::BLOCK_ENTITY_TAG_QUERY,
    &mappings::serverbound::play::BUNDLE_ITEM_SELECTED,
    &mappings::serverbound::play::CHANGE_DIFFICULTY,
    &mappings::serverbound::play::CHANGE_GAME_MODE,
    &mappings::serverbound::play::CHAT,
    &mappings::serverbound::play::CHAT_ACK,
    &mappings::serverbound::play::CHAT_COMMAND,
    &mappings::serverbound::play::CHAT_COMMAND_SIGNED,
    &mappings::serverbound::play::CHAT_PREVIEW,
    &mappings::serverbound::play::CHAT_SESSION_UPDATE,
    &mappings::serverbound::play::CHUNK_BATCH_RECEIVED,
    &mappings::serverbound::play::CLIENT_COMMAND,
    &mappings::serverbound::play::CLIENT_INFORMATION,
    &mappings::serverbound::play::CLIENT_TICK_END,
    &mappings::serverbound::play::COMMAND_SUGGESTION,
    &mappings::serverbound::play::COMMAND_SUGGESTIONS,
    &mappings::serverbound::play::CONFIGURATION_ACKNOWLEDGED,
    &mappings::serverbound::play::CONTAINER_BUTTON_CLICK,
    &mappings::serverbound::play::CONTAINER_CLICK,
    &mappings::serverbound::play::CONTAINER_CLOSE,
    &mappings::serverbound::play::CONTAINER_SLOT_STATE_CHANGED,
    &mappings::serverbound::play::COOKIE_RESPONSE,
    &mappings::serverbound::play::CUSTOM_CLICK_ACTION,
    &mappings::serverbound::play::CUSTOM_PAYLOAD,
    &mappings::serverbound::play::DEBUG_SAMPLE_SUBSCRIPTION,
    &mappings::serverbound::play::DEBUG_SUBSCRIPTION_REQUEST,
    &mappings::serverbound::play::EDIT_BOOK,
    &mappings::serverbound::play::ENTITY_TAG_QUERY,
    &mappings::serverbound::play::INTERACT,
    &mappings::serverbound::play::JIGSAW_GENERATE,
    &mappings::serverbound::play::KEEP_ALIVE,
    &mappings::serverbound::play::LOCK_DIFFICULTY,
    &mappings::serverbound::play::MOVE_PLAYER_POS,
    &mappings::serverbound::play::MOVE_PLAYER_POS_ROT,
    &mappings::serverbound::play::MOVE_PLAYER_ROT,
    &mappings::serverbound::play::MOVE_PLAYER_STATUS_ONLY,
    &mappings::serverbound::play::MOVE_VEHICLE,
    &mappings::serverbound::play::PADDLE_BOAT,
    &mappings::serverbound::play::PICK_ITEM,
    &mappings::serverbound::play::PICK_ITEM_FROM_BLOCK,
    &mappings::serverbound::play::PICK_ITEM_FROM_ENTITY,
    &mappings::serverbound::play::PING_REQUEST,
    &mappings::serverbound::play::PLACE_RECIPE,
    &mappings::serverbound::play::PLAYER_ABILITIES,
    &mappings::serverbound::play::PLAYER_ACTION,
    &mappings::serverbound::play::PLAYER_COMMAND,
    &mappings::serverbound::play::PLAYER_INPUT,
    &mappings::serverbound::play::PLAYER_LOADED,
    &mappings::serverbound::play::PONG,
    &mappings::serverbound::play::PUNCH,
    &mappings::serverbound::play::RECIPE_BOOK_CHANGE_SETTINGS,
    &mappings::serverbound::play::RECIPE_BOOK_DATA,
    &mappings::serverbound::play::RECIPE_BOOK_SEEN_RECIPE,
    &mappings::serverbound::play::RENAME_ITEM,
    &mappings::serverbound::play::RESOURCE_PACK,
    &mappings::serverbound::play::SEEN_ADVANCEMENTS,
    &mappings::serverbound::play::SELECT_TRADE,
    &mappings::serverbound::play::SET_BEACON,
    &mappings::serverbound::play::SET_CARRIED_ITEM,
    &mappings::serverbound::play::SET_COMMAND_BLOCK,
    &mappings::serverbound::play::SET_COMMAND_MINECART,
    &mappings::serverbound::play::SET_CREATIVE_MODE_SLOT,
    &mappings::serverbound::play::SET_GAME_RULE,
    &mappings::serverbound::play::SET_JIGSAW_BLOCK,
    &mappings::serverbound::play::SET_STRUCTURE_BLOCK,
    &mappings::serverbound::play::SET_TEST_BLOCK,
    &mappings::serverbound::play::SIGN_UPDATE,
    &mappings::serverbound::play::SPECTATE_ENTITY,
    &mappings::serverbound::play::SPECTATOR_ACTION,
    &mappings::serverbound::play::STEER_VEHICLE,
    &mappings::serverbound::play::SWING,
    &mappings::serverbound::play::TELEPORT_TO_ENTITY,
    &mappings::serverbound::play::TEST_INSTANCE_BLOCK_ACTION,
    &mappings::serverbound::play::USE_ITEM,
    &mappings::serverbound::play::USE_ITEM_ON,
    &mappings::serverbound::play::WINDOW_CONFIRMATION,
];

pub static CLIENTBOUND_STATUS: &[&PacketId] = &[
    &mappings::clientbound::status::PONG_RESPONSE,
    &mappings::clientbound::status::STATUS_RESPONSE,
];

pub static CLIENTBOUND_LOGIN: &[&PacketId] = &[
    &mappings::clientbound::login::COOKIE_REQUEST,
    &mappings::clientbound::login::CUSTOM_QUERY,
    &mappings::clientbound::login::GAME_PROFILE,
    &mappings::clientbound::login::HELLO,
    &mappings::clientbound::login::LOGIN_COMPRESSION,
    &mappings::clientbound::login::LOGIN_DISCONNECT,
    &mappings::clientbound::login::LOGIN_FINISHED,
];

pub static CLIENTBOUND_CONFIG: &[&PacketId] = &[
    &mappings::clientbound::config::CLEAR_DIALOG,
    &mappings::clientbound::config::CODE_OF_CONDUCT,
    &mappings::clientbound::config::COOKIE_REQUEST,
    &mappings::clientbound::config::CUSTOM_PAYLOAD,
    &mappings::clientbound::config::CUSTOM_REPORT_DETAILS,
    &mappings::clientbound::config::DISCONNECT,
    &mappings::clientbound::config::FINISH_CONFIGURATION,
    &mappings::clientbound::config::KEEP_ALIVE,
    &mappings::clientbound::config::PING,
    &mappings::clientbound::config::POST_EFFECTS,
    &mappings::clientbound::config::REGISTRY_DATA,
    &mappings::clientbound::config::RESET_CHAT,
    &mappings::clientbound::config::RESOURCE_PACK_POP,
    &mappings::clientbound::config::RESOURCE_PACK_PUSH,
    &mappings::clientbound::config::SELECT_KNOWN_PACKS,
    &mappings::clientbound::config::SERVER_LINKS,
    &mappings::clientbound::config::SHOW_DIALOG,
    &mappings::clientbound::config::STORE_COOKIE,
    &mappings::clientbound::config::TRANSFER,
    &mappings::clientbound::config::UPDATE_ENABLED_FEATURES,
    &mappings::clientbound::config::UPDATE_TAGS,
];

pub static CLIENTBOUND_PLAY: &[&PacketId] = &[
    &mappings::clientbound::play::ACKNOWLEDGE_PLAYER_DIGGING,
    &mappings::clientbound::play::ADD_ENTITY,
    &mappings::clientbound::play::ADD_TRANSIENT_BLOCK,
    &mappings::clientbound::play::ANIMATE,
    &mappings::clientbound::play::AWARD_STATS,
    &mappings::clientbound::play::BLOCK_CHANGED_ACK,
    &mappings::clientbound::play::BLOCK_DESTRUCTION,
    &mappings::clientbound::play::BLOCK_ENTITY_DATA,
    &mappings::clientbound::play::BLOCK_EVENT,
    &mappings::clientbound::play::BLOCK_UPDATE,
    &mappings::clientbound::play::BOSS_EVENT,
    &mappings::clientbound::play::BUNDLE_DELIMITER,
    &mappings::clientbound::play::CHANGE_DIFFICULTY,
    &mappings::clientbound::play::CHAT,
    &mappings::clientbound::play::CHAT_PREVIEW_PACKET,
    &mappings::clientbound::play::CHUNKS_BIOMES,
    &mappings::clientbound::play::CHUNK_BATCH_FINISHED,
    &mappings::clientbound::play::CHUNK_BATCH_START,
    &mappings::clientbound::play::CLEAR_DIALOG,
    &mappings::clientbound::play::CLEAR_TITLES,
    &mappings::clientbound::play::COMBAT_EVENT,
    &mappings::clientbound::play::COMMANDS,
    &mappings::clientbound::play::COMMAND_SUGGESTIONS,
    &mappings::clientbound::play::CONTAINER_CLOSE,
    &mappings::clientbound::play::CONTAINER_SET_CONTENT,
    &mappings::clientbound::play::CONTAINER_SET_DATA,
    &mappings::clientbound::play::CONTAINER_SET_SLOT,
    &mappings::clientbound::play::COOKIE_REQUEST,
    &mappings::clientbound::play::COOLDOWN,
    &mappings::clientbound::play::CUSTOM_CHAT_COMPLETIONS,
    &mappings::clientbound::play::CUSTOM_PAYLOAD,
    &mappings::clientbound::play::CUSTOM_REPORT_DETAILS,
    &mappings::clientbound::play::DAMAGE_EVENT,
    &mappings::clientbound::play::DEBUG_BLOCK_VALUE,
    &mappings::clientbound::play::DEBUG_CHUNK_VALUE,
    &mappings::clientbound::play::DEBUG_ENTITY_VALUE,
    &mappings::clientbound::play::DEBUG_EVENT,
    &mappings::clientbound::play::DEBUG_SAMPLE,
    &mappings::clientbound::play::DELETE_CHAT,
    &mappings::clientbound::play::DISCONNECT,
    &mappings::clientbound::play::DISGUISED_CHAT,
    &mappings::clientbound::play::DISPLAY_CHAT_PREVIEW,
    &mappings::clientbound::play::ENTITY_EVENT,
    &mappings::clientbound::play::ENTITY_MOVEMENT,
    &mappings::clientbound::play::ENTITY_POSITION_SYNC,
    &mappings::clientbound::play::EXPLODE,
    &mappings::clientbound::play::FORGET_LEVEL_CHUNK,
    &mappings::clientbound::play::GAME_EVENT,
    &mappings::clientbound::play::GAME_RULE_VALUES,
    &mappings::clientbound::play::GAME_TEST_HIGHLIGHT_POS,
    &mappings::clientbound::play::HURT_ANIMATION,
    &mappings::clientbound::play::INITIALIZE_BORDER,
    &mappings::clientbound::play::KEEP_ALIVE,
    &mappings::clientbound::play::LEVEL_CHUNK_WITH_LIGHT,
    &mappings::clientbound::play::LEVEL_EVENT,
    &mappings::clientbound::play::LEVEL_PARTICLES,
    &mappings::clientbound::play::LIGHT_UPDATE,
    &mappings::clientbound::play::LOGIN,
    &mappings::clientbound::play::LOW_DISK_SPACE_WARNING,
    &mappings::clientbound::play::MAP_CHUNK_BULK,
    &mappings::clientbound::play::MAP_ITEM_DATA,
    &mappings::clientbound::play::MERCHANT_OFFERS,
    &mappings::clientbound::play::MOUNT_SCREEN_OPEN,
    &mappings::clientbound::play::MOVE_ENTITY_POS,
    &mappings::clientbound::play::MOVE_ENTITY_POS_ROT,
    &mappings::clientbound::play::MOVE_ENTITY_ROT,
    &mappings::clientbound::play::MOVE_MINECART_ALONG_TRACK,
    &mappings::clientbound::play::MOVE_PLAYER_ROT,
    &mappings::clientbound::play::MOVE_VEHICLE,
    &mappings::clientbound::play::NAMED_SOUND_EFFECT,
    &mappings::clientbound::play::OPEN_BOOK,
    &mappings::clientbound::play::OPEN_SCREEN,
    &mappings::clientbound::play::OPEN_SIGN_EDITOR,
    &mappings::clientbound::play::PING,
    &mappings::clientbound::play::PLACE_GHOST_RECIPE,
    &mappings::clientbound::play::PLAYER_ABILITIES,
    &mappings::clientbound::play::PLAYER_CHAT,
    &mappings::clientbound::play::PLAYER_CHAT_HEADER,
    &mappings::clientbound::play::PLAYER_COMBAT_END,
    &mappings::clientbound::play::PLAYER_COMBAT_ENTER,
    &mappings::clientbound::play::PLAYER_COMBAT_KILL,
    &mappings::clientbound::play::PLAYER_INFO,
    &mappings::clientbound::play::PLAYER_INFO_REMOVE,
    &mappings::clientbound::play::PLAYER_INFO_UPDATE,
    &mappings::clientbound::play::PLAYER_LOOK_AT,
    &mappings::clientbound::play::PLAYER_POSITION,
    &mappings::clientbound::play::PLAYER_ROTATION,
    &mappings::clientbound::play::PONG_RESPONSE,
    &mappings::clientbound::play::POST_EFFECTS,
    &mappings::clientbound::play::PROJECTILE_POWER,
    &mappings::clientbound::play::RECIPE_BOOK_ADD,
    &mappings::clientbound::play::RECIPE_BOOK_REMOVE,
    &mappings::clientbound::play::RECIPE_BOOK_SETTINGS,
    &mappings::clientbound::play::REMOVE_ENTITIES,
    &mappings::clientbound::play::REMOVE_MOB_EFFECT,
    &mappings::clientbound::play::RESET_SCORE,
    &mappings::clientbound::play::RESOURCE_PACK_POP,
    &mappings::clientbound::play::RESOURCE_PACK_PUSH,
    &mappings::clientbound::play::RESPAWN,
    &mappings::clientbound::play::ROTATE_HEAD,
    &mappings::clientbound::play::SCULK_VIBRATION_SIGNAL,
    &mappings::clientbound::play::SECTION_BLOCKS_UPDATE,
    &mappings::clientbound::play::SELECT_ADVANCEMENTS_TAB,
    &mappings::clientbound::play::SERVER_DATA,
    &mappings::clientbound::play::SERVER_LINKS,
    &mappings::clientbound::play::SET_ACTION_BAR_TEXT,
    &mappings::clientbound::play::SET_BORDER_CENTER,
    &mappings::clientbound::play::SET_BORDER_LERP_SIZE,
    &mappings::clientbound::play::SET_BORDER_SIZE,
    &mappings::clientbound::play::SET_BORDER_WARNING_DELAY,
    &mappings::clientbound::play::SET_BORDER_WARNING_DISTANCE,
    &mappings::clientbound::play::SET_CAMERA,
    &mappings::clientbound::play::SET_CARRIED_ITEM,
    &mappings::clientbound::play::SET_CHUNK_CACHE_CENTER,
    &mappings::clientbound::play::SET_CHUNK_CACHE_RADIUS,
    &mappings::clientbound::play::SET_COMPRESSION,
    &mappings::clientbound::play::SET_CURSOR_ITEM,
    &mappings::clientbound::play::SET_DEFAULT_SPAWN_POSITION,
    &mappings::clientbound::play::SET_DISPLAY_OBJECTIVE,
    &mappings::clientbound::play::SET_ENTITY_DATA,
    &mappings::clientbound::play::SET_ENTITY_LINK,
    &mappings::clientbound::play::SET_ENTITY_MOTION,
    &mappings::clientbound::play::SET_EQUIPMENT,
    &mappings::clientbound::play::SET_EXPERIENCE,
    &mappings::clientbound::play::SET_HEALTH,
    &mappings::clientbound::play::SET_HELD_SLOT,
    &mappings::clientbound::play::SET_OBJECTIVE,
    &mappings::clientbound::play::SET_PASSENGERS,
    &mappings::clientbound::play::SET_PLAYER_INVENTORY,
    &mappings::clientbound::play::SET_PLAYER_TEAM,
    &mappings::clientbound::play::SET_SCORE,
    &mappings::clientbound::play::SET_SIMULATION_DISTANCE,
    &mappings::clientbound::play::SET_SUBTITLE_TEXT,
    &mappings::clientbound::play::SET_TIME,
    &mappings::clientbound::play::SET_TITLES_ANIMATION,
    &mappings::clientbound::play::SET_TITLE_TEXT,
    &mappings::clientbound::play::SHOW_DIALOG,
    &mappings::clientbound::play::SIGN_UPDATE,
    &mappings::clientbound::play::SOUND,
    &mappings::clientbound::play::SOUND_ENTITY,
    &mappings::clientbound::play::SPAWN_EXPERIENCE_ORB,
    &mappings::clientbound::play::SPAWN_LIVING_ENTITY,
    &mappings::clientbound::play::SPAWN_PAINTING,
    &mappings::clientbound::play::SPAWN_PLAYER,
    &mappings::clientbound::play::SPAWN_WEATHER_ENTITY,
    &mappings::clientbound::play::START_CONFIGURATION,
    &mappings::clientbound::play::STOP_SOUND,
    &mappings::clientbound::play::STORE_COOKIE,
    &mappings::clientbound::play::SWING_ANIMATION,
    &mappings::clientbound::play::SYSTEM_CHAT,
    &mappings::clientbound::play::TAB_LIST,
    &mappings::clientbound::play::TAG_QUERY,
    &mappings::clientbound::play::TAKE_ITEM_ENTITY,
    &mappings::clientbound::play::TELEPORT_ENTITY,
    &mappings::clientbound::play::TEST_INSTANCE_BLOCK_STATUS,
    &mappings::clientbound::play::TICKING_STATE,
    &mappings::clientbound::play::TICKING_STEP,
    &mappings::clientbound::play::TITLE,
    &mappings::clientbound::play::TRANSFER,
    &mappings::clientbound::play::UNLOCK_RECIPES,
    &mappings::clientbound::play::UPDATE_ADVANCEMENTS,
    &mappings::clientbound::play::UPDATE_ATTRIBUTES,
    &mappings::clientbound::play::UPDATE_ENABLED_FEATURES,
    &mappings::clientbound::play::UPDATE_ENTITY_NBT,
    &mappings::clientbound::play::UPDATE_MOB_EFFECT,
    &mappings::clientbound::play::UPDATE_RECIPES,
    &mappings::clientbound::play::UPDATE_TAGS,
    &mappings::clientbound::play::USE_BED,
    &mappings::clientbound::play::WAYPOINT,
    &mappings::clientbound::play::WINDOW_CONFIRMATION,
    &mappings::clientbound::play::WORLD_BORDER,
];

pub struct PacketTranslator;

/// Whether a type is a `LivingEntity` on the client, which is what decides
/// between `SPAWN_LIVING_ENTITY` and `SPAWN_ENTITY` below 1.19. Every living
/// type has attributes (health at least) and no other type does; `mob` alone
/// misses armor stands.
fn is_living_type(entity_type: &EntityType) -> bool {
    entity_type.mob || !entity_type.attributes.is_empty()
}

impl PacketTranslator {
    /// Translates an incoming serverbound packet ID from a specific client version into the 26.3 packet ID.
    #[must_use]
    /// `state` uses the same encoding as [`Self::translate_clientbound_packet_id`];
    /// ids are only unique within a connection state.
    pub fn translate_serverbound_packet_id(
        packet_id: i32,
        version: JavaMinecraftVersion,
        state: u8,
    ) -> Option<i32> {
        if version == JavaMinecraftVersion::V_26_3 {
            return Some(packet_id);
        }
        // 26.3 renamed SWING to PUNCH. They are separate rows in the table and
        // SWING has no 26.3 id, so the generic lookup below cannot cross the
        // rename and the packet would fall through untranslated.
        if state == 5
            && mappings::serverbound::play::SWING.v26_3 == -1
            && mappings::serverbound::play::SWING.to_id(version) == packet_id
            && mappings::serverbound::play::PUNCH.v26_3 != -1
        {
            return Some(mappings::serverbound::play::PUNCH.v26_3);
        }

        let table: &[&PacketId] = match state {
            0 => SERVERBOUND_HANDSHAKE,
            1 => SERVERBOUND_STATUS,
            2 | 3 => SERVERBOUND_LOGIN,
            4 => SERVERBOUND_CONFIG,
            5 => SERVERBOUND_PLAY,
            _ => return None,
        };
        for &packet in table {
            if packet.to_id(version) == packet_id && packet.v26_3 != -1 {
                return Some(packet.v26_3);
            }
        }
        None
    }

    /// Translates an outgoing 26.3 clientbound packet ID into the target client version packet ID.
    /// Translates a clientbound packet id.
    ///
    /// `state` is the connection state the packet belongs to, encoded as
    /// 0 handshake, 1 status, 2 login, 3 transfer, 4 config, 5 play. It is
    /// required: packet ids are only unique within a state, so searching
    /// every table would mistranslate. Login `LOGIN_COMPRESSION` and play
    /// `AWARD_STATS` both sit at 26.3 id 3, for instance.
    #[must_use]
    pub fn translate_clientbound_packet_id(
        packet_id_26_3: i32,
        version: JavaMinecraftVersion,
        state: u8,
    ) -> Option<i32> {
        if version == JavaMinecraftVersion::V_26_3 {
            return Some(packet_id_26_3);
        }
        let table: &[&PacketId] = match state {
            1 => CLIENTBOUND_STATUS,
            2 | 3 => CLIENTBOUND_LOGIN,
            4 => CLIENTBOUND_CONFIG,
            5 => CLIENTBOUND_PLAY,
            // Handshake has no clientbound packets.
            _ => return Some(packet_id_26_3),
        };
        for &packet in table {
            if packet.v26_3 == packet_id_26_3 {
                let client_id = packet.to_id(version);
                if client_id != -1 {
                    return Some(client_id);
                }
            }
        }
        None
    }

    /// Translates a sound ID from 26.3 to the client's version.
    #[must_use]
    pub fn translate_sound_id(sound_id: u16, version: JavaMinecraftVersion) -> u16 {
        remap::sound_id_remap::remap_sound_id_for_version(sound_id, version)
    }

    /// Translates a block state ID from 26.3 to the client's version.
    #[must_use]
    pub fn translate_block_state(state_id: u16, version: JavaMinecraftVersion) -> u16 {
        remap::block_state_remap::remap_block_state_for_version(state_id, version)
    }

    /// Translates an item ID from 26.3 to the client's version.
    #[must_use]
    pub fn translate_item_id(item_id: u16, version: JavaMinecraftVersion) -> u16 {
        remap::item_id_remap::remap_item_id_for_version(item_id, version)
    }

    /// Translates an incoming item ID from the client's version to 26.3.
    #[must_use]
    pub fn translate_item_id_to_server(item_id: u16, version: JavaMinecraftVersion) -> u16 {
        remap::item_id_remap::remap_item_id_from_version(item_id, version)
    }

    /// Translates an entity type ID from 26.3 to the client's version.
    #[must_use]
    pub fn translate_entity_id(entity_id: u16, version: JavaMinecraftVersion) -> u16 {
        remap::entity_id_remap::remap_entity_id_for_version(entity_id, version)
    }

    /// Translates a particle ID from 26.3 to the client's version.
    #[must_use]
    pub fn translate_particle_id(particle_id: u16, version: JavaMinecraftVersion) -> u16 {
        remap::particle_id_remap::remap_particle_id_for_version(particle_id, version)
    }

    /// Translates a menu ID from 26.3 to the client's version.
    #[must_use]
    pub fn translate_menu_id(menu_id: u8, version: JavaMinecraftVersion) -> u8 {
        remap::menu_id_remap::remap_menu_id_for_version(menu_id, version)
    }

    /// Translates an attribute ID from 26.3 to the client's version.
    #[must_use]
    pub fn translate_attribute_id(attr_id: u8, version: JavaMinecraftVersion) -> u8 {
        remap::attribute_id_remap::remap_attribute_id_for_version(u32::from(attr_id), version) as u8
    }

    /// Translates a custom stat ID from 26.3 to the client's version.
    #[must_use]
    pub fn translate_custom_stat_id(stat_id: u16, version: JavaMinecraftVersion) -> u16 {
        remap::custom_stat_id_remap::remap_custom_stat_id_for_version(u32::from(stat_id), version)
            as u16
    }

    /// Translates a painting variant ID from 26.3 to the client's version.
    #[must_use]
    pub fn translate_painting_variant(variant_id: u32, version: JavaMinecraftVersion) -> u32 {
        remap::painting_variant_id_remap::remap_motive_id_for_version(variant_id, version)
    }

    /// Translates an incoming packet (from an older client to 26.3).
    /// Returns the normalized 26.3 packet ID and potentially translated payload.
    #[must_use]
    pub fn translate_incoming_packet(
        packet_id: i32,
        raw_payload: &[u8],
        version: JavaMinecraftVersion,
        state: u8,
    ) -> Option<(i32, Vec<u8>)> {
        if version == JavaMinecraftVersion::V_26_3 {
            return None;
        }

        // Serverbound layouts that changed on the way down (`use_item` gained
        // a rotation in 767, `enchant_item` widened its fields, and 1.20.5 is
        // where `chat_command` split into a signed and an unsigned form) are
        // core's readers' business: they branch on the client's version
        // themselves. Nothing is rewritten here, so tiers 2 and 3 need no work
        // on this side; the id table is what carries the difference, and it
        // matches minecraft-data exactly for 1.20.2 and 1.20.4.
        let new_id = Self::translate_serverbound_packet_id(packet_id, version, state)?;
        // Only the id changes. Pumpkin decodes every serverbound packet with
        // the client's own version and each reader carries the older layouts
        // itself, so rewriting payloads into 26.3 shape here got them read
        // twice. PLAYER_COMMAND was the worst of the old rules: it dropped
        // sneak actions to an empty payload and the client was kicked the
        // first time it crouched.
        Some((new_id, raw_payload.to_vec()))
    }

    /// Translates an outgoing packet (from 26.3 server to an older client).
    ///
    /// Intercepts entity spawning (`ADD_ENTITY`) and wraps with `CSpawnLivingEntity`
    /// or `CSpawnPainting` when targeting versions before entity unification.
    /// Also remaps packet IDs and payload IDs (sounds, particles, blocks, items, etc.).
    #[must_use]
    pub fn translate_outgoing_packet(
        packet_id: i32,
        raw_payload: &[u8],
        version: JavaMinecraftVersion,
        state: u8,
    ) -> Option<(i32, Vec<u8>)> {
        if version == JavaMinecraftVersion::V_26_3 {
            return None;
        }

        // Check for CSpawnEntity (ADD_ENTITY). Play state only: the same id
        // means something else in login/config. Core wrote the payload in the
        // client's layout, so it is decoded with the client's version.
        //
        // The 1.20.2, 1.20.3 and 1.20.5 wire layouts of this packet are
        // identical to 1.21's (minecraft-data 1.20.2, 1.20.3, 1.20.6 and
        // 1.21.1 agree field for field: id, uuid, type, three f64, three
        // angles, varint data, vec3i16 velocity), and both core's writer and
        // its reader branch at 1.9, 1.14, 1.19 and 1.21.9 only, so protocols
        // 764, 765 and 766 need no branch of their own. When the decode does
        // not line up the packet is left to the byte level fallback further
        // down, which rewrites just the type field.
        if state == 5
            && packet_id == mappings::clientbound::play::ADD_ENTITY.v26_3
            && let Ok(spawn_entity) = CSpawnEntity::read_packet_data(raw_payload, &version)
        {
            let entity_type_id = spawn_entity.r#type.0 as u16;

            // 0. Players have their own spawn packet below 1.20.2, and
            // experience orbs below 1.21.5. Without these wrappers other
            // players and orbs are invisible: ADD_ENTITY with the player
            // type is not something those clients accept.
            if version < JavaMinecraftVersion::V_1_20_2 && entity_type_id == EntityType::PLAYER.id {
                let player = CSpawnPlayer::new(
                    spawn_entity.entity_id,
                    spawn_entity.entity_uuid,
                    spawn_entity.position,
                    spawn_entity.yaw,
                    spawn_entity.pitch,
                );
                let mut buf = Vec::new();
                if player.write_packet_data(&mut buf, &version).is_ok() {
                    return Some((CSpawnPlayer::to_id(version), buf));
                }
                return None;
            }
            if version < JavaMinecraftVersion::V_1_21_5
                && entity_type_id == EntityType::EXPERIENCE_ORB.id
            {
                let count = i16::try_from(spawn_entity.data.0).unwrap_or(i16::MAX);
                let orb =
                    CSpawnExperienceOrb::new(spawn_entity.entity_id, spawn_entity.position, count);
                let mut buf = Vec::new();
                if orb.write_packet_data(&mut buf, &version).is_ok() {
                    return Some((CSpawnExperienceOrb::to_id(version), buf));
                }
                return None;
            }

            // 1. Paintings have their own packet in <= 1.18.2. Its direction
            // is the 2D facing, while ADD_ENTITY's data field carries the 3D
            // one; the variant lives in metadata from 1.19 and cannot be
            // recovered here, so every painting shows the default (kebab).
            if version <= JavaMinecraftVersion::V_1_18_2
                && entity_type_id == EntityType::PAINTING.id
            {
                let direction = direction_2d_from_3d_index(spawn_entity.data.0)?;
                let painting = CSpawnPainting::new(
                    spawn_entity.entity_id,
                    spawn_entity.entity_uuid,
                    String::new(),
                    DEFAULT_VARIANT,
                    BlockPos::new(
                        spawn_entity.position.x.floor() as i32,
                        spawn_entity.position.y.floor() as i32,
                        spawn_entity.position.z.floor() as i32,
                    ),
                    direction,
                );
                let mut buf = Vec::new();
                if painting.write_packet_data(&mut buf, &version).is_ok() {
                    let target_id = CSpawnPainting::to_id(version);
                    return Some((target_id, buf));
                }
                return None;
            }

            // 2. Every living entity except the player uses SPAWN_LIVING_ENTITY
            // below 1.19 (vanilla's LivingEntity::getAddEntityPacket), armor
            // stands included, which `EntityType::mob` does not cover.
            if version < JavaMinecraftVersion::V_1_19 {
                let is_living = EntityType::from_raw(entity_type_id).is_some_and(is_living_type);
                if is_living {
                    let living = CSpawnLivingEntity::new(
                        spawn_entity.entity_id,
                        spawn_entity.entity_uuid,
                        spawn_entity.r#type,
                        spawn_entity.position,
                        spawn_entity.pitch_degrees(),
                        spawn_entity.yaw_degrees(),
                        spawn_entity.head_yaw_degrees(),
                        spawn_entity.velocity.0,
                        None,
                    );
                    let mut buf = Vec::new();
                    if living.write_packet_data(&mut buf, &version).is_ok() {
                        let target_id = CSpawnLivingEntity::to_id(version);
                        return Some((target_id, buf));
                    }
                }
            }

            // 3. Normal entity: remap object/entity type and falling block state
            let remapped_type = if version < JavaMinecraftVersion::V_1_14 {
                VarInt(i32::from(remap_object_type_for_version(
                    entity_type_id,
                    version,
                )))
            } else {
                VarInt(i32::from(
                    remap::entity_id_remap::remap_entity_id_for_version(entity_type_id, version),
                ))
            };

            let remapped_data = if entity_type_id == EntityType::FALLING_BLOCK.id {
                u16::try_from(spawn_entity.data.0).map_or(spawn_entity.data, |state_id| {
                    VarInt(i32::from(remap_block_state_for_version(state_id, version)))
                })
            } else {
                spawn_entity.data
            };

            let modified_spawn = CSpawnEntity {
                entity_id: spawn_entity.entity_id,
                entity_uuid: spawn_entity.entity_uuid,
                r#type: remapped_type,
                position: spawn_entity.position,
                velocity: spawn_entity.velocity,
                pitch: spawn_entity.pitch,
                yaw: spawn_entity.yaw,
                head_yaw: spawn_entity.head_yaw,
                data: remapped_data,
            };

            let mut buf = Vec::new();
            if modified_spawn.write_packet_data(&mut buf, &version).is_ok() {
                let target_id = mappings::clientbound::play::ADD_ENTITY.to_id(version);
                return Some((target_id, buf));
            }
        }

        // Status ping: report the client's own protocol so the server list shows
        // it as joinable instead of "Incompatible version!".
        // Versions below the supported floor keep the 26.3 response and show
        // "Incompatible version!", which is what they would get on joining.
        if state == 1
            && packet_id == mappings::clientbound::status::STATUS_RESPONSE.v26_3
            && crate::packet::is_version_supported(version)
        {
            let target_id = mappings::clientbound::status::STATUS_RESPONSE.to_id(version);
            if target_id != -1
                && let Some(payload) =
                    crate::packet::status::rewrite_status_response(raw_payload, version)
            {
                return Some((target_id, payload));
            }
        }

        // SET_ENTITY_DATA needs no translation for these clients: every entity
        // type shared by 26.2 and 26.3 has an identical metadata layout, so the
        // indices already line up. An earlier out-of-bounds crash here was the
        // entity *type* id going untranslated in ADD_ENTITY, which made the
        // client build the wrong entity class and overflow its field list.

        // The recipe book embeds item ids inside nested recipe-display codecs,
        // which would need a full parser for that structure to translate. The
        // recipe book is not load bearing, so drop it rather than hand an older
        // client ids its registry does not have. Stopgap, not a translation.
        if state == 5 && packet_id == mappings::clientbound::play::RECIPE_BOOK_ADD.v26_3 {
            return None;
        }

        // Single and multi block changes carry raw state ids too. Without these
        // the client's world drifts from the server's after any block changes,
        // so what you aim at stops matching what the server thinks is there.
        // Core writes them in the client's layout and the parsers follow the
        // same branches; a payload that does not parse is dropped, never sent
        // with 26.3 ids.
        if state == 5 && version >= crate::packet::block_update::OLDEST_LAYOUT {
            if packet_id == mappings::clientbound::play::BLOCK_UPDATE.v26_3 {
                let target_id = mappings::clientbound::play::BLOCK_UPDATE.to_id(version);
                if target_id == -1 {
                    return None;
                }
                let payload =
                    crate::packet::block_update::remap_block_update(raw_payload, version)?;
                return Some((target_id, payload));
            }
            if packet_id == mappings::clientbound::play::LEVEL_EVENT.v26_3 {
                let target_id = mappings::clientbound::play::LEVEL_EVENT.to_id(version);
                if target_id == -1 {
                    return None;
                }
                let payload = crate::packet::block_update::remap_level_event(raw_payload, version)?;
                return Some((target_id, payload));
            }
            if packet_id == mappings::clientbound::play::SECTION_BLOCKS_UPDATE.v26_3 {
                let target_id = mappings::clientbound::play::SECTION_BLOCKS_UPDATE.to_id(version);
                if target_id == -1 {
                    return None;
                }
                let payload =
                    crate::packet::block_update::remap_section_blocks_update(raw_payload, version)?;
                return Some((target_id, payload));
            }
        }

        // 1.20.2 has no RESET_SCORE; it clears a score with SET_SCORE action
        // 1. Rewrite rather than drop, or scores never clear on that client.
        if state == 5
            && packet_id == mappings::clientbound::play::RESET_SCORE.v26_3
            && version < crate::packet::score::FIRST_WITH_RESET_SCORE
        {
            let target_id = mappings::clientbound::play::SET_SCORE.to_id(version);
            if target_id == -1 {
                return None;
            }
            return crate::packet::score::rewrite_reset_score(raw_payload)
                .map(|payload| (target_id, payload));
        }

        // Chunk sections carry raw block state ids, which shift between
        // versions. Core writes the chunk in the client's layout and the
        // remapper follows the same branches, so only the palettes need
        // rewriting. A chunk the remapper cannot handle (a direct palette
        // section, a layout older than it knows) is dropped rather than sent
        // with 26.3 ids the client would crash on.
        if state == 5 && packet_id == mappings::clientbound::play::LEVEL_CHUNK_WITH_LIGHT.v26_3 {
            let target_id = mappings::clientbound::play::LEVEL_CHUNK_WITH_LIGHT.to_id(version);
            if target_id == -1 {
                return None;
            }
            return crate::packet::chunk_remap::remap_chunk_payload(raw_payload, version)
                .map(|payload| (target_id, payload));
        }

        // Clients without a configuration state get their registries inside
        // the play LOGIN packet, as the NBT dimension codec, and 1.16.2 to
        // 1.18.2 get the current dimension's own element inline there and in
        // RESPAWN. Same problem as REGISTRY_DATA below, different carrier: the
        // 26.3 NBT core writes fails the client's registry load outright, so
        // the codec and the inline element are replaced with that version's
        // own. A payload that does not parse in full is dropped.
        if state == 5 && version < crate::packet::join::FIRST_WITH_CONFIG_STATE {
            if packet_id == mappings::clientbound::play::LOGIN.v26_3 {
                let target_id = mappings::clientbound::play::LOGIN.to_id(version);
                if target_id == -1 {
                    return None;
                }
                return crate::packet::join::rewrite_login(raw_payload, version)
                    .map(|payload| (target_id, payload));
            }
            if packet_id == mappings::clientbound::play::RESPAWN.v26_3
                && version < crate::packet::join::FIRST_WITH_DIMENSION_NAME
            {
                let target_id = mappings::clientbound::play::RESPAWN.to_id(version);
                if target_id == -1 {
                    return None;
                }
                return crate::packet::join::rewrite_respawn(raw_payload, version)
                    .map(|payload| (target_id, payload));
            }
        }

        // Registry contents are version specific. A 26.3 entry carries NBT an
        // older client cannot decode, and the client rejects the whole registry
        // load rather than the single bad entry, so send that version's own data.
        if state == 4 && packet_id == mappings::clientbound::config::REGISTRY_DATA.v26_3 {
            let target_id = mappings::clientbound::config::REGISTRY_DATA.to_id(version);
            if target_id != -1 {
                // 1.20.2 to 1.20.4 get every registry in one NBT compound
                // instead of one packet each, and core writes that bundle for
                // them in the vanilla layout. The rewriter leaves out the
                // registries the version does not have and replaces every
                // entry's element with that version's own NBT. A bundle that
                // cannot be parsed in full is dropped, never half rewritten.
                if version >= JavaMinecraftVersion::V_1_20_2
                    && version < JavaMinecraftVersion::V_1_20_5
                {
                    return crate::registry::build_registry_bundle_payload(version, raw_payload)
                        .map(|payload| (target_id, payload));
                }
                match crate::registry::build_registry_payload(version, raw_payload) {
                    Some(Some(payload)) => return Some((target_id, payload)),
                    // This version has no such registry; drop rather than send it.
                    Some(None) => return None,
                    // No generated data for this version: fall through to id-only.
                    None => {}
                }
            }
        }

        // UPDATE_TAGS: drop the groups for registries this client does not
        // have, which it treats as fatal, and renumber block, item and entity
        // members for its registries.
        let is_update_tags = (state == 4
            && packet_id == mappings::clientbound::config::UPDATE_TAGS.v26_3)
            || (state == 5 && packet_id == mappings::clientbound::play::UPDATE_TAGS.v26_3);
        if is_update_tags {
            let target_id = if state == 4 {
                mappings::clientbound::config::UPDATE_TAGS.to_id(version)
            } else {
                mappings::clientbound::play::UPDATE_TAGS.to_id(version)
            };
            if target_id != -1
                && let Some(payload) =
                    crate::packet::update_tags::rewrite_update_tags(raw_payload, version)
            {
                return Some((target_id, payload));
            }
        }

        // Fallback for ADD_ENTITY when the full decode above did not fire or did
        // not return: the entity type id still has to be translated, or the
        // client builds the wrong entity class for every spawn.
        if state == 5 && packet_id == mappings::clientbound::play::ADD_ENTITY.v26_3 {
            let target_id = mappings::clientbound::play::ADD_ENTITY.to_id(version);
            if target_id != -1
                && let Some(payload) =
                    crate::packet::entity::remap_spawn_entity(raw_payload, version)
            {
                return Some((target_id, payload));
            }
        }

        // Generic packet ID translation
        let client_id = Self::translate_clientbound_packet_id(packet_id, version, state)?;
        Some((client_id, raw_payload.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_data::entity::EntityType;

    #[test]
    fn living_types_include_armor_stands_but_not_objects() {
        assert!(super::is_living_type(&EntityType::ARMOR_STAND));
        assert!(super::is_living_type(&EntityType::PIG));
        assert!(super::is_living_type(&EntityType::VILLAGER));
        assert!(!super::is_living_type(&EntityType::ARROW));
        assert!(!super::is_living_type(&EntityType::ITEM));
        assert!(!super::is_living_type(&EntityType::OAK_BOAT));
        assert!(!super::is_living_type(&EntityType::PAINTING));
    }
}
