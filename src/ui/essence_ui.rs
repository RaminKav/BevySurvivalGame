use std::{fs::File, io::BufReader};

use bevy::{prelude::*, render::view::RenderLayers};
use bevy_proto::backend::schematics::{ReflectSchematic, Schematic};
use rand::seq::IteratorRandom;

use crate::{
    assets::Graphics,
    attributes::attribute_helpers::create_new_random_item_stack_with_attributes,
    client::GameData,
    datafiles,
    inventory::{Inventory, ItemStack},
    item::WorldObject,
    player::Player,
    proto::proto_param::ProtoParam,
    GameParam, ScreenResolution, GAME_HEIGHT,
};

use super::{spawn_item_stack_icon, Interactable, UIElement, UIState, ESSENCE_UI_SIZE};

#[derive(Component)]
pub struct EssenceUI;

#[derive(Component, Clone, Debug, Resource, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct EssenceOption {
    pub item: ItemStack,
    pub cost: u32,
}

impl EssenceOption {
    fn get_obj(&self) -> WorldObject {
        self.item.obj_type
    }
}
#[derive(Debug)]
pub struct SubmitEssenceChoice {
    pub choice: EssenceOption,
}

#[derive(Resource, Component, Clone, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct EssenceShopChoices {
    pub choices: Vec<EssenceOption>,
}

pub fn setup_essence_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shop: Res<EssenceShopChoices>,
    resolution: Res<ScreenResolution>,
) {
    let (size, texture, t_offset) = (
        ESSENCE_UI_SIZE,
        graphics.get_ui_element_texture(UIElement::Essence),
        Vec2::new(3.5, 3.5),
    );

    let overlay = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(146. / 255., 116. / 255., 65. / 255., 0.3),
                custom_size: Some(Vec2::new(resolution.game_width + 10., GAME_HEIGHT + 10.)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-t_offset.x, -t_offset.y, -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("overlay"))
        .id();

    let essence_ui_e = commands
        .spawn(SpriteBundle {
            texture,
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(t_offset.x, t_offset.y, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(EssenceUI)
        .insert(Name::new("STATS UI"))
        .insert(UIState::Essence)
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    for (i, essence_option) in shop.choices.iter().enumerate() {
        let translation = Vec3::new(24.5, 40.5 - (i as f32 * 29.), 1.);
        let slot_entity = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::EssenceButton),
                    transform: Transform {
                        translation,
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(20., 20.)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Interactable::default(),
                UIElement::EssenceButton,
                essence_option.clone(),
                RenderLayers::from_layers(&[3]),
                Name::new("Essence Loot Button"),
            ))
            .set_parent(essence_ui_e)
            .id();

        // icon
        let icon = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &essence_option.item,
            &asset_server,
            Vec2::ZERO,
            Vec2::new(0., 0.),
            3,
        );
        commands.entity(icon).set_parent(slot_entity);

        let slot_entity = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::EssenceButton),
                    transform: Transform {
                        translation: translation + Vec3::new(-49., 0., 0.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(20., 20.)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                // Interactable::default(),
                UIElement::EssenceButton,
                RenderLayers::from_layers(&[3]),
                Name::new("Essence Cost Button"),
            ))
            .set_parent(essence_ui_e)
            .id();

        // cost icon
        let cost_icon = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &ItemStack::crate_icon_stack(WorldObject::Essence)
                .copy_with_count(essence_option.cost as usize),
            &asset_server,
            Vec2::ZERO,
            Vec2::new(0., 0.),
            3,
        );
        commands.entity(cost_icon).set_parent(slot_entity);
    }

    commands.entity(essence_ui_e).push_children(&[overlay]);
}

pub fn handle_submit_essence_choice(
    mut commands: Commands,
    mut ev: EventReader<SubmitEssenceChoice>,
    mut next_inv_state: ResMut<NextState<UIState>>,
    essence_ui: Query<Entity, With<EssenceUI>>,
    mut inv: Query<&mut Inventory>,
    mut game_param: GameParam,
    player_t: Query<&GlobalTransform, With<Player>>,
) {
    for choice in ev.iter() {
        let mut inv = inv.single_mut();
        if inv
            .items
            .remove_from_inventory(choice.choice.cost as usize, WorldObject::Essence).is_ok()
        {
            choice.choice.item.spawn_as_drop(
                &mut commands,
                &mut game_param,
                player_t.single().translation().truncate(),
            );

            next_inv_state.set(UIState::Closed);
            commands.remove_resource::<EssenceShopChoices>();
            if let Ok(e) = essence_ui.get_single() {
                commands.entity(e).despawn_recursive();
            }
        }
    }
}

pub fn handle_populate_essence_shop_on_new_spawn(
    mut new_spawns: Query<&mut EssenceShopChoices, Added<EssenceShopChoices>>,
    proto_param: ProtoParam,
) {
    for mut shop in new_spawns.iter_mut() {
        let mut GENERIC_SHOP_OPTIONS = vec![
            EssenceOption {
                item: create_new_random_item_stack_with_attributes(
                    &ItemStack::crate_icon_stack(WorldObject::LargePotion).copy_with_count(3),
                    &proto_param,
                ),
                cost: 3,
            },
            EssenceOption {
                item: create_new_random_item_stack_with_attributes(
                    &ItemStack::crate_icon_stack(WorldObject::MiracleSeed).copy_with_count(1),
                    &proto_param,
                ),
                cost: 5,
            },
            EssenceOption {
                item: create_new_random_item_stack_with_attributes(
                    &ItemStack::crate_icon_stack(WorldObject::UpgradeTome).copy_with_count(1),
                    &proto_param,
                ),
                cost: 4,
            },
            EssenceOption {
                item: create_new_random_item_stack_with_attributes(
                    &ItemStack::crate_icon_stack(WorldObject::Key).copy_with_count(1),
                    &proto_param,
                ),
                cost: 10,
            },
        ];

        let mut shop_choices = vec![];
        if let Ok(file_file) = File::open(datafiles::game_data()) {
            let reader = BufReader::new(file_file);
            let mut rng = rand::thread_rng();
            // Read the JSON contents of the file as an instance of `User`.
            match serde_json::from_reader::<_, GameData>(reader) {
                Ok(data) => {
                    if let Some(seen_item) = data.seen_gear.iter().choose(&mut rng) {
                        shop_choices.push(EssenceOption {
                            item: seen_item.clone(),
                            cost: 5,
                        });
                        while shop_choices.len() < 4 {
                            let pick = GENERIC_SHOP_OPTIONS
                                .iter()
                                .choose(&mut rng)
                                .unwrap()
                                .clone();
                            shop_choices.push(pick.clone());
                            GENERIC_SHOP_OPTIONS.retain(|x| x.get_obj() != pick.get_obj());
                        }
                    } else {
                        shop_choices = GENERIC_SHOP_OPTIONS;
                    }
                }
                Err(err) => {
                    warn!("No previous runs found, no gear to populate shop with {err:?}");
                    shop_choices = GENERIC_SHOP_OPTIONS;
                }
            }
        };
        shop.choices = shop_choices;
    }
}
