use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::types::{BOOL, BYTE_ARRAY, I64T, STRING, VAR_INT};
use crate::api::{Ctx, PacketWrapper, Protocol, Registry, Step, TranslateError, UserConnection};
use crate::packet::mappings::serverbound;

pub struct Protocol1_19_1To1_19;

impl Protocol for Protocol1_19_1To1_19 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_19_1,
            to: JavaMinecraftVersion::V_1_19,
        }
    }

    fn register(&self, reg: &mut Registry) {
        reg.serverbound_layout(&serverbound::play::CHAT, chat);
    }
}

/// 1.19.1 appended the last seen list and the last rejected message.
fn chat(
    wrapper: &mut PacketWrapper,
    _connection: &mut UserConnection,
    _ctx: &Ctx,
) -> Result<(), TranslateError> {
    wrapper.passthrough(&STRING)?;
    wrapper.passthrough(&I64T)?;
    wrapper.passthrough(&I64T)?;
    wrapper.passthrough(&BYTE_ARRAY)?;
    wrapper.passthrough(&BOOL)?;
    wrapper.write(&VAR_INT, &VarInt(0))?;
    wrapper.write(&BOOL, &false)?;
    Ok(())
}
