use pumpkin_protocol::ser::NetworkReadExt;
use pumpkin_protocol::{ClientPacket, java::client::play::CSpawnEntity};
use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{PacketWrapper, TranslateError, UserConnection};
use crate::packet::mappings::PacketId;

/// The 26.3 entity type the tracker holds for the spawn payload in `wrapper`.
#[must_use]
pub fn spawned_type(wrapper: &PacketWrapper, connection: &UserConnection) -> Option<u16> {
    let mut cursor = wrapper.remaining();
    let entity_id = cursor.get_var_int().ok()?.0;
    connection.entity_tracker.entity_type(entity_id)
}

pub fn read_spawn(
    wrapper: &PacketWrapper,
    layout: JavaMinecraftVersion,
) -> Result<CSpawnEntity, TranslateError> {
    Ok(CSpawnEntity::read_packet_data(
        wrapper.remaining(),
        &layout,
    )?)
}

/// Sends `value` under `packet` instead of what is left of the input.
pub fn replace<P: ClientPacket>(
    wrapper: &mut PacketWrapper,
    packet: &'static PacketId,
    value: &P,
    layout: JavaMinecraftVersion,
) -> Result<(), TranslateError> {
    let mut buf = Vec::new();
    value.write_packet_data(&mut buf, &layout)?;
    wrapper.replace_remaining(buf);
    wrapper.set_packet(packet);
    Ok(())
}
