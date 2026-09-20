use pumpkin_data::entity::EntityType;
use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::rewriter::entity::{read_spawn, replace, spawned_type};
use crate::api::{Ctx, PacketWrapper, Protocol, Registry, Step, TranslateError, UserConnection};
use crate::packet::legacy::CSpawnPlayer;
use crate::packet::mappings::clientbound;

pub struct Protocol1_20_2To1_20;

impl Protocol for Protocol1_20_2To1_20 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_20_2,
            to: JavaMinecraftVersion::V_1_20,
        }
    }

    fn register(&self, reg: &mut Registry) {
        reg.clientbound(&clientbound::play::ADD_ENTITY, add_entity);
    }
}

/// `handleAddEntity` has no case for the player type up to 1.20.1, so other
/// players stay invisible without `SPAWN_PLAYER`.
fn add_entity(
    wrapper: &mut PacketWrapper,
    connection: &mut UserConnection,
    ctx: &Ctx,
) -> Result<(), TranslateError> {
    if spawned_type(wrapper, connection) != Some(EntityType::PLAYER.id) {
        wrapper.passthrough_all();
        return Ok(());
    }
    let spawn = read_spawn(wrapper, ctx.layout)?;
    let player = CSpawnPlayer::new(
        spawn.entity_id,
        spawn.entity_uuid,
        spawn.position,
        spawn.yaw,
        spawn.pitch,
    );
    replace(
        wrapper,
        &clientbound::play::SPAWN_PLAYER,
        &player,
        ctx.layout,
    )
}
