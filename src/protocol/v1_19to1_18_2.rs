use pumpkin_data::entity::EntityType;
use pumpkin_protocol::VarInt;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::rewriter::entity::{read_spawn, replace, spawned_type};
use crate::api::{Ctx, PacketWrapper, Protocol, Registry, Step, TranslateError, UserConnection};
use crate::packet::legacy::{
    CSpawnLivingEntity, CSpawnPainting, DEFAULT_VARIANT, direction_2d_from_3d_index,
};
use crate::packet::mappings::clientbound;

pub struct Protocol1_19To1_18_2;

impl Protocol for Protocol1_19To1_18_2 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_19,
            to: JavaMinecraftVersion::V_1_18_2,
        }
    }

    fn register(&self, reg: &mut Registry) {
        reg.clientbound(&clientbound::play::ADD_ENTITY, add_entity);
    }
}

/// Vanilla sends every `LivingEntity` but the player through
/// `ClientboundAddMobPacket` here, and `EntityType::mob` leaves out the armor
/// stand.
fn spawns_as_living(entity_type: u16) -> bool {
    entity_type == EntityType::ARMOR_STAND.id
        || EntityType::from_raw(entity_type).is_some_and(|kind| kind.mob)
}

fn add_entity(
    wrapper: &mut PacketWrapper,
    connection: &mut UserConnection,
    ctx: &Ctx,
) -> Result<(), TranslateError> {
    let Some(entity_type) = spawned_type(wrapper, connection) else {
        wrapper.passthrough_all();
        return Ok(());
    };

    if entity_type == EntityType::PAINTING.id {
        let spawn = read_spawn(wrapper, ctx.layout)?;
        // 26.3 puts the 3D facing in the data field and the variant in metadata.
        let direction = direction_2d_from_3d_index(spawn.data.0)
            .ok_or(TranslateError::Unsupported("painting facing"))?;
        let painting = CSpawnPainting::new(
            spawn.entity_id,
            spawn.entity_uuid,
            String::new(),
            DEFAULT_VARIANT,
            BlockPos::new(
                spawn.position.x.floor() as i32,
                spawn.position.y.floor() as i32,
                spawn.position.z.floor() as i32,
            ),
            direction,
        );
        return replace(
            wrapper,
            &clientbound::play::SPAWN_PAINTING,
            &painting,
            ctx.layout,
        );
    }

    if spawns_as_living(entity_type) {
        let spawn = read_spawn(wrapper, ctx.layout)?;
        let living = CSpawnLivingEntity::new(
            spawn.entity_id,
            spawn.entity_uuid,
            VarInt(i32::from(entity_type)),
            spawn.position,
            spawn.pitch_degrees(),
            spawn.yaw_degrees(),
            spawn.head_yaw_degrees(),
            spawn.velocity.0,
            None,
        );
        return replace(
            wrapper,
            &clientbound::play::SPAWN_LIVING_ENTITY,
            &living,
            ctx.layout,
        );
    }

    wrapper.passthrough_all();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::spawns_as_living;
    use pumpkin_data::entity::EntityType;

    #[test]
    fn living_types_include_armor_stands_but_not_objects() {
        assert!(spawns_as_living(EntityType::ARMOR_STAND.id));
        assert!(spawns_as_living(EntityType::PIG.id));
        assert!(spawns_as_living(EntityType::VILLAGER.id));
        assert!(!spawns_as_living(EntityType::ARROW.id));
        assert!(!spawns_as_living(EntityType::ITEM.id));
        assert!(!spawns_as_living(EntityType::OAK_BOAT.id));
        assert!(!spawns_as_living(EntityType::PAINTING.id));
        assert!(!spawns_as_living(EntityType::PLAYER.id));
    }
}
