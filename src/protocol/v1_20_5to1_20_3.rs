use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::types::{ItemT, U8, VAR_INT};
use crate::api::{Ctx, PacketWrapper, Protocol, Registry, Step, TranslateError, UserConnection};
use crate::packet::mappings::clientbound;

/// `body`, the animal armour slot 1.20.5 added.
const BODY: u8 = 6;

pub struct Protocol1_20_5To1_20_3;

impl Protocol for Protocol1_20_5To1_20_3 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_20_5,
            to: JavaMinecraftVersion::V_1_20_3,
        }
    }

    fn register(&self, reg: &mut Registry) {
        reg.clientbound(&clientbound::play::SET_EQUIPMENT, equipment);
    }
}

/// The client indexes its slot enum with the raw value, so a slot it does not
/// have is not a wrong item but a crash. The list has no count and ends at the
/// first entry without the continuation bit, so the terminator moves with it.
fn equipment(
    wrapper: &mut PacketWrapper,
    _connection: &mut UserConnection,
    ctx: &Ctx,
) -> Result<(), TranslateError> {
    if ctx.layout < JavaMinecraftVersion::V_1_16 {
        wrapper.passthrough_all();
        return Ok(());
    }
    wrapper.passthrough(&VAR_INT)?;

    let item = ItemT::for_version(ctx.layout);
    let mut kept = Vec::new();
    loop {
        let slot = wrapper.read(&U8)?;
        let value = wrapper.read(&item)?;
        if slot & 0x7F != BODY {
            kept.push((slot & 0x7F, value));
        }
        if slot & 0x80 == 0 {
            break;
        }
    }
    if kept.is_empty() {
        wrapper.cancel();
        return Ok(());
    }

    let last = kept.len() - 1;
    for (index, (slot, value)) in kept.iter().enumerate() {
        let slot = if index == last { *slot } else { slot | 0x80 };
        wrapper.write(&U8, &slot)?;
        wrapper.write(&item, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::WireType;
    use crate::pipeline::translate_clientbound;
    use pumpkin_data::item::Item;
    use pumpkin_data::item_stack::ItemStack;
    use pumpkin_protocol::ClientPacket;
    use pumpkin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
    use pumpkin_protocol::codec::var_int::VarInt;
    use pumpkin_protocol::java::client::play::CSetEquipment;

    const PLAY: u8 = 5;

    fn written_by_core(version: JavaMinecraftVersion, slots: &[(i8, &'static Item)]) -> Vec<u8> {
        let equipment = slots
            .iter()
            .map(|(slot, item)| (*slot, ItemStackSerializer::from(ItemStack::new(1, item))))
            .collect();
        let packet = CSetEquipment::new(VarInt(42), equipment);
        let mut bytes = Vec::new();
        packet.write_packet_data(&mut bytes, &version).unwrap();
        bytes
    }

    fn translate(version: JavaMinecraftVersion, payload: &[u8]) -> Option<Vec<u8>> {
        translate_clientbound(
            0,
            version,
            PLAY,
            clientbound::play::SET_EQUIPMENT.v26_3,
            payload,
        )
        .map(|out| out.payload)
    }

    /// `md('1.20.2').protocol.play.toClient.packet_entity_equipment`: a
    /// `topBitSetTerminatedArray` of slot byte and item.
    #[test]
    fn the_body_slot_leaves_the_list_and_the_terminator_moves() {
        let version = JavaMinecraftVersion::V_1_20_3;
        let payload = written_by_core(
            version,
            &[
                (BODY as i8, &Item::DIAMOND_HORSE_ARMOR),
                (4, &Item::DIAMOND_CHESTPLATE),
            ],
        );
        let out = translate(version, &payload).unwrap();

        let mut read: &[u8] = &out;
        assert_eq!(VAR_INT.read(&mut read).unwrap().0, 42);
        assert_eq!(U8.read(&mut read).unwrap(), 4, "the only entry left");
        ItemT::for_version(version).read(&mut read).unwrap();
        assert!(read.is_empty());
    }

    #[test]
    fn a_body_only_packet_is_dropped() {
        let version = JavaMinecraftVersion::V_1_20_3;
        let payload = written_by_core(version, &[(BODY as i8, &Item::DIAMOND_HORSE_ARMOR)]);
        assert!(translate(version, &payload).is_none());
    }

    #[test]
    fn the_body_slot_is_kept_from_1_20_5() {
        let version = JavaMinecraftVersion::V_1_20_5;
        let payload = written_by_core(version, &[(BODY as i8, &Item::DIAMOND_HORSE_ARMOR)]);
        let out = translate(version, &payload).unwrap();

        let mut read: &[u8] = &out;
        assert_eq!(VAR_INT.read(&mut read).unwrap().0, 42);
        assert_eq!(U8.read(&mut read).unwrap(), BODY);
    }

    /// The whole tier below 1.20.5 carries the same list, item form aside.
    #[test]
    fn the_terminator_is_rebuilt_on_the_nbt_form_too() {
        let version = JavaMinecraftVersion::V_1_16_2;
        let payload = written_by_core(
            version,
            &[
                (2, &Item::DIAMOND_BOOTS),
                (BODY as i8, &Item::DIAMOND_HORSE_ARMOR),
                (4, &Item::DIAMOND_CHESTPLATE),
            ],
        );
        let out = translate(version, &payload).unwrap();

        let mut read: &[u8] = &out;
        assert_eq!(VAR_INT.read(&mut read).unwrap().0, 42);
        assert_eq!(U8.read(&mut read).unwrap(), 2 | 0x80);
        ItemT::for_version(version).read(&mut read).unwrap();
        assert_eq!(U8.read(&mut read).unwrap(), 4);
        ItemT::for_version(version).read(&mut read).unwrap();
        assert!(read.is_empty());
    }
}
