//! Generates `src/registry/generated.rs` for the multiversion plugin.
//!
//! Reads `assets/datapacks/<version>/data/minecraft/<registry>/*.json` and emits
//! one `REGISTRY_V_<VERSION>` static per version, holding pre-serialized NBT for
//! every synced registry entry. The JSON to NBT conversion mirrors Pumpkin's
//! `tools/pumpkin-codegen/src/registry.rs`, so the bytes match what the server
//! itself produces for its own version.
//!
//! Run from the repository root: `cargo run --manifest-path tools/registry-codegen/Cargo.toml`

use std::fs;
use std::path::Path;

use indexmap::IndexMap;
use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use serde_json::Value;

/// Registries the server sends to the client during configuration.
const SYNCED_REGISTRIES: &[&str] = &[
    "worldgen/biome",
    "chat_type",
    "trim_pattern",
    "trim_material",
    "wolf_variant",
    "wolf_sound_variant",
    "pig_variant",
    "pig_sound_variant",
    "frog_variant",
    "cat_variant",
    "cat_sound_variant",
    "cow_variant",
    "cow_sound_variant",
    "chicken_variant",
    "chicken_sound_variant",
    "zombie_nautilus_variant",
    "painting_variant",
    "dimension_type",
    "damage_type",
    "jukebox_song",
    "banner_pattern",
    "instrument",
    "enchantment",
    "timeline",
    "dialog",
    "world_clock",
    "test_environment",
    "test_instance",
    "sulfur_cube_archetype",
    "decorated_pot_pattern",
    "block_transformer",
    "worldgen/block_state_provider",
];

/// Datapack folder name paired with the `JavaMinecraftVersion` variant suffix.
const VERSIONS: &[(&str, &str)] = &[
    ("1_21_5", "V_1_21_5"),
    ("1_21_6", "V_1_21_6"),
    ("1_21_7", "V_1_21_7"),
    ("1_21_9", "V_1_21_9"),
    ("1_21_11", "V_1_21_11"),
    ("26_1", "V_26_1"),
    ("26_2", "V_26_2"),
];

fn json_to_nbt_tag(v: &Value) -> pumpkin_nbt::tag::NbtTag {
    match v {
        Value::Null => pumpkin_nbt::tag::NbtTag::End,
        Value::Bool(b) => pumpkin_nbt::tag::NbtTag::Byte(i8::from(*b)),
        Value::Number(num) => {
            if let Some(i) = num.as_i64() {
                if i >= i64::from(i32::MIN) && i <= i64::from(i32::MAX) {
                    pumpkin_nbt::tag::NbtTag::Int(i as i32)
                } else {
                    pumpkin_nbt::tag::NbtTag::Long(i)
                }
            } else if let Some(f) = num.as_f64() {
                pumpkin_nbt::tag::NbtTag::Double(f)
            } else {
                pumpkin_nbt::tag::NbtTag::Int(0)
            }
        }
        Value::String(s) => pumpkin_nbt::tag::NbtTag::String(s.clone().into()),
        Value::Array(arr) => {
            pumpkin_nbt::tag::NbtTag::List(arr.iter().map(json_to_nbt_tag).collect())
        }
        Value::Object(obj) => {
            let mut compound = pumpkin_nbt::compound::NbtCompound::new();
            for (k, val) in obj {
                compound.put(k, json_to_nbt_tag(val));
            }
            pumpkin_nbt::tag::NbtTag::Compound(compound)
        }
    }
}

fn entry_bytes(entry_data: &Value) -> Vec<u8> {
    match json_to_nbt_tag(entry_data) {
        pumpkin_nbt::tag::NbtTag::Compound(compound) => {
            pumpkin_nbt::Nbt::from(compound).write_unnamed().to_vec()
        }
        other => {
            let mut bytes = Vec::new();
            let mut writer = pumpkin_nbt::serializer::NbtWriteHelperJava::new(&mut bytes);
            let _ = other.serialize(&mut writer);
            bytes
        }
    }
}

fn process_version(ver_folder: &str) -> TokenStream {
    let base_path = Path::new("assets/datapacks").join(ver_folder).join("data/minecraft");

    let mut data: IndexMap<String, IndexMap<String, Value>> = IndexMap::new();

    for &reg_name in SYNCED_REGISTRIES {
        let reg_dir = base_path.join(reg_name);
        if !reg_dir.is_dir() {
            continue;
        }
        let mut entries = IndexMap::new();
        let mut paths: Vec<_> = fs::read_dir(&reg_dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort_by_key(std::fs::DirEntry::path);

        for entry in paths {
            let path = entry.path();
            let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                continue;
            };
            if let Ok(content) = fs::read_to_string(&path)
                && let Ok(val) = serde_json::from_str::<Value>(&content)
            {
                entries.insert(stem, val);
            }
        }

        if !entries.is_empty() {
            data.insert(reg_name.to_string(), entries);
        }
    }

    // The vanilla server synthesises a "raw" chat type; datapacks do not carry it.
    if let Some(chat) = data.get_mut("chat_type") {
        chat.insert(
            "raw".to_string(),
            serde_json::json!({
                "chat": { "translation_key": "%s", "parameters": ["content"] },
                "narration": { "translation_key": "%s says %s", "parameters": ["sender", "content"] }
            }),
        );
    }

    let reg_tokens: Vec<TokenStream> = data
        .iter()
        .map(|(reg_name, entries)| {
            let entry_tokens: Vec<TokenStream> = entries
                .iter()
                .map(|(entry_name, entry_data)| {
                    let byte_literal = Literal::byte_string(&entry_bytes(entry_data));
                    quote! {
                        StaticRegistryEntry { name: #entry_name, data: #byte_literal }
                    }
                })
                .collect();
            quote! {
                StaticRegistry {
                    registry_id: #reg_name,
                    entries: &[#(#entry_tokens),*],
                }
            }
        })
        .collect();

    quote! { &[#(#reg_tokens),*] }
}

/// Registry keys that have a `tags/` folder in `ver_folder`'s datapack: the
/// registries that version's client can resolve tags for. A key is the first
/// path segment, or the first two under `worldgen/`; deeper folders are tag
/// namespaces (`block/mineable`), not registries.
fn tag_registries(ver_folder: &str) -> Vec<String> {
    let tags_dir = Path::new("assets/datapacks")
        .join(ver_folder)
        .join("data/minecraft/tags");
    let mut keys = std::collections::BTreeSet::new();
    let mut stack = vec![tags_dir.clone()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(rel) = path.strip_prefix(&tags_dir)
            {
                let segments: Vec<String> = rel
                    .parent()
                    .map(|p| {
                        p.components()
                            .map(|c| c.as_os_str().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                let take = if segments.first().is_some_and(|s| s == "worldgen") {
                    2
                } else {
                    1
                };
                if segments.len() >= take {
                    keys.insert(segments[..take].join("/"));
                }
            }
        }
    }
    keys.into_iter().collect()
}

/// Registries whose tag members can be resolved to 26.3 numeric ids by name.
const STATIC_TAG_REGISTRIES: &[&str] = &["block", "item", "entity_type"];

fn static_id(registry: &str, name: &str) -> Option<u16> {
    match registry {
        "block" => pumpkin_data::Block::from_name(name).map(|b| u16::from(b.id)),
        "item" => pumpkin_data::item::Item::from_registry_key(name).map(|i| i.id),
        "entity_type" => pumpkin_data::entity::EntityType::from_name(name).map(|e| e.id),
        _ => None,
    }
}

/// Reads one tag file, following `#other` references inside the same datapack.
fn tag_members(tags_dir: &Path, registry: &str, tag: &str, depth: usize) -> Vec<u16> {
    if depth > 8 {
        return Vec::new();
    }
    let path = tags_dir.join(registry).join(format!("{tag}.json"));
    let Ok(content) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<Value>(&content) else {
        return Vec::new();
    };
    let mut members = Vec::new();
    for value in json.get("values").and_then(Value::as_array).into_iter().flatten() {
        let name = match value {
            Value::String(s) => s.as_str(),
            Value::Object(o) => o.get("id").and_then(Value::as_str).unwrap_or(""),
            _ => "",
        };
        if let Some(nested) = name.strip_prefix('#') {
            let nested = nested.strip_prefix("minecraft:").unwrap_or(nested);
            members.extend(tag_members(tags_dir, registry, nested, depth + 1));
        } else if let Some(id) = static_id(registry, name) {
            members.push(id);
        }
    }
    members.sort_unstable();
    members.dedup();
    members
}

/// Tags in `ver_folder`'s datapack that 26.3's datapack does not have, for
/// the registries whose members can be numbered. Older registry entries still
/// reference them (`enchantable/sword` on 1.21.5), and the client fails the
/// whole registry load when a referenced tag never arrives.
fn extra_tags(ver_folder: &str) -> Vec<(String, String, Vec<u16>)> {
    let old_dir = Path::new("assets/datapacks").join(ver_folder).join("data/minecraft/tags");
    let new_dir = Path::new("assets/datapacks/26_3/data/minecraft/tags");
    let mut out = Vec::new();
    for &registry in STATIC_TAG_REGISTRIES {
        let reg_dir = old_dir.join(registry);
        let mut stack = vec![reg_dir.clone()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !path.extension().is_some_and(|ext| ext == "json") {
                    continue;
                }
                let Ok(rel) = path.strip_prefix(&reg_dir) else {
                    continue;
                };
                if new_dir.join(registry).join(rel).exists() {
                    continue;
                }
                let tag = rel
                    .with_extension("")
                    .to_string_lossy()
                    .replace('\\', "/");
                let members = tag_members(&old_dir, registry, &tag, 0);
                out.push((registry.to_string(), tag, members));
            }
        }
    }
    out.sort();
    out
}

fn main() {
    let mut statics = TokenStream::new();
    let mut match_arms = TokenStream::new();
    let mut tag_arms = TokenStream::new();
    let mut extra_arms = TokenStream::new();

    for (ver_folder, ident_str) in VERSIONS {
        if !Path::new("assets/datapacks").join(ver_folder).is_dir() {
            eprintln!("skipping {ver_folder}: no datapack folder");
            continue;
        }
        eprintln!("generating registries for {ver_folder}");
        let registries = process_version(ver_folder);
        let ident = format_ident!("REGISTRY_{ident_str}");
        statics.extend(quote! {
            pub static #ident: &[StaticRegistry] = #registries;
        });
        let ver_ident = format_ident!("{ident_str}");
        match_arms.extend(quote! {
            JavaMinecraftVersion::#ver_ident => Some(#ident),
        });
        let tag_keys = tag_registries(ver_folder);
        let tag_ident = format_ident!("TAG_REGISTRIES_{ident_str}");
        statics.extend(quote! {
            pub static #tag_ident: &[&str] = &[#(#tag_keys),*];
        });
        tag_arms.extend(quote! {
            JavaMinecraftVersion::#ver_ident => Some(#tag_ident),
        });
        let extras = extra_tags(ver_folder);
        eprintln!("  {} extra tags for {ver_folder}", extras.len());
        let extra_tokens: Vec<TokenStream> = extras
            .iter()
            .map(|(registry, tag, members)| {
                quote! { ExtraTag { registry: #registry, tag: #tag, members: &[#(#members),*] } }
            })
            .collect();
        let extra_ident = format_ident!("EXTRA_TAGS_{ident_str}");
        statics.extend(quote! {
            pub static #extra_ident: &[ExtraTag] = &[#(#extra_tokens),*];
        });
        extra_arms.extend(quote! {
            JavaMinecraftVersion::#ver_ident => Some(#extra_ident),
        });
    }

    let out = quote! {
        use pumpkin_util::version::JavaMinecraftVersion;

        pub struct StaticRegistryEntry {
            pub name: &'static str,
            pub data: &'static [u8],
        }

        pub struct StaticRegistry {
            pub registry_id: &'static str,
            pub entries: &'static [StaticRegistryEntry],
        }

        /// A tag this version has that 26.3 does not, with its members as
        /// 26.3 ids.
        pub struct ExtraTag {
            pub registry: &'static str,
            pub tag: &'static str,
            pub members: &'static [u16],
        }

        #statics

        /// Returns the synced registries for `version`, or `None` when no
        /// registry data has been generated for it.
        #[must_use]
        pub const fn get_synced(
            version: JavaMinecraftVersion,
        ) -> Option<&'static [StaticRegistry]> {
            match version {
                #match_arms
                _ => None,
            }
        }

        /// Returns the registries `version` can resolve tags for, or `None`
        /// when no data has been generated for it.
        #[must_use]
        pub const fn get_tag_registries(
            version: JavaMinecraftVersion,
        ) -> Option<&'static [&'static str]> {
            match version {
                #tag_arms
                _ => None,
            }
        }

        /// Returns the tags `version` has that 26.3 does not, or `None` when
        /// no data has been generated for it.
        #[must_use]
        pub const fn get_extra_tags(
            version: JavaMinecraftVersion,
        ) -> Option<&'static [ExtraTag]> {
            match version {
                #extra_arms
                _ => None,
            }
        }
    };

    let header = "/* This file is generated. Do not edit manually. */\n";
    let code = format!("{header}{out}");
    fs::create_dir_all("src/registry").expect("create src/registry");
    fs::write("src/registry/generated.rs", code).expect("write generated.rs");
    eprintln!("wrote src/registry/generated.rs");
}
