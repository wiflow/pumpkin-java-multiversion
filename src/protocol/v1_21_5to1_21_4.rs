use pumpkin_data::entity::EntityType;
use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::rewriter::entity::{read_spawn, replace, spawned_type};
use crate::api::{Ctx, PacketWrapper, Protocol, Registry, Step, TranslateError, UserConnection};
use crate::packet::legacy::CSpawnExperienceOrb;
use crate::packet::mappings::clientbound;

pub struct Protocol1_21_5To1_21_4;

impl Protocol for Protocol1_21_5To1_21_4 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_21_5,
            to: JavaMinecraftVersion::V_1_21_4,
        }
    }

    fn register(&self, reg: &mut Registry) {
        reg.clientbound(&clientbound::play::ADD_ENTITY, add_entity);
    }
}

/// Orbs keep their own spawn packet up to 1.21.4 and are invisible without it.
fn add_entity(
    wrapper: &mut PacketWrapper,
    connection: &mut UserConnection,
    ctx: &Ctx,
) -> Result<(), TranslateError> {
    if spawned_type(wrapper, connection) != Some(EntityType::EXPERIENCE_ORB.id) {
        wrapper.passthrough_all();
        return Ok(());
    }
    let spawn = read_spawn(wrapper, ctx.layout)?;
    let count = i16::try_from(spawn.data.0).unwrap_or(i16::MAX);
    let orb = CSpawnExperienceOrb::new(spawn.entity_id, spawn.position, count);
    replace(
        wrapper,
        &clientbound::play::SPAWN_EXPERIENCE_ORB,
        &orb,
        ctx.layout,
    )
}
