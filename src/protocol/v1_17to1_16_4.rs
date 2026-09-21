use crate::api::types::{BOOL, I8, I16T, U8, VAR_INT, WireType};
use crate::api::{Ctx, PacketWrapper, Protocol, Registry, Step, TranslateError, UserConnection};
use crate::packet::chunk_legacy;
use crate::packet::mappings::{clientbound, serverbound};
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::version::JavaMinecraftVersion;

pub struct Protocol1_17To1_16_4;

impl Protocol for Protocol1_17To1_16_4 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_17,
            to: JavaMinecraftVersion::V_1_16_4,
        }
    }

    fn register(&self, reg: &mut Registry) {
        reg.serverbound_layout(&serverbound::play::CONTAINER_CLICK, container_click);
        reg.clientbound_layout(&clientbound::play::LEVEL_CHUNK_WITH_LIGHT, chunk);
    }
}

/// 1.17 dropped the action number and sends the slots the click changed
/// instead of the clicked stack. Nothing here can predict those, so the list
/// goes out empty and the clicked stack becomes the carried one, which is what
/// it usually is. The action number is answered with the confirmation the
/// client waits for before it accepts another click.
fn container_click(
    wrapper: &mut PacketWrapper,
    _connection: &mut UserConnection,
    _ctx: &Ctx,
) -> Result<(), TranslateError> {
    let window = wrapper.read(&U8)?;
    let slot = wrapper.read(&I16T)?;
    let button = wrapper.read(&I8)?;
    let action = wrapper.read(&I16T)?;
    let mode = wrapper.read(&U8)?;
    if mode > 6 {
        return Err(TranslateError::Unsupported("slot action"));
    }
    wrapper.write(&U8, &window)?;
    wrapper.write(&I16T, &slot)?;
    wrapper.write(&I8, &button)?;
    wrapper.write(&U8, &mode)?;
    wrapper.write(&VAR_INT, &VarInt(0))?;
    wrapper.passthrough_all();

    wrapper.send_reply(
        &clientbound::play::WINDOW_CONFIRMATION,
        confirmation(window as i8, action)?,
    );
    Ok(())
}

fn confirmation(window: i8, action: i16) -> Result<Vec<u8>, TranslateError> {
    let mut payload = Vec::with_capacity(4);
    I8.write(&mut payload, &window)?;
    I16T.write(&mut payload, &action)?;
    BOOL.write(&mut payload, &true)?;
    Ok(payload)
}

/// 1.16 reads a "full chunk" flag and a varint mask where 1.17 reads a bit set.
fn chunk(
    wrapper: &mut PacketWrapper,
    _connection: &mut UserConnection,
    ctx: &Ctx,
) -> Result<(), TranslateError> {
    let out = chunk_legacy::to_v1_16(wrapper.remaining(), &ctx.mappings.blockstates)
        .ok_or(TranslateError::Unsupported("chunk"))?;
    wrapper.replace_remaining(out);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::translate_serverbound;
    use pumpkin_protocol::ServerPacket;
    use pumpkin_protocol::java::server::play::SClickSlot;

    const PLAY: u8 = 5;

    /// `md('1.16.2').protocol.play.toServer.packet_window_click`: window id,
    /// slot, mouse button, action number, mode and the clicked stack.
    fn sent_by_client() -> Vec<u8> {
        vec![1, 0, 36, 0, 0, 7, 0, 0]
    }

    #[test]
    fn a_1_16_click_reaches_core_with_a_state_id_and_no_changed_slots() {
        let version = JavaMinecraftVersion::V_1_16_2;
        let out = translate_serverbound(
            0,
            version,
            PLAY,
            serverbound::play::CONTAINER_CLICK.to_id(version),
            &sent_by_client(),
        )
        .unwrap();

        let mut read: &[u8] = &out.payload;
        let packet = SClickSlot::read(&mut read, &version).unwrap();
        assert!(read.is_empty());
        assert_eq!(packet.sync_id, VarInt(1));
        assert_eq!(packet.revision, VarInt(-1), "forces a resync");
        assert_eq!(packet.slot, 36);
        assert_eq!(packet.button, 0);
        assert_eq!(packet.length_of_array, VarInt(0));
        assert!(packet.carried_item.0.is_none());
    }

    #[test]
    fn the_click_is_answered_with_the_confirmation_the_client_waits_for() {
        let version = JavaMinecraftVersion::V_1_16_2;
        let out = translate_serverbound(
            0,
            version,
            PLAY,
            serverbound::play::CONTAINER_CLICK.to_id(version),
            &sent_by_client(),
        )
        .unwrap();

        let (packet, payload) = out.replies.first().expect("a confirmation");
        assert_eq!(
            packet.to_id(version),
            clientbound::play::WINDOW_CONFIRMATION.to_id(version)
        );
        assert_eq!(payload, &[1, 0, 7, 1], "window, action number, accepted");
    }

    #[test]
    fn a_1_17_click_is_not_answered() {
        let version = JavaMinecraftVersion::V_1_17;
        let out = translate_serverbound(
            0,
            version,
            PLAY,
            serverbound::play::CONTAINER_CLICK.to_id(version),
            &[1, 0, 36, 0, 0, 0, 0],
        )
        .unwrap();
        assert!(out.replies.is_empty());
    }
}
