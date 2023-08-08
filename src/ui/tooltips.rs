use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    attributes::{
        Attack, BonusDamage, CritChance, CritDamage, Defence, Dodge, Healing, HealthRegen,
        ItemAttributes, Lifesteal, LootRateBonus, MaxHealth, Speed, Thorns, XpRateBonus,
    },
    colors::{BLACK, GOLD, LIGHT_GREEN, LIGHT_GREY},
    player::Player,
};

use super::{
    InventorySlotState, InventoryUI, InventoryUIState, ShowInvPlayerStatsEvent, ToolTipUpdateEvent,
    UIElement, CHEST_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE,
};
#[derive(Component)]
pub struct PlayerStatsTooltip;

pub fn handle_spawn_inv_item_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut updates: EventReader<ToolTipUpdateEvent>,
    inv: Query<Entity, With<InventoryUI>>,
    cur_inv_state: Res<State<InventoryUIState>>,
    old_tooltips: Query<
        (Entity, &UIElement, &Parent),
        (Without<InventorySlotState>, Without<PlayerStatsTooltip>),
    >,
) {
    for item in updates.iter() {
        for tooltip in old_tooltips.iter() {
            commands.entity(tooltip.0).despawn_recursive();
        }
        let inv = inv.single();
        let parent_inv_size = match cur_inv_state.0 {
            InventoryUIState::Open => INVENTORY_UI_SIZE,
            InventoryUIState::Chest => CHEST_INVENTORY_UI_SIZE,
            _ => unreachable!(),
        };
        let attributes = item.item_stack.attributes.get_tooltips();
        let durability = item.item_stack.attributes.get_durability_tooltip();
        let has_attributes = attributes.len() > 0;
        let size = Vec2::new(93., 120.5);
        let tooltip = commands
            .spawn((
                SpriteBundle {
                    texture: graphics
                        .ui_image_handles
                        .as_ref()
                        .unwrap()
                        .get(&item.item_stack.rarity.get_tooltip_ui_element())
                        .unwrap()
                        .clone(),
                    transform: Transform {
                        translation: Vec3::new(-(parent_inv_size.x + size.x + 2.) / 2., 0., 4.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(size),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                item.item_stack.rarity.get_tooltip_ui_element(),
                Name::new("TOOLTIP"),
            ))
            .id();

        let mut tooltip_text: Vec<(String, f32)> = vec![];
        tooltip_text.push((item.item_stack.metadata.name.clone(), 0.));
        // tooltip_text.push(item.item_stack.metadata.desc.clone());
        for (_i, a) in attributes.iter().enumerate().clone() {
            tooltip_text.push((a.to_string(), 0.));
        }
        if has_attributes {
            tooltip_text.push((
                durability.clone(),
                size.y - (tooltip_text.len() + 1) as f32 * 10. - 14.,
            ));
        }

        for (i, (text, d)) in tooltip_text.iter().enumerate() {
            let text_pos = if i == 0 {
                Vec3::new(
                    -(f32::ceil((text.chars().count() * 6 - 1) as f32 / 2.)) + 0.5,
                    size.y / 2. - 12.,
                    1.,
                )
            } else {
                Vec3::new(
                    -size.x / 2. + 8.,
                    size.y / 2. - 12. - (i as f32 * 10.) - d - 2.,
                    1.,
                )
            };
            let text = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            text,
                            TextStyle {
                                font: asset_server.load("fonts/Kitchen Sink.ttf"),
                                font_size: 8.0,
                                color: if i == 0 {
                                    item.item_stack.rarity.get_color()
                                } else if i > 1 && i == tooltip_text.len() - 1 {
                                    LIGHT_GREY
                                } else if i > 2 {
                                    GOLD
                                } else {
                                    LIGHT_GREY
                                },
                            },
                        ),
                        text_anchor: Anchor::CenterLeft,
                        transform: Transform {
                            translation: text_pos,
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    Name::new("TOOLTIP TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .id();
            commands.entity(tooltip).add_child(text);
        }
        commands.entity(inv).add_child(tooltip);
    }
}

pub fn handle_spawn_inv_player_stats(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut updates: EventReader<ShowInvPlayerStatsEvent>,
    player_stats: Query<
        (
            &Attack,
            &MaxHealth,
            &Defence,
            &CritChance,
            &CritDamage,
            &BonusDamage,
            &HealthRegen,
            &Healing,
            &Thorns,
            &Dodge,
            &Speed,
            &Lifesteal,
            &XpRateBonus,
            &LootRateBonus,
        ),
        With<Player>,
    >,
    inv: Query<Entity, With<InventoryUI>>,
    old_tooltips: Query<
        (Entity, &UIElement, &Parent),
        (Without<InventorySlotState>, With<PlayerStatsTooltip>),
    >,
) {
    if updates.iter().len() > 0 {
        for tooltip in old_tooltips.iter() {
            commands.entity(tooltip.0).despawn_recursive();
        }
        let inv = inv.single();
        let (
            attack,
            max_health,
            defence,
            crit_chance,
            crit_damage,
            bonus_damage,
            health_regen,
            healing,
            thorns,
            dodge,
            speed,
            lifesteal,
            xp_rate_bonus,
            loot_rate_bonus,
        ) = player_stats.single();
        let attributes = ItemAttributes {
            attack: attack.0,
            health: max_health.0,
            defence: defence.0,
            crit_chance: crit_chance.0,
            crit_damage: crit_damage.0,
            bonus_damage: bonus_damage.0,
            health_regen: health_regen.0,
            healing: healing.0,
            thorns: thorns.0,
            dodge: dodge.0,
            speed: speed.0,
            lifesteal: lifesteal.0,
            xp_rate: xp_rate_bonus.0,
            loot_rate: loot_rate_bonus.0,
            ..default()
        }
        .get_stats_summary();

        let size = Vec2::new(93., 120.5);
        let tooltip = commands
            .spawn((
                SpriteBundle {
                    texture: graphics
                        .ui_image_handles
                        .as_ref()
                        .unwrap()
                        .get(&UIElement::LargeTooltipCommon)
                        .unwrap()
                        .clone(),
                    transform: Transform {
                        translation: Vec3::new(-(INVENTORY_UI_SIZE.x + size.x + 2.) / 2., 0., 2.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(size),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                UIElement::LargeTooltipCommon,
                PlayerStatsTooltip,
                Name::new("TOOLTIP"),
            ))
            .id();

        let mut tooltip_text: Vec<((String, String), f32)> = vec![];
        tooltip_text.push((("Stats".to_string(), "".to_string()), 0.));
        for (_i, a) in attributes.iter().enumerate().clone() {
            tooltip_text.push(((a.0.clone(), a.1.clone()), 0.));
        }

        for (i, (text, d)) in tooltip_text.iter().enumerate() {
            let text_pos = if i == 0 {
                Vec3::new(
                    -(f32::ceil((text.0.chars().count() * 6 - 1) as f32 / 2.)) + 0.5,
                    size.y / 2. - 12.,
                    1.,
                )
            } else {
                Vec3::new(
                    -size.x / 2. + 8.,
                    size.y / 2. - 12. - (i as f32 * 8.) - d - 2.,
                    1.,
                )
            };
            let text = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_sections(vec![
                            TextSection {
                                value: text.0.to_string(),
                                style: TextStyle {
                                    font: asset_server.load("fonts/Kitchen Sink.ttf"),
                                    font_size: 8.0,
                                    color: if i == 0 { BLACK } else { LIGHT_GREY },
                                },
                            },
                            TextSection {
                                value: text.1.to_string(),
                                style: TextStyle {
                                    font: asset_server.load("fonts/Kitchen Sink.ttf"),
                                    font_size: 8.0,
                                    color: LIGHT_GREEN,
                                },
                            },
                        ]),
                        text_anchor: Anchor::CenterLeft,
                        transform: Transform {
                            translation: text_pos,
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    Name::new("TOOLTIP TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .id();
            commands.entity(tooltip).add_child(text);
        }
        commands.entity(inv).add_child(tooltip);
    }
}
