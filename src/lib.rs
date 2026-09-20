#[cfg(not(target_family = "wasm"))]
pub mod chunk;
pub mod data;
pub mod packet;
pub mod registry;
pub mod remap;
pub mod tag;

use pumpkin_plugin_api::{
    Context, Plugin, PluginMetadata, Server,
    events::{
        EventHandler, EventPriority,
        packet::{PacketReceivedEvent, PacketSentEvent},
    },
    events_wit::{PacketReceivedEventData, PacketSentEventData},
    register_plugin,
};

use crate::packet::translator::PacketTranslator;
use crate::packet::{HIGHEST_SUPPORTED, LOWEST_SUPPORTED, is_version_supported};
use pumpkin_protocol::ser::NetworkWriteExt;
use pumpkin_util::version::JavaMinecraftVersion;

/// The multi-version plugin allowing older Minecraft Java clients (from
/// [`LOWEST_SUPPORTED`]) to connect to a Pumpkin 26.3 server.
pub struct MultiVersionPlugin;

impl Plugin for MultiVersionPlugin {
    fn new() -> Self {
        Self
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "pumpkin-java-multiversion".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            authors: vec!["Pumpkin Developer".into()],
            description: "Multi-version Java Edition protocol translation plugin for Pumpkin."
                .into(),
            dependencies: vec![],
            permissions: vec![],
        }
    }

    fn on_load(&self, context: Context) -> Result<(), String> {
        tracing::info!("Loading Pumpkin Java Multi-Version Plugin...");

        // Register packet event handlers with High priority to translate before/after game logic
        context.register_event_handler(PacketReceivedHandler, EventPriority::Highest, true)?;

        context.register_event_handler(PacketSentHandler, EventPriority::Lowest, true)?;

        tracing::info!(
            "Pumpkin Java Multi-Version Plugin enabled! Supporting {LOWEST_SUPPORTED} - {HIGHEST_SUPPORTED}"
        );
        Ok(())
    }

    fn on_unload(&self, _context: Context) -> Result<(), String> {
        tracing::info!("Unloading Pumpkin Java Multi-Version Plugin");
        Ok(())
    }
}

/// Handles incoming packets from clients and translates them if the client is on an older version.
struct PacketReceivedHandler;

impl EventHandler<PacketReceivedEvent> for PacketReceivedHandler {
    fn handle(
        &self,
        _server: Server,
        mut event: PacketReceivedEventData,
    ) -> PacketReceivedEventData {
        // Login and configuration packets arrive before a Player exists, so
        // the version comes off the event itself rather than off the player.
        let version = JavaMinecraftVersion::from_protocol(event.protocol_version as u32);
        if version == JavaMinecraftVersion::V_26_3 {
            return event;
        }
        // An unsupported client is refused on its first clientbound login
        // packet (see `refuse_unsupported`); its login start has to reach the
        // server untouched for that packet to be sent at all.
        if !is_version_supported(version) {
            return event;
        }
        // Handshake and status ids never changed and carry no version yet.
        if event.connection_state < 2 {
            return event;
        }
        match PacketTranslator::translate_incoming_packet(
            event.packet_id,
            &event.raw_payload,
            version,
            event.connection_state,
        ) {
            Some((new_id, new_payload)) => {
                event.packet_id = new_id;
                event.raw_payload = new_payload;
            }
            // No 26.3 equivalent. Forwarding it unchanged makes the server
            // read the id as whatever packet now occupies that slot and
            // desync the stream, so drop it instead.
            None => event.cancelled = true,
        }
        event
    }
}

/// Handles outgoing packets to clients and translates them to match the client's expected version.
struct PacketSentHandler;

impl EventHandler<PacketSentEvent> for PacketSentHandler {
    fn handle(&self, _server: Server, mut event: PacketSentEventData) -> PacketSentEventData {
        // Login and configuration packets are sent before a Player exists, so the
        // version comes off the event itself rather than off the player.
        let version = JavaMinecraftVersion::from_protocol(event.protocol_version as u32);
        if version == JavaMinecraftVersion::V_26_3 {
            return event;
        }
        if !is_version_supported(version) {
            return refuse_unsupported(event, version);
        }
        match PacketTranslator::translate_outgoing_packet(
            event.packet_id,
            &event.raw_payload,
            version,
            event.connection_state,
        ) {
            Some((new_id, new_payload)) => {
                event.packet_id = new_id;
                event.raw_payload = new_payload;
            }
            // No id for this version: the packet does not exist on the client.
            // Sending it under a 26.3 id would desync the stream, so drop it.
            None => event.cancelled = true,
        }
        event
    }
}

/// Turns the first login packet for a client below the supported floor into a
/// disconnect with a readable reason, and drops everything else meant for it.
///
/// The status response is left alone on purpose: with the server's own
/// protocol in it the client's server list already shows the version as
/// incompatible. During login the server's first packet (compression or the
/// game profile) is replaced, so the client reads the disconnect before any
/// compression takes effect on its side.
///
/// This keeps working for the versions just under the tier 4 floor. The login
/// `DISCONNECT` is packet 0 on every version minecraft-data has, 1.16.1 (736)
/// and 1.12.2 (340) included, and its single field is a JSON chat string on
/// all of them, which is exactly what is written here. `to_id` also falls back
/// to the 26.3 column for [`JavaMinecraftVersion::Unknown`], where that column
/// is 0 as well, so a protocol number no version claims is refused rather than
/// silently let through.
fn refuse_unsupported(
    mut event: PacketSentEventData,
    version: JavaMinecraftVersion,
) -> PacketSentEventData {
    // 0 handshake, 1 status, 2 login, 3 transfer, 4 config, 5 play.
    if event.connection_state != 2 && event.connection_state != 3 {
        event.cancelled = event.connection_state != 1;
        return event;
    }
    let disconnect_id = packet::mappings::clientbound::login::LOGIN_DISCONNECT.to_id(version);
    if disconnect_id == -1 {
        event.cancelled = true;
        return event;
    }
    let reason = serde_json::json!({
        "text": format!(
            "This server supports Minecraft {LOWEST_SUPPORTED} to {HIGHEST_SUPPORTED}. You are on {version}."
        ),
        "color": "red"
    });
    let mut payload = Vec::new();
    if payload.write_string(&reason.to_string()).is_err() {
        event.cancelled = true;
        return event;
    }
    event.packet_id = disconnect_id;
    event.raw_payload = payload;
    event
}

register_plugin!(MultiVersionPlugin);

#[cfg(test)]
mod tests {
    use crate::packet::mappings::clientbound::login::LOGIN_DISCONNECT;
    use crate::packet::{LOWEST_SUPPORTED, is_version_supported};
    use pumpkin_util::version::JavaMinecraftVersion;

    /// `refuse_unsupported` can only send the disconnect if the version has an
    /// id for it. Checked against minecraft-data, where `login.toClient`
    /// `disconnect` is `0x00` for 1.12.2, 1.16.1 and every version between.
    #[test]
    fn versions_below_the_floor_still_have_a_login_disconnect() {
        for version in [
            JavaMinecraftVersion::V_1_12_2,
            JavaMinecraftVersion::V_1_13_2,
            JavaMinecraftVersion::V_1_16,
            JavaMinecraftVersion::V_1_16_1,
            JavaMinecraftVersion::Unknown,
        ] {
            assert!(
                !is_version_supported(version),
                "{version} should be below the floor"
            );
            assert_eq!(
                LOGIN_DISCONNECT.to_id(version),
                0,
                "{version} has no login disconnect id to refuse it with"
            );
        }
    }

    /// The floor itself, and the version directly under it, are on the right
    /// sides of the comparison the handler uses.
    #[test]
    fn the_floor_is_1_16_2() {
        assert_eq!(LOWEST_SUPPORTED, JavaMinecraftVersion::V_1_16_2);
        assert!(is_version_supported(JavaMinecraftVersion::V_1_16_2));
        assert!(is_version_supported(JavaMinecraftVersion::V_1_20));
        assert!(!is_version_supported(JavaMinecraftVersion::V_1_16_1));
        assert!(!is_version_supported(JavaMinecraftVersion::Unknown));
    }
}
