use bevy::prelude::*;
use bevy::time::FixedTimestep;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_rapier2d::prelude::{
    Collider, KinematicCharacterController, KinematicCharacterControllerOutput, MoveShapeOptions,
    QueryFilter, QueryFilterFlags, RapierContext,
};

use crate::item::{Breakable, WorldObjectResource};
use crate::world_generation::{TileMapPositionData, WorldObjectEntityData};
use crate::{
    assets::{Graphics, WORLD_SCALE},
    item::WorldObject,
    world_generation::{ChunkManager, WorldGenerationPlugin, CHUNK_SIZE},
    Game, GameState, Player, PLAYER_DASH_SPEED, PLAYER_MOVE_SPEED, TIME_STEP,
};

#[derive(Default, Resource)]
pub struct CursorPos(Vec3);

#[derive(Component)]
pub struct Direction(pub f32);

pub struct InputsPlugin;

impl Plugin for InputsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorPos(Vec3::new(-100.0, -100.0, 0.0)))
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::move_player)
                    .with_system(Self::update_cursor_pos.after(Self::move_player))
                    .with_system(Self::mouse_click_system),
            );
    }
}

impl InputsPlugin {
    fn move_player(
        mut key_input: ResMut<Input<KeyCode>>,
        mut game: ResMut<Game>,
        mut player_query: Query<
            (Entity, &mut Transform, &Collider, &mut Direction),
            (With<Player>, Without<Camera>),
        >,
        mut camera_query: Query<&mut Transform, (With<Camera>, Without<Player>)>,
        time: Res<Time>,
        mut controlers: Query<&mut KinematicCharacterController>,
        controlers_output: Query<&mut KinematicCharacterControllerOutput>,
        mut context: ResMut<RapierContext>,
    ) {
        let (ent, mut player_transform, player_collider, mut dir) = player_query.single_mut();
        let mut camera_transform = camera_query.single_mut();

        let mut dx = 0.0;
        let mut dy = 0.0;
        let s = 10.0 / WORLD_SCALE;

        if key_input.pressed(KeyCode::A) {
            dx -= s;
            game.player.is_moving = true;
            player_transform.rotation = Quat::from_rotation_y(std::f32::consts::PI);
        }
        if key_input.pressed(KeyCode::D) {
            dx += s;
            println!("D: {:?}", dx);
            game.player.is_moving = true;
            player_transform.rotation = Quat::default();
        }
        if key_input.pressed(KeyCode::W) {
            dy += s;
            game.player.is_moving = true;
        }
        if key_input.pressed(KeyCode::S) {
            dy -= s;
            game.player.is_moving = true;
        }
        if game.player_dash_cooldown.tick(time.delta()).finished() {
            if key_input.pressed(KeyCode::Space) {
                game.player.is_dashing = true;

                game.player_dash_cooldown.reset();
                game.player_dash_duration.reset();
            }
        }
        if key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W])
            || (dx == 0. && dy == 0.)
        {
            game.player.is_moving = false;
            // key_input.release_all();
        }
        if dx != 0. && dy != 0. {
            dx = if dx == -s { -(s * 0.66) } else { s * 0.66 };
            dy = if dy == -s { -(s * 0.66) } else { s * 0.66 };
        }

        if game.player.is_dashing {
            game.player_dash_duration.tick(time.delta());

            dx += dx * PLAYER_DASH_SPEED * TIME_STEP;
            dy += dy * PLAYER_DASH_SPEED * TIME_STEP;
            if game.player_dash_duration.just_finished() {
                game.player.is_dashing = false;
            }
        }
        let cx = player_transform.translation.x + dx;
        let cy = player_transform.translation.y + dy;

        let output_ws = context.move_shape(
            Vec2::new(0., dy),
            player_collider,
            player_transform.translation.truncate(),
            0.,
            0.,
            &MoveShapeOptions::default(),
            QueryFilter {
                flags: QueryFilterFlags::EXCLUDE_SENSORS,
                exclude_collider: Some(ent),
                ..default()
            },
            |_| {},
        );

        let output_ad = context.move_shape(
            Vec2::new(dbg!(dx), 0.),
            player_collider,
            player_transform.translation.truncate(),
            0.,
            0.,
            &MoveShapeOptions::default(),
            QueryFilter {
                flags: QueryFilterFlags::EXCLUDE_SENSORS,
                exclude_collider: Some(ent),
                ..default()
            },
            |_| {},
        );
        player_transform.translation +=
            output_ws.effective_translation.extend(0.) + output_ad.effective_translation.extend(0.);
        camera_transform.translation.x = cx;
        camera_transform.translation.y = cy;
        // for mut c in controlers.iter_mut() {
        //     // player_transform.translation.x = px;
        //     // player_transform.translation.y = py;
        //     c.translation = Some(Vec2::new(dx, dy));

        //     camera_transform.translation.x = cx;
        //     camera_transform.translation.y = cy;
        //     if let Ok(output) = controlers_output.get_single() {
        //         // println!("{:?}", output.collisions);

        //         if output.collisions.len() != 0 {
        //             let up_or_down_collisions = output
        //                 .collisions
        //                 .iter()
        //                 .filter(|c| c.toi.normal1.y != 0.)
        //                 .collect::<Vec<_>>();
        //             let left_or_right_collisions = output
        //                 .collisions
        //                 .iter()
        //                 .filter(|c| c.toi.normal1.x != 0.)
        //                 .collect::<Vec<_>>();

        //             if left_or_right_collisions.len() > 0
        //                 && up_or_down_collisions.len() == 0
        //                 && dy != 0.
        //             {
        //                 // pressing A/D
        //                 println!("GOING UP");

        //                 player_transform.translation += output.effective_translation.extend(0.);
        //             } else if up_or_down_collisions.len() > 0
        //                 && left_or_right_collisions.len() == 0
        //                 && dx != 0.
        //             {
        //                 // pressing W/D
        //                 println!("GOING LEft");
        //                 let output = context.move_shape(
        //                     Vec2::new(dx, 0.),
        //                     player_collider,
        //                     player_transform.translation.truncate(),
        //                     0.,
        //                     0.,
        //                     &MoveShapeOptions::default(),
        //                     QueryFilter::default(),
        //                     |_| {},
        //                 );
        //                 player_transform.translation +=
        //                     dbg!(output.effective_translation.extend(0.));
        //             }
        //         }
        //     }
        // }

        if game.player.is_moving == true {
            // println!(
            //     "Player is on chunk {:?} at pos: {:?}",
            //     WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
            //         player_transform.translation.x,
            //         player_transform.translation.y
            //     )),
            //     player_transform.translation
            // );
        }

        if dx != 0. {
            dir.0 = dx;
        }
    }

    pub fn update_cursor_pos(
        windows: Res<Windows>,
        camera_q: Query<(&Transform, &Camera)>,
        mut cursor_moved_events: EventReader<CursorMoved>,
        mut cursor_pos: ResMut<CursorPos>,
    ) {
        for cursor_moved in cursor_moved_events.iter() {
            // To get the mouse's world position, we have to transform its window position by
            // any transforms on the camera. This is done by projecting the cursor position into
            // camera space (world space).
            for (cam_t, cam) in camera_q.iter() {
                *cursor_pos = CursorPos(Self::cursor_pos_in_world(
                    &windows,
                    cursor_moved.position,
                    cam_t,
                    cam,
                ));
            }
        }
    }
    // Converts the cursor position into a world position, taking into account any transforms applied
    // the camera.
    pub fn cursor_pos_in_world(
        windows: &Windows,
        cursor_pos: Vec2,
        cam_t: &Transform,
        cam: &Camera,
    ) -> Vec3 {
        let window = windows.primary();

        let window_size = Vec2::new(window.width(), window.height());

        // Convert screen position [0..resolution] to ndc [-1..1]
        // (ndc = normalized device coordinates)
        let ndc_to_world = cam_t.compute_matrix() * cam.projection_matrix().inverse();
        let ndc = (cursor_pos / window_size) * 2.0 - Vec2::ONE;
        ndc_to_world.project_point3(ndc.extend(0.0))
    }

    fn mouse_click_system(
        mouse_button_input: Res<Input<MouseButton>>,
        cursor_pos: Res<CursorPos>,
        mut chunk_manager: ResMut<ChunkManager>,
        mut commands: Commands,
        mut breakable_query: Query<&Breakable, With<WorldObject>>,
        graphics: Res<Graphics>,
        world_obj_data: Res<WorldObjectResource>,
    ) {
        if mouse_button_input.just_released(MouseButton::Left) {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            if chunk_manager
                .chunk_generation_data
                .contains_key(&TileMapPositionData {
                    chunk_pos,
                    tile_pos: TilePos {
                        x: tile_pos.x as u32,
                        y: tile_pos.y as u32,
                    },
                })
            {
                let obj_data = chunk_manager
                    .chunk_generation_data
                    .get(&TileMapPositionData {
                        chunk_pos,
                        tile_pos: TilePos {
                            x: tile_pos.x as u32,
                            y: tile_pos.y as u32,
                        },
                    })
                    .unwrap();
                obj_data.object.break_item(
                    &mut commands,
                    &world_obj_data,
                    &graphics,
                    &mut chunk_manager,
                    tile_pos,
                    chunk_pos,
                );
            } else {
                let stone = WorldObject::StoneFull.spawn_with_collider(
                    &mut commands,
                    &world_obj_data,
                    &graphics,
                    &mut chunk_manager,
                    tile_pos,
                    chunk_pos,
                    Vec2::new(32., 64.),
                );
                commands
                    .entity(stone)
                    .insert(Breakable(Some(WorldObject::StoneHalf)));
                // commands.spawn(stone);
                // .insert(Name::new("Test Objects"))
                // .push_children(&children)
                chunk_manager.chunk_generation_data.insert(
                    TileMapPositionData {
                        chunk_pos,
                        tile_pos: TilePos {
                            x: tile_pos.x as u32,
                            y: tile_pos.y as u32,
                        },
                    },
                    WorldObjectEntityData {
                        object: WorldObject::StoneFull,
                        entity: stone,
                    },
                );
            }
            // WorldGenerationPlugin::change_tile_and_update_neighbours(
            //     TilePos {
            //         x: tile_pos.x as u32,
            //         y: tile_pos.y as u32,
            //     },
            //     chunk_pos,
            //     0b0000,
            //     0,
            //     &mut chunk_manager,
            //     &mut commands,
            // );
        }
        if mouse_button_input.just_released(MouseButton::Right) {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = dbg!(WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            )));
            let stone = WorldObject::StoneTop.spawn_with_collider(
                &mut commands,
                &world_obj_data,
                &graphics,
                &mut chunk_manager,
                tile_pos,
                chunk_pos,
                Vec2::new(32., 32.),
            );
            commands
                .spawn(SpatialBundle::default())
                // .insert(Name::new("Test Objects"))
                // .push_children(&children)
                .push_children(&[stone]);
            // WorldGenerationPlugin::change_tile_and_update_neighbours(
            //     TilePos {
            //         x: tile_pos.x as u32,
            //         y: tile_pos.y as u32,
            //     },
            //     chunk_pos,
            //     0b0000,
            //     16,
            //     &mut chunk_manager,
            //     &mut commands,
            // );
        }
        if mouse_button_input.just_released(MouseButton::Middle) {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = dbg!(WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            )));
            let stone = WorldObject::StoneFull.spawn(
                &mut commands,
                &world_obj_data,
                &graphics,
                &mut chunk_manager,
                tile_pos,
                chunk_pos,
            );
            commands
                .spawn(SpatialBundle::default())
                // .insert(Name::new("Test Objects"))
                // .push_children(&children)
                .push_children(&[stone]);
            // WorldGenerationPlugin::change_tile_and_update_neighbours(
            //     TilePos {
            //         x: tile_pos.x as u32,
            //         y: tile_pos.y as u32,
            //     },
            //     chunk_pos,
            //     0b0000,
            //     16,
            //     &mut chunk_manager,
            //     &mut commands,
            // );
        }
    }
}
