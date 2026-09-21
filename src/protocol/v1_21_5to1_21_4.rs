use pumpkin_data::entity::EntityType;
use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::rewriter::entity::{read_spawn, replace, spawned_type};
use crate::api::rewriter::item::ClientItemT;
use crate::api::types::{HASHED_ITEM, HashedItem, I16T, Item, VAR_INT};
use crate::api::{
    Ctx, MappingData, PacketWrapper, Protocol, Registry, Step, TranslateError, UserConnection,
};
use crate::packet::legacy::CSpawnExperienceOrb;
use crate::packet::mappings::{clientbound, serverbound};
use crate::pipeline::item_pass;

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
        reg.serverbound_layout(&serverbound::play::CONTAINER_CLICK, container_click);
    }
}

/// Clients below 1.21.5 send whole stacks where core reads component hashes.
/// The hashes themselves cannot be recomputed from the older component
/// shapes, so a stack with components arrives as a bare id and count and the
/// server answers the click with a resync.
fn container_click(
    wrapper: &mut PacketWrapper,
    connection: &mut UserConnection,
    ctx: &Ctx,
) -> Result<(), TranslateError> {
    item_pass::click_frame(wrapper, connection.version)?;
    let changed = wrapper.passthrough(&VAR_INT)?.0;
    if !(0..=256).contains(&changed) {
        return Err(TranslateError::Unsupported("changed slot count"));
    }
    for _ in 0..changed {
        wrapper.passthrough(&I16T)?;
        hashed(wrapper, connection.version, ctx.step.from)?;
    }
    hashed(wrapper, connection.version, ctx.step.from)
}

/// Reads one stack as the client sent it and writes the hash core reads,
/// numbered like `from` so the id pass can map it back to 26.3.
fn hashed(
    wrapper: &mut PacketWrapper,
    version: JavaMinecraftVersion,
    from: JavaMinecraftVersion,
) -> Result<(), TranslateError> {
    let item = wrapper.read(&ClientItemT::new(
        version,
        MappingData::get().composed(version),
    ))?;
    let items = &MappingData::get().composed(from).items;
    let (id, count) = match item {
        Item::Empty => (None, 0),
        Item::Structured { id, count, .. } => (Some(id), count),
        Item::Nbt { id, count, .. } => (Some(id), i32::from(count)),
    };
    let out = id
        .and_then(|id| u32::try_from(id).ok())
        .and_then(|id| items.map(id))
        .and_then(|id| i32::try_from(id).ok())
        .map(|id| HashedItem {
            id,
            count,
            added: Vec::new(),
            removed: Vec::new(),
        });
    wrapper.write(&HASHED_ITEM, &out)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::rewriter::item::StructuredItemRewriter;
    use crate::api::types::{ItemT, WireType};
    use crate::pipeline::translate_serverbound;
    use pumpkin_data::item::Item as DataItem;
    use pumpkin_data::item_stack::ItemStack;
    use pumpkin_protocol::ServerPacket;
    use pumpkin_protocol::codec::var_int::VarInt;
    use pumpkin_protocol::java::server::play::SClickSlot;

    const PLAY: u8 = 5;

    /// `md('1.21.4').protocol.play.toServer.packet_window_click`: the changed
    /// slots and the cursor item are whole stacks, not hashes.
    fn sent_by_client(version: JavaMinecraftVersion) -> Vec<u8> {
        let native = Item::Structured {
            count: 1,
            id: i32::from(DataItem::DIAMOND_SWORD.id),
            added: Vec::new(),
            removed: Vec::new(),
        };
        let ids = MappingData::get().composed(version);
        let carried = StructuredItemRewriter::to_version(&native, version, ids);

        let mut bytes = vec![1, 9, 0, 36, 0, 0, 0];
        ItemT::for_version(version)
            .write(&mut bytes, &carried)
            .unwrap();
        bytes
    }

    fn clicked_item(version: JavaMinecraftVersion) -> SClickSlot {
        let out = translate_serverbound(
            0,
            version,
            PLAY,
            serverbound::play::CONTAINER_CLICK.to_id(version),
            &sent_by_client(version),
        )
        .unwrap();
        let mut read: &[u8] = &out.payload;
        let packet = SClickSlot::read(&mut read, &version).unwrap();
        assert!(read.is_empty(), "{version}");
        packet
    }

    #[test]
    fn a_whole_stack_reaches_core_as_a_hash_with_26_3_ids() {
        for version in [
            JavaMinecraftVersion::V_1_21_4,
            JavaMinecraftVersion::V_1_20_2,
            JavaMinecraftVersion::V_1_18_2,
        ] {
            let packet = clicked_item(version);
            assert_eq!(packet.sync_id, VarInt(1), "{version}");
            assert!(
                packet
                    .carried_item
                    .hash_equals(&ItemStack::new(1, &DataItem::DIAMOND_SWORD)),
                "{version}"
            );
        }
    }
}
