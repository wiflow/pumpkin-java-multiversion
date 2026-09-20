//! Rewrites the status response so older clients show the server as joinable.
//!
//! The server list entry is driven by the protocol number in the status JSON.
//! A 26.3 server always reports 777, so every older client draws a red cross and
//! "Incompatible version!" even though this plugin lets it connect. Reporting
//! the client's own protocol back to it is what ViaVersion does, and it only
//! affects the server list entry, not the handshake.

use pumpkin_protocol::ser::{NetworkReadExt, NetworkWriteExt};
use pumpkin_util::version::JavaMinecraftVersion;

/// Rewrites `version.protocol` (and `version.name`) in a status response so the
/// client sees itself as compatible.
///
/// Returns `None` if the payload is not the single JSON string we expect, in
/// which case the caller should leave the packet alone.
#[must_use]
pub fn rewrite_status_response(payload: &[u8], version: JavaMinecraftVersion) -> Option<Vec<u8>> {
    let mut cursor = payload;
    let json = cursor.get_str().ok()?;

    let mut value: serde_json::Value = serde_json::from_str(&json).ok()?;
    let version_obj = value.get_mut("version")?.as_object_mut()?;

    version_obj.insert(
        "protocol".to_string(),
        serde_json::Value::from(version.protocol_version()),
    );
    version_obj.insert(
        "name".to_string(),
        serde_json::Value::from(version.to_string()),
    );

    let rewritten = serde_json::to_string(&value).ok()?;
    let mut out = Vec::with_capacity(rewritten.len() + 5);
    out.write_string(&rewritten).ok()?;
    Some(out)
}
