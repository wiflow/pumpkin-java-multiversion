use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::types::{BOOL, BYTE_ARRAY, I64T, STRING, U8, UUID, VAR_INT};
use crate::api::{Ctx, PacketWrapper, Protocol, Registry, Step, TranslateError, UserConnection};
use crate::packet::mappings::serverbound;

pub struct Protocol1_19_3To1_19_1;

impl Protocol for Protocol1_19_3To1_19_1 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_19_3,
            to: JavaMinecraftVersion::V_1_19_1,
        }
    }

    fn register(&self, reg: &mut Registry) {
        reg.serverbound_layout(&serverbound::play::CHAT, chat);
    }
}

/// 1.19.3 turned the signature into a fixed 256 byte option and replaced the
/// last seen list with a counter and an acknowledgement set. The signature is
/// over the older scheme and cannot be carried over, so it is left out; core
/// keeps its own pre-1.19.3 branch, which reads the signed preview byte in
/// place of the counter.
fn chat(
    wrapper: &mut PacketWrapper,
    _connection: &mut UserConnection,
    _ctx: &Ctx,
) -> Result<(), TranslateError> {
    wrapper.passthrough(&STRING)?;
    wrapper.passthrough(&I64T)?;
    wrapper.passthrough(&I64T)?;
    wrapper.read(&BYTE_ARRAY)?;
    wrapper.read(&BOOL)?;
    let seen = wrapper.read(&VAR_INT)?.0;
    if !(0..=5).contains(&seen) {
        return Err(TranslateError::Unsupported("last seen message count"));
    }
    for _ in 0..seen {
        wrapper.read(&UUID)?;
        wrapper.read(&BYTE_ARRAY)?;
    }
    if wrapper.read(&BOOL)? {
        wrapper.read(&UUID)?;
        wrapper.read(&BYTE_ARRAY)?;
    }
    wrapper.write(&BOOL, &false)?;
    wrapper.write(&U8, &0)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::WireType;
    use crate::pipeline::translate_serverbound;
    use pumpkin_protocol::ServerPacket;
    use pumpkin_protocol::java::server::play::SChatMessage;

    const PLAY: u8 = 5;

    /// `md('1.19').protocol.play.toServer.packet_chat_message`: message,
    /// timestamp, salt, a varint prefixed signature and the signed preview
    /// flag. 1.19.2 adds the last seen list and the last rejected message.
    fn sent_by_client(version: JavaMinecraftVersion) -> Vec<u8> {
        let mut bytes = Vec::new();
        STRING.write(&mut bytes, &"hello".into()).unwrap();
        I64T.write(&mut bytes, &7).unwrap();
        I64T.write(&mut bytes, &9).unwrap();
        BYTE_ARRAY.write(&mut bytes, &vec![0xab; 256]).unwrap();
        BOOL.write(&mut bytes, &false).unwrap();
        if version >= JavaMinecraftVersion::V_1_19_1 {
            VAR_INT
                .write(&mut bytes, &pumpkin_protocol::codec::var_int::VarInt(1))
                .unwrap();
            UUID.write(&mut bytes, &uuid::Uuid::from_u128(3)).unwrap();
            BYTE_ARRAY.write(&mut bytes, &vec![0xcd; 256]).unwrap();
            BOOL.write(&mut bytes, &true).unwrap();
            UUID.write(&mut bytes, &uuid::Uuid::from_u128(4)).unwrap();
            BYTE_ARRAY.write(&mut bytes, &vec![0xef; 256]).unwrap();
        }
        bytes
    }

    fn read_by_core(version: JavaMinecraftVersion) {
        let out = translate_serverbound(
            0,
            version,
            PLAY,
            serverbound::play::CHAT.to_id(version),
            &sent_by_client(version),
        )
        .unwrap();
        let mut read: &[u8] = &out.payload;
        let packet = SChatMessage::read(&mut read, &version).unwrap();
        assert!(read.is_empty(), "{version}");
        assert_eq!(packet.message, "hello", "{version}");
        assert_eq!(packet.timestamp, 7, "{version}");
        assert_eq!(packet.salt, 9, "{version}");
        assert!(packet.signature.is_none(), "{version}");
    }

    #[test]
    fn the_signed_chat_forms_below_1_19_3_reach_core() {
        read_by_core(JavaMinecraftVersion::V_1_19);
        read_by_core(JavaMinecraftVersion::V_1_19_1);
    }

    /// Below 1.19 the packet is the message alone and core reads it as sent.
    #[test]
    fn a_plain_message_is_left_alone() {
        let version = JavaMinecraftVersion::V_1_18_2;
        let mut payload = Vec::new();
        STRING.write(&mut payload, &"hello".into()).unwrap();
        let out = translate_serverbound(
            0,
            version,
            PLAY,
            serverbound::play::CHAT.to_id(version),
            &payload,
        )
        .unwrap();
        assert_eq!(out.payload, payload);
    }
}
