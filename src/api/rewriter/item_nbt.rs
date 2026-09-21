use std::borrow::Cow;

use pumpkin_data::AttributeModifierSlot;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{
    AttributeModifiersImpl, BlockEntityDataImpl, CustomDataImpl, CustomModelDataImpl,
    CustomNameImpl, DamageImpl, DataComponentImpl, DyedColorImpl, EnchantmentsImpl, LoreImpl,
    MapIdImpl, Modifier, Operation, PotionContentsImpl, ProfileImpl, ProfileProperty,
    RepairCostImpl, StoredEnchantmentsImpl, TrimImpl, UnbreakableImpl, WritableBookContentImpl,
    WrittenBookContentImpl,
};
use pumpkin_data::enchantment::Enchantment;
use pumpkin_data::potion::Potion;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_protocol::codec::data_component;
use pumpkin_util::text::TextComponent;
use pumpkin_util::version::JavaMinecraftVersion as V;

use crate::api::ComposedMappings;
use crate::api::rewriter::item_component::legacy_modifier_uuid;
use crate::api::types::ItemComponent;

type ComponentPatch = Vec<(DataComponent, Option<Box<dyn DataComponentImpl>>)>;

/// The enchantments every client from 1.13.2 up has, by registry name
/// (`md('1.13.2').enchantmentsArray`).
static ENCHANTMENTS_1_13: &[&str] = &[
    "aqua_affinity",
    "bane_of_arthropods",
    "binding_curse",
    "blast_protection",
    "channeling",
    "depth_strider",
    "efficiency",
    "feather_falling",
    "fire_aspect",
    "fire_protection",
    "flame",
    "fortune",
    "frost_walker",
    "impaling",
    "infinity",
    "knockback",
    "looting",
    "loyalty",
    "luck_of_the_sea",
    "lure",
    "mending",
    "power",
    "projectile_protection",
    "protection",
    "punch",
    "respiration",
    "riptide",
    "sharpness",
    "silk_touch",
    "smite",
    "sweeping",
    "thorns",
    "unbreaking",
    "vanishing_curse",
];

/// The crossbow enchantments, added with the crossbow in 1.14.
static ENCHANTMENTS_1_14: &[&str] = &["multishot", "piercing", "quick_charge"];

/// Soul speed, in `md('1.16.5')` and not in `md('1.16.1')`.
static ENCHANTMENTS_1_16: &[&str] = &["soul_speed"];

/// Swift sneak, in `md('1.19')` and not in `md('1.18.2')`.
static ENCHANTMENTS_1_19: &[&str] = &["swift_sneak"];

/// `minecraft-data` reports `sweeping` up to 1.20.4 and `sweeping_edge` from
/// 1.20.6.
const NATIVE_SWEEPING: &str = "sweeping_edge";
const LEGACY_SWEEPING: &str = "sweeping";

/// The potions added in 1.21; a bottle holding one loses its `Potion` tag
/// rather than taking a substitute.
static POTIONS_1_21: &[&str] = &["infested", "oozing", "weaving", "wind_charged"];

fn enchantment_exists_on(name: &str, version: V) -> bool {
    ENCHANTMENTS_1_13.contains(&name)
        || (version >= V::V_1_14 && ENCHANTMENTS_1_14.contains(&name))
        || (version >= V::V_1_16 && ENCHANTMENTS_1_16.contains(&name))
        || (version >= V::V_1_19 && ENCHANTMENTS_1_19.contains(&name))
}

fn legacy_enchantment_name(enchantment: &Enchantment, version: V) -> Option<&'static str> {
    let key = if enchantment.registry_key == NATIVE_SWEEPING {
        LEGACY_SWEEPING
    } else {
        enchantment.registry_key
    };
    enchantment_exists_on(key, version).then_some(key)
}

fn native_enchantment(id: &str) -> Option<&'static Enchantment> {
    let bare = id.strip_prefix("minecraft:").unwrap_or(id);
    if bare == LEGACY_SWEEPING {
        return Enchantment::from_name(NATIVE_SWEEPING);
    }
    Enchantment::from_name(bare)
}

/// The category prefixed attribute names in the 1.20.4 server jar, which are
/// what `md('1.20.1').attributesArray[].resource` holds.
static LEGACY_ATTRIBUTE_NAMES: &[(&str, &str)] = &[
    ("minecraft:armor", "generic.armor"),
    ("minecraft:armor_toughness", "generic.armor_toughness"),
    ("minecraft:attack_damage", "generic.attack_damage"),
    ("minecraft:attack_knockback", "generic.attack_knockback"),
    ("minecraft:attack_speed", "generic.attack_speed"),
    ("minecraft:flying_speed", "generic.flying_speed"),
    ("minecraft:follow_range", "generic.follow_range"),
    (
        "minecraft:knockback_resistance",
        "generic.knockback_resistance",
    ),
    ("minecraft:luck", "generic.luck"),
    ("minecraft:max_health", "generic.max_health"),
    ("minecraft:movement_speed", "generic.movement_speed"),
    ("minecraft:jump_strength", "horse.jump_strength"),
    (
        "minecraft:spawn_reinforcements",
        "zombie.spawn_reinforcements",
    ),
];

/// The one attribute here that is not there for the whole range: it arrives
/// in 1.20.2.
const LEGACY_MAX_ABSORPTION: (&str, &str) = ("minecraft:max_absorption", "generic.max_absorption");

fn legacy_attribute_name(attribute: &Attributes, version: V) -> Option<&'static str> {
    if attribute.name == LEGACY_MAX_ABSORPTION.0 {
        return (version >= V::V_1_20_2).then_some(LEGACY_MAX_ABSORPTION.1);
    }
    LEGACY_ATTRIBUTE_NAMES
        .iter()
        .find(|(native, _)| *native == attribute.name)
        .map(|(_, legacy)| *legacy)
}

/// Clients before 1.20.5 only have the six equipment slots; a modifier on a
/// group slot has no spelling there.
enum LegacySlot {
    Everywhere,
    Named(&'static str),
}

const fn legacy_slot(slot: &AttributeModifierSlot) -> Option<LegacySlot> {
    match slot {
        AttributeModifierSlot::Any => Some(LegacySlot::Everywhere),
        AttributeModifierSlot::MainHand => Some(LegacySlot::Named("mainhand")),
        AttributeModifierSlot::OffHand => Some(LegacySlot::Named("offhand")),
        AttributeModifierSlot::Feet => Some(LegacySlot::Named("feet")),
        AttributeModifierSlot::Legs => Some(LegacySlot::Named("legs")),
        AttributeModifierSlot::Chest => Some(LegacySlot::Named("chest")),
        AttributeModifierSlot::Head => Some(LegacySlot::Named("head")),
        AttributeModifierSlot::Hand
        | AttributeModifierSlot::Armor
        | AttributeModifierSlot::Body
        | AttributeModifierSlot::Saddle => None,
    }
}

const fn legacy_operation(operation: Operation) -> i32 {
    match operation {
        Operation::AddValue => 0,
        Operation::AddMultipliedBase => 1,
        Operation::AddMultipliedTotal => 2,
    }
}

fn downcast<T: DataComponentImpl + 'static>(value: &dyn DataComponentImpl) -> Option<&T> {
    value.as_any().downcast_ref::<T>()
}

/// Reads a tag clients write as an int but may narrow to a byte or short.
fn extract_int_like(tag: &NbtTag) -> Option<i32> {
    match tag {
        NbtTag::Byte(v) => Some(i32::from(*v)),
        NbtTag::Short(v) => Some(i32::from(*v)),
        NbtTag::Int(v) => Some(*v),
        NbtTag::Long(v) => i32::try_from(*v).ok(),
        _ => None,
    }
}

/// Text that is not valid component JSON is kept as plain text, which is what
/// the client itself shows for it.
fn text_from_json(raw: &str) -> TextComponent {
    serde_json::from_str(raw).unwrap_or_else(|_| TextComponent::text(raw.to_owned()))
}

fn text_to_json(text: &TextComponent, version: V, ids: &ComposedMappings) -> String {
    sanitize_text(text, ids).to_json_for_version(&version)
}

/// `display.Lore` holds JSON text components from 1.14; on 1.13 the entries
/// are plain strings.
fn lore_line(text: &TextComponent, version: V, ids: &ComposedMappings) -> String {
    if version >= V::V_1_14 {
        text_to_json(text, version, ids)
    } else {
        text.clone().get_text()
    }
}

fn lore_from_wire(raw: &str, version: V) -> TextComponent {
    if version >= V::V_1_14 {
        text_from_json(raw)
    } else {
        TextComponent::text(raw.to_owned())
    }
}

/// Drops hover events naming an item or entity the target has no id for; the
/// client resolves those names while decoding and one unknown name fails the
/// packet.
fn sanitize_text(text: &TextComponent, ids: &ComposedMappings) -> TextComponent {
    let mut out = text.clone();
    sanitize_base(&mut out.0, ids);
    out
}

fn hover_exists_on(hover: &pumpkin_util::text::hover::HoverEvent, ids: &ComposedMappings) -> bool {
    use pumpkin_util::text::hover::HoverEvent;
    match hover {
        HoverEvent::ShowText { .. } => true,
        HoverEvent::ShowItem { id, .. } => pumpkin_data::item::Item::from_registry_key(id)
            .is_some_and(|item| ids.items.map(u32::from(item.id)).is_some()),
        HoverEvent::ShowEntity { id, .. } => pumpkin_data::entity::EntityType::from_name(id)
            .is_some_and(|entity| ids.entities.map(u32::from(entity.id)).is_some()),
    }
}

fn sanitize_base(base: &mut pumpkin_util::text::TextComponentBase, ids: &ComposedMappings) {
    use pumpkin_util::text::hover::HoverEvent;
    if base
        .style
        .hover_event
        .as_ref()
        .is_some_and(|hover| !hover_exists_on(hover, ids))
    {
        base.style.hover_event = None;
    }
    match base.style.hover_event.as_mut() {
        Some(HoverEvent::ShowText { value }) => {
            for child in value {
                sanitize_base(child, ids);
            }
        }
        Some(HoverEvent::ShowEntity {
            name: Some(name), ..
        }) => {
            for child in name {
                sanitize_base(child, ids);
            }
        }
        _ => {}
    }
    if let pumpkin_util::text::TextContent::Translate { with, .. } = base.content.as_mut() {
        for arg in with {
            sanitize_base(arg, ids);
        }
    }
    for child in &mut base.extra {
        sanitize_base(child, ids);
    }
}

fn legacy_enchantment_list(list: &[(&'static Enchantment, i32)], version: V) -> Vec<NbtTag> {
    let mut tags = Vec::with_capacity(list.len());
    for (enchantment, level) in list {
        let Some(name) = legacy_enchantment_name(enchantment, version) else {
            continue;
        };
        let mut entry = NbtCompound::new();
        entry.put_string("id", format!("minecraft:{name}"));
        entry.put_short("lvl", (*level).clamp(0, i32::from(i16::MAX)) as i16);
        tags.push(NbtTag::Compound(entry));
    }
    tags
}

fn native_enchantment_list(tags: &[NbtTag]) -> Vec<(&'static Enchantment, i32)> {
    let mut list = Vec::with_capacity(tags.len());
    for tag in tags {
        let Some(entry) = tag.extract_compound() else {
            continue;
        };
        let Some(enchantment) = entry.get_string("id").and_then(native_enchantment) else {
            continue;
        };
        let level = entry.get("lvl").and_then(extract_int_like).unwrap_or(1);
        list.push((enchantment, level));
    }
    list
}

/// The single int a legacy `CustomModelData` tag holds, if the 26.3 component
/// holds exactly that and nothing else.
fn legacy_custom_model_data(data: &CustomModelDataImpl) -> Option<i32> {
    if data.floats.len() != 1
        || !data.flags.is_empty()
        || !data.strings.is_empty()
        || !data.colors.is_empty()
    {
        return None;
    }
    let value = data.floats[0];
    let rounded = value as i32;
    (value == rounded as f32).then_some(rounded)
}

fn legacy_potion_name(potion_id: i32, version: V) -> Option<String> {
    let id = u8::try_from(potion_id).ok()?;
    let potion = Potion::from_id(id)?;
    let known_here = version >= V::V_1_21 || !POTIONS_1_21.contains(&potion.name);
    known_here.then(|| format!("minecraft:{}", potion.name))
}

fn legacy_attribute_entry(modifier: &Modifier, index: i32, version: V) -> Option<NbtCompound> {
    let name = legacy_attribute_name(modifier.r#type, version)?;
    let slot = legacy_slot(&modifier.slot)?;
    let mut entry = NbtCompound::new();
    entry.put_string("AttributeName", name.to_owned());
    entry.put_string("Name", modifier.id.to_owned());
    entry.put_double("Amount", modifier.amount);
    entry.put_int("Operation", legacy_operation(modifier.operation));
    let (high, low) = legacy_modifier_uuid(modifier.id, index).as_u64_pair();
    if version >= V::V_1_16 {
        // 1.16 replaced the two longs with an int array, vanilla's `ItemStackUUIDFix`.
        entry.put(
            "UUID",
            NbtTag::IntArray(vec![
                (high >> 32) as i32,
                (high & 0xffff_ffff) as i32,
                (low >> 32) as i32,
                (low & 0xffff_ffff) as i32,
            ]),
        );
    } else {
        entry.put_long("UUIDMost", high as i64);
        entry.put_long("UUIDLeast", low as i64);
    }
    if let LegacySlot::Named(slot) = slot {
        entry.put_string("Slot", slot.to_owned());
    }
    Some(entry)
}

fn legacy_skull_owner(profile: &ProfileImpl, version: V) -> Option<NbtCompound> {
    let mut owner = NbtCompound::new();
    if let Some(name) = &profile.name {
        owner.put_string("Name", name.clone());
    }
    if let Some(id) = &profile.id {
        if version >= V::V_1_16 {
            owner.put("Id", NbtTag::IntArray(id.to_vec()));
        } else {
            let high = (u64::from(id[0] as u32) << 32) | u64::from(id[1] as u32);
            let low = (u64::from(id[2] as u32) << 32) | u64::from(id[3] as u32);
            owner.put_string("Id", uuid::Uuid::from_u64_pair(high, low).to_string());
        }
    }
    let textures: Vec<NbtTag> = profile
        .properties
        .iter()
        .filter(|property| property.name == "textures")
        .map(|property| {
            let mut texture = NbtCompound::new();
            texture.put_string("Value", property.value.clone());
            if let Some(signature) = &property.signature {
                texture.put_string("Signature", signature.clone());
            }
            NbtTag::Compound(texture)
        })
        .collect();
    if !textures.is_empty() {
        let mut properties = NbtCompound::new();
        properties.put_list("textures", textures);
        owner.put_compound("Properties", properties);
    }
    (!owner.is_empty()).then_some(owner)
}

fn custom_data_base(patch: &ComponentPatch) -> Option<NbtCompound> {
    patch
        .iter()
        .find(|(id, _)| *id == DataComponent::CustomData)
        .and_then(|(_, data)| data.as_ref())
        .and_then(|data| downcast::<CustomDataImpl>(data.as_ref()))
        .map(|value| value.data.clone())
}

/// The 26.3 components of a stack, decoded from the bytes core wrote.
fn decode(added: &[ItemComponent]) -> ComponentPatch {
    let mut patch = ComponentPatch::new();
    for component in added {
        let Some(id) = u8::try_from(component.id)
            .ok()
            .and_then(DataComponent::try_from_id)
        else {
            continue;
        };
        let mut cursor = component.data.as_slice();
        if let Ok(value) = data_component::deserialize(id, &mut cursor) {
            patch.push((id, Some(value)));
        }
    }
    patch
}

/// The NBT a client below 1.20.5 should receive for a stack's components, or
/// `None` when none of them can be expressed there.
#[must_use]
pub fn components_to_nbt(
    added: &[ItemComponent],
    version: V,
    ids: &ComposedMappings,
) -> Option<NbtCompound> {
    patch_to_nbt(&decode(added), version, ids)
}

#[allow(clippy::too_many_lines)]
fn patch_to_nbt(patch: &ComponentPatch, version: V, ids: &ComposedMappings) -> Option<NbtCompound> {
    // `custom_data` holds the tags with no component of their own, so it is
    // the base every real component writes over.
    let mut root = custom_data_base(patch).unwrap_or_default();
    let mut display = NbtCompound::new();

    for (id, data) in patch {
        let Some(data) = data.as_ref().map(AsRef::as_ref) else {
            continue;
        };
        match id {
            DataComponent::Damage => {
                if let Some(value) = downcast::<DamageImpl>(data) {
                    root.put_int("Damage", value.damage);
                }
            }
            DataComponent::RepairCost => {
                if let Some(value) = downcast::<RepairCostImpl>(data) {
                    root.put_int("RepairCost", value.cost);
                }
            }
            DataComponent::Unbreakable => root.put_bool("Unbreakable", true),
            DataComponent::CustomName => {
                if let Some(value) = downcast::<CustomNameImpl>(data) {
                    display.put_string("Name", text_to_json(&value.name, version, ids));
                }
            }
            DataComponent::Lore => {
                if let Some(value) = downcast::<LoreImpl>(data) {
                    let lines: Vec<NbtTag> = value
                        .lines
                        .iter()
                        .map(|line| NbtTag::String(lore_line(line, version, ids).into()))
                        .collect();
                    if !lines.is_empty() {
                        display.put_list("Lore", lines);
                    }
                }
            }
            DataComponent::DyedColor => {
                if let Some(value) = downcast::<DyedColorImpl>(data) {
                    display.put_int("color", value.rgb);
                }
            }
            DataComponent::Enchantments => {
                if let Some(value) = downcast::<EnchantmentsImpl>(data) {
                    let list = legacy_enchantment_list(&value.enchantment, version);
                    if !list.is_empty() {
                        root.put_list("Enchantments", list);
                    }
                }
            }
            DataComponent::StoredEnchantments => {
                if let Some(value) = downcast::<StoredEnchantmentsImpl>(data) {
                    let list = legacy_enchantment_list(&value.enchantment, version);
                    if !list.is_empty() {
                        root.put_list("StoredEnchantments", list);
                    }
                }
            }
            DataComponent::CustomModelData => {
                if let Some(value) =
                    downcast::<CustomModelDataImpl>(data).and_then(legacy_custom_model_data)
                {
                    root.put_int("CustomModelData", value);
                }
            }
            DataComponent::MapId => {
                if let Some(value) = downcast::<MapIdImpl>(data) {
                    root.put_int("map", value.id);
                }
            }
            DataComponent::PotionContents => {
                if let Some(value) = downcast::<PotionContentsImpl>(data) {
                    if let Some(name) = value
                        .potion_id
                        .and_then(|id| legacy_potion_name(id, version))
                    {
                        root.put_string("Potion", name);
                    }
                    if let Some(color) = value.custom_color {
                        root.put_int("CustomPotionColor", color);
                    }
                }
            }
            DataComponent::Profile => {
                if let Some(owner) = downcast::<ProfileImpl>(data)
                    .and_then(|value| legacy_skull_owner(value, version))
                {
                    root.put_compound("SkullOwner", owner);
                }
            }
            DataComponent::BlockEntityData => {
                if let Some(value) = downcast::<BlockEntityDataImpl>(data) {
                    root.put_compound("BlockEntityTag", value.nbt.clone());
                }
            }
            DataComponent::Trim => {
                // Armour trims arrive in 1.20; below that the tag means nothing.
                if version >= V::V_1_20
                    && let Some(value) = downcast::<TrimImpl>(data)
                {
                    let mut trim = NbtCompound::new();
                    trim.put("material", value.material.clone());
                    trim.put("pattern", value.pattern.clone());
                    root.put_compound("Trim", trim);
                }
            }
            DataComponent::WrittenBookContent => {
                if let Some(value) = downcast::<WrittenBookContentImpl>(data) {
                    root.put_string("title", value.title.clone());
                    root.put_string("author", value.author.clone());
                    root.put_list(
                        "pages",
                        value
                            .pages
                            .iter()
                            .map(|page| NbtTag::String(page.clone().into()))
                            .collect(),
                    );
                    root.put_bool("resolved", true);
                }
            }
            DataComponent::WritableBookContent => {
                if let Some(value) = downcast::<WritableBookContentImpl>(data) {
                    root.put_list(
                        "pages",
                        value
                            .pages
                            .iter()
                            .map(|page| NbtTag::String(page.clone().into()))
                            .collect(),
                    );
                }
            }
            DataComponent::AttributeModifiers => {
                if let Some(value) = downcast::<AttributeModifiersImpl>(data) {
                    let entries: Vec<NbtTag> = value
                        .attribute_modifiers
                        .iter()
                        .enumerate()
                        .filter_map(|(index, modifier)| {
                            legacy_attribute_entry(
                                modifier,
                                i32::try_from(index).unwrap_or(0),
                                version,
                            )
                            .map(NbtTag::Compound)
                        })
                        .collect();
                    if !entries.is_empty() {
                        root.put_list("AttributeModifiers", entries);
                    }
                }
            }
            _ => {}
        }
    }

    if !display.is_empty() {
        root.put_compound("display", display);
    }
    if root.is_empty() { None } else { Some(root) }
}

/// The root tags this module turns into components; everything else a client
/// sends is kept in `minecraft:custom_data`.
static CONSUMED_ROOT_TAGS: &[&str] = &[
    "BlockEntityTag",
    "CustomModelData",
    "CustomPotionColor",
    "Damage",
    "Enchantments",
    "Potion",
    "RepairCost",
    "SkullOwner",
    "StoredEnchantments",
    "Trim",
    "Unbreakable",
    "author",
    "display",
    "map",
    "pages",
    "resolved",
    "title",
];

fn profile_from_skull_owner(owner: &NbtTag) -> Option<ProfileImpl> {
    let mut profile = ProfileImpl::default();
    match owner {
        NbtTag::String(name) => profile.name = Some(name.to_string()),
        NbtTag::Compound(compound) => {
            profile.name = compound.get_string("Name").map(ToOwned::to_owned);
            profile.id = compound
                .get_int_array("Id")
                .and_then(|id| <[i32; 4]>::try_from(id).ok());
            if let Some(textures) = compound
                .get_compound("Properties")
                .and_then(|properties| properties.get_list("textures"))
            {
                for texture in textures {
                    let Some(texture) = texture.extract_compound() else {
                        continue;
                    };
                    let Some(value) = texture.get_string("Value") else {
                        continue;
                    };
                    profile.properties.push(ProfileProperty {
                        name: "textures".to_owned(),
                        value: value.to_owned(),
                        signature: texture.get_string("Signature").map(ToOwned::to_owned),
                    });
                }
            }
        }
        _ => return None,
    }
    (profile.name.is_some() || profile.id.is_some() || !profile.properties.is_empty())
        .then_some(profile)
}

/// The 26.3 components a client's item NBT stands for.
#[must_use]
pub fn nbt_to_components(nbt: &NbtCompound, version: V) -> Vec<ItemComponent> {
    let mut out = Vec::new();
    for (id, value) in nbt_to_patch(nbt, version) {
        let Some(value) = value else {
            continue;
        };
        let mut data = Vec::new();
        if data_component::serialize(id, value.as_ref(), &mut data).is_ok() {
            out.push(ItemComponent {
                id: i32::from(id.to_id()),
                data,
            });
        }
    }
    out
}

#[allow(clippy::too_many_lines)]
fn nbt_to_patch(nbt: &NbtCompound, version: V) -> ComponentPatch {
    let mut patch = ComponentPatch::new();

    if let Some(damage) = nbt.get("Damage").and_then(extract_int_like) {
        patch.push((DataComponent::Damage, Some(DamageImpl { damage }.to_dyn())));
    }
    if let Some(cost) = nbt.get("RepairCost").and_then(extract_int_like) {
        patch.push((
            DataComponent::RepairCost,
            Some(RepairCostImpl { cost }.to_dyn()),
        ));
    }
    if nbt
        .get("Unbreakable")
        .and_then(extract_int_like)
        .is_some_and(|value| value != 0)
    {
        patch.push((DataComponent::Unbreakable, Some(UnbreakableImpl.to_dyn())));
    }
    if let Some(value) = nbt.get("CustomModelData").and_then(extract_int_like) {
        patch.push((
            DataComponent::CustomModelData,
            Some(
                CustomModelDataImpl {
                    floats: vec![value as f32],
                    flags: Vec::new(),
                    strings: Vec::new(),
                    colors: Vec::new(),
                }
                .to_dyn(),
            ),
        ));
    }
    if let Some(value) = nbt.get("map").and_then(extract_int_like) {
        patch.push((DataComponent::MapId, Some(MapIdImpl { id: value }.to_dyn())));
    }
    if let Some(list) = nbt.get_list("Enchantments") {
        let enchantment = native_enchantment_list(list);
        if !enchantment.is_empty() {
            patch.push((
                DataComponent::Enchantments,
                Some(
                    EnchantmentsImpl {
                        enchantment: Cow::Owned(enchantment),
                    }
                    .to_dyn(),
                ),
            ));
        }
    }
    if let Some(list) = nbt.get_list("StoredEnchantments") {
        let enchantment = native_enchantment_list(list);
        if !enchantment.is_empty() {
            patch.push((
                DataComponent::StoredEnchantments,
                Some(
                    StoredEnchantmentsImpl {
                        enchantment: Cow::Owned(enchantment),
                    }
                    .to_dyn(),
                ),
            ));
        }
    }
    let potion = nbt
        .get_string("Potion")
        .map(|name| name.strip_prefix("minecraft:").unwrap_or(name))
        .and_then(Potion::from_name)
        .map(|potion| i32::from(potion.id));
    let potion_color = nbt.get("CustomPotionColor").and_then(extract_int_like);
    if potion.is_some() || potion_color.is_some() {
        patch.push((
            DataComponent::PotionContents,
            Some(
                PotionContentsImpl {
                    potion_id: potion,
                    custom_color: potion_color,
                    custom_effects: Vec::new(),
                    custom_name: None,
                }
                .to_dyn(),
            ),
        ));
    }
    if let Some(profile) = nbt.get("SkullOwner").and_then(profile_from_skull_owner) {
        patch.push((DataComponent::Profile, Some(profile.to_dyn())));
    }
    if let Some(block_entity) = nbt.get_compound("BlockEntityTag") {
        patch.push((
            DataComponent::BlockEntityData,
            Some(
                BlockEntityDataImpl {
                    nbt: block_entity.clone(),
                }
                .to_dyn(),
            ),
        ));
    }
    if let Some(trim) = nbt.get_compound("Trim")
        && let (Some(material), Some(pattern)) = (trim.get("material"), trim.get("pattern"))
    {
        patch.push((
            DataComponent::Trim,
            Some(
                TrimImpl {
                    material: material.clone(),
                    pattern: pattern.clone(),
                }
                .to_dyn(),
            ),
        ));
    }
    let pages: Vec<String> = nbt
        .get_list("pages")
        .map(|pages| {
            pages
                .iter()
                .filter_map(NbtTag::extract_string)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if !pages.is_empty() {
        match (nbt.get_string("title"), nbt.get_string("author")) {
            (Some(title), Some(author)) => patch.push((
                DataComponent::WrittenBookContent,
                Some(
                    WrittenBookContentImpl {
                        title: title.to_owned(),
                        author: author.to_owned(),
                        pages,
                    }
                    .to_dyn(),
                ),
            )),
            _ => patch.push((
                DataComponent::WritableBookContent,
                Some(WritableBookContentImpl { pages }.to_dyn()),
            )),
        }
    }
    if let Some(display) = nbt.get_compound("display") {
        if let Some(name) = display.get_string("Name") {
            patch.push((
                DataComponent::CustomName,
                Some(
                    CustomNameImpl {
                        name: text_from_json(name),
                    }
                    .to_dyn(),
                ),
            ));
        }
        if let Some(lore) = display.get_list("Lore") {
            let lines: Vec<TextComponent> = lore
                .iter()
                .filter_map(NbtTag::extract_string)
                .map(|line| lore_from_wire(line, version))
                .collect();
            if !lines.is_empty() {
                patch.push((DataComponent::Lore, Some(LoreImpl { lines }.to_dyn())));
            }
        }
        if let Some(color) = display.get("color").and_then(extract_int_like) {
            patch.push((
                DataComponent::DyedColor,
                Some(DyedColorImpl { rgb: color }.to_dyn()),
            ));
        }
    }

    // Everything else the client sent, so the stack is the same stack when it
    // comes back; `AttributeModifiers` is here because a 26.3 modifier is
    // identified by a `'static` resource id a string off the wire cannot become.
    let mut custom = NbtCompound::new();
    for (name, tag) in &nbt.child_tags {
        if !CONSUMED_ROOT_TAGS.contains(&name.as_ref()) {
            custom.put(name, tag.clone());
        }
    }
    if !custom.is_empty() {
        patch.push((
            DataComponent::CustomData,
            Some(CustomDataImpl::new(custom).to_dyn()),
        ));
    }

    patch
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweeping_is_renamed_in_both_directions() {
        let sweeping = Enchantment::from_name(NATIVE_SWEEPING).unwrap();
        assert_eq!(
            legacy_enchantment_name(sweeping, V::V_1_20),
            Some(LEGACY_SWEEPING)
        );
        assert_eq!(
            native_enchantment("minecraft:sweeping").map(|e| e.registry_key),
            Some(NATIVE_SWEEPING)
        );
    }

    /// `breach` arrives in 1.21, so no client in the NBT range has the name.
    #[test]
    fn enchantments_the_client_lacks_are_dropped() {
        let breach = Enchantment::from_name("breach").unwrap();
        assert_eq!(legacy_enchantment_name(breach, V::V_1_20), None);
        let sharpness = Enchantment::from_name("sharpness").unwrap();
        assert_eq!(
            legacy_enchantment_name(sharpness, V::V_1_16_2),
            Some("sharpness")
        );
    }

    #[test]
    fn swift_sneak_starts_at_1_19() {
        let swift_sneak = Enchantment::from_name("swift_sneak").unwrap();
        assert_eq!(legacy_enchantment_name(swift_sneak, V::V_1_18_2), None);
        assert_eq!(
            legacy_enchantment_name(swift_sneak, V::V_1_19),
            Some("swift_sneak")
        );
    }

    #[test]
    fn attribute_names_are_the_version_s_own() {
        assert_eq!(
            legacy_attribute_name(&Attributes::MAX_HEALTH, V::V_1_16_2),
            Some("generic.max_health")
        );
        assert_eq!(
            legacy_attribute_name(&Attributes::JUMP_STRENGTH, V::V_1_20),
            Some("horse.jump_strength")
        );
        assert_eq!(
            legacy_attribute_name(&Attributes::MAX_ABSORPTION, V::V_1_20),
            None
        );
        assert_eq!(
            legacy_attribute_name(&Attributes::MAX_ABSORPTION, V::V_1_20_2),
            Some("generic.max_absorption")
        );
    }

    #[test]
    fn potions_added_in_1_21_have_no_name_here() {
        let oozing = Potion::from_name("oozing").unwrap();
        assert_eq!(legacy_potion_name(i32::from(oozing.id), V::V_1_20), None);
        let healing = Potion::from_name("healing").unwrap();
        assert_eq!(
            legacy_potion_name(i32::from(healing.id), V::V_1_20),
            Some("minecraft:healing".to_owned())
        );
    }

    #[test]
    fn unknown_tags_round_trip_through_custom_data() {
        let ids = crate::api::MappingData::get().composed(V::V_1_16_2);
        let mut nbt = NbtCompound::new();
        nbt.put_int("HideFlags", 63);
        nbt.put_int("Damage", 3);

        let patch = nbt_to_patch(&nbt, V::V_1_16_2);
        let back = patch_to_nbt(&patch, V::V_1_16_2, ids).unwrap();
        assert_eq!(back.get_int("HideFlags"), Some(63));
        assert_eq!(back.get_int("Damage"), Some(3));
    }

    #[test]
    fn a_group_slot_modifier_is_left_out() {
        let modifier = Modifier {
            r#type: &Attributes::ATTACK_DAMAGE,
            id: "minecraft:base_attack_damage",
            amount: 3.0,
            operation: Operation::AddValue,
            slot: AttributeModifierSlot::Armor,
        };
        assert!(legacy_attribute_entry(&modifier, 0, V::V_1_20).is_none());
    }

    /// 1.16 replaced the two UUID longs with an int array.
    #[test]
    fn an_attribute_entry_has_the_1_16_uuid_array() {
        let modifier = Modifier {
            r#type: &Attributes::ATTACK_DAMAGE,
            id: "minecraft:base_attack_damage",
            amount: 3.0,
            operation: Operation::AddMultipliedBase,
            slot: AttributeModifierSlot::MainHand,
        };
        let entry = legacy_attribute_entry(&modifier, 0, V::V_1_16_2).unwrap();
        assert_eq!(
            entry.get_string("AttributeName"),
            Some("generic.attack_damage")
        );
        assert_eq!(entry.get_string("Slot"), Some("mainhand"));
        assert_eq!(entry.get_int("Operation"), Some(1));
        assert_eq!(entry.get_int_array("UUID").map(<[i32]>::len), Some(4));
    }
}
