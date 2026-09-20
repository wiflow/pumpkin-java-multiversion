#[cfg(not(target_family = "wasm"))]
pub mod chunk;
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

use crate::packet::translator::{PacketTranslator, from_wasm_java_version};
use pumpkin_util::version::JavaMinecraftVersion;

/// The multi-version plugin allowing Minecraft Java clients across versions (1.7 - 26.2)
/// to connect to a Pumpkin 26.3 server.
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

        tracing::info!("Pumpkin Java Multi-Version Plugin enabled! Supporting 1.7.2 - 26.3");
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
        if let Some(java_player) = event.player.as_java() {
            let version = from_wasm_java_version(java_player.get_version());
            // PacketReceivedEvent is only fired for play-state packets.
            match PacketTranslator::translate_incoming_packet(
                event.packet_id,
                &event.raw_payload,
                version,
                5,
            ) {
                Some((new_id, new_payload)) => {
                    event.packet_id = new_id;
                    event.raw_payload = new_payload;
                }
                // No 26.3 equivalent. Forwarding it unchanged makes the server
                // read the id as whatever packet now occupies that slot and
                // desync the stream, so drop it instead.
                None if version != JavaMinecraftVersion::V_26_3 => {
                    event.cancelled = true;
                }
                None => {}
            }
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
        if version == JavaMinecraftVersion::Unknown || version == JavaMinecraftVersion::V_26_3 {
            return event;
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

register_plugin!(MultiVersionPlugin);
