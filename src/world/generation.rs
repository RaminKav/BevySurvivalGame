use super::chunk::{GenerateObjectsEvent, TileSpriteData};
use super::dimension::{ActiveDimension, GenerationSeed};
use super::dungeon::Dungeon;
use super::noise_helpers::{_poisson_disk_sampling, get_object_points_for_chunk};
use super::portal::{Portal, TimePortal};
use super::wall_auto_tile::{handle_wall_break, handle_wall_placed, update_wall, ChunkWallCache};
use super::world_helpers::tile_pos_to_world_pos;
use super::y_sort::YSort;
use super::{WorldGeneration, ISLAND_SIZE};
use crate::assets::SpriteAnchor;
use crate::container::ContainerRegistry;
use crate::enemy::spawn_helpers::is_tile_water;
use crate::enemy::Mob;
use crate::item::{handle_break_object, WorldObject};
use crate::player::Player;
use crate::proto::proto_param::ProtoParam;
use crate::schematic::loot_chests::get_random_loot_chest_type;
use crate::schematic::SchematicSpawnEvent;
use crate::ui::key_input_guide::InteractionGuideTrigger;
use crate::ui::minimap::UpdateMiniMapEvent;

use bevy_aseprite::anim::AsepriteAnimation;
use bevy_aseprite::AsepriteBundle;
use itertools::Itertools;

use crate::world::world_helpers::{get_neighbour_tile, world_pos_to_tile_pos};
use crate::world::{noise_helpers, world_helpers, TileMapPosition, CHUNK_SIZE, TILE_SIZE};
use crate::{custom_commands::CommandsExt, CustomFlush, GameParam, GameState};
use crate::{DEBUG, NO_GEN};

use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::prelude::*;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use bevy_rapier2d::prelude::Collider;

use rand::{
    seq::{IteratorRandom, SliceRandom},
    Rng,
};

#[derive(Debug, Clone)]
pub struct WallBreakEvent {
    pub pos: TileMapPosition,
}
pub struct DoneGeneratingEvent {
    pub chunk_pos: IVec2,
}

const UNIQUE_OBJECTS_DATA: [(WorldObject, Vec2, i32); 2] = [
    (WorldObject::BossShrine, Vec2::new(8., 8.), 10),
    (WorldObject::DungeonEntrance, Vec2::new(2., 2.), 7),
    // (WorldObject::TimeGate, Vec2::new(2., 2.), 3),
];
const STARTING_ZONE_OBJS: [(WorldObject, i32); 3] = [
    (WorldObject::Pebble, 1),
    (WorldObject::DeadSapling, 1),
    (WorldObject::BrownMushroom, 1),
];

#[derive(Resource, Debug, Default, Clone)]
pub struct WorldObjectCache {
    pub objects: HashMap<TileMapPosition, WorldObject>,
    pub unique_objs: HashMap<WorldObject, TileMapPosition>,
    pub dungeon_objects: HashMap<TileMapPosition, WorldObject>,
    pub generated_chunks: Vec<IVec2>,
    pub generated_dungeon_chunks: Vec<IVec2>,
    pub tile_data_cache: HashMap<TileMapPosition, TileSpriteData>,
}
pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<WallBreakEvent>()
            .add_event::<DoneGeneratingEvent>()
            .add_systems(
                (
                    handle_wall_break
                        .before(CustomFlush)
                        .before(handle_break_object),
                    handle_wall_placed.before(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                Self::generate_unique_objects_for_new_world.in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                Self::generate_and_cache_objects.before(CustomFlush).run_if(
                    resource_exists::<GenerationSeed>().and_then(in_state(GameState::Main)),
                ),
            )
            .add_system(
                update_wall
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(spawn_debug_chunk_borders.in_schedule(OnEnter(GameState::Main)))
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

impl GenerationPlugin {
    fn get_perlin_block_at_tile(
        world_generation_params: &WorldGeneration,
        pos: TileMapPosition,
        seed: u64,
    ) -> Option<WorldObject> {
        let x = pos.tile_pos.x as f64;
        let y = pos.tile_pos.y as f64;
        // dont need to use expencive noise fn if it will always
        // result in the same tile
        if world_generation_params.stone_wall_frequency == 1. {
            return Some(WorldObject::StoneWall);
        }
        let nx = (x as i32 + pos.chunk_pos.x * CHUNK_SIZE as i32) as f64;
        let ny = (y as i32 + pos.chunk_pos.y * CHUNK_SIZE as i32) as f64;
        let e = noise_helpers::get_perlin_noise_for_tile(nx, ny, seed);
        if e <= world_generation_params.stone_wall_frequency {
            return Some(WorldObject::StoneWall);
        }
        None
    }
    fn generate_stone_for_chunk(
        world_generation_params: &WorldGeneration,
        chunk_pos: IVec2,
        seed: u64,
    ) -> Vec<(TileMapPosition, WorldObject)> {
        let mut stone_blocks: Vec<(TileMapPosition, WorldObject)> = vec![];
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                let pos = TileMapPosition::new(chunk_pos, TilePos { x, y });
                if let Some(block) =
                    Self::get_perlin_block_at_tile(world_generation_params, pos, seed)
                {
                    stone_blocks.push((pos, block));
                }
            }
        }
        stone_blocks
    }
    fn generate_forest_for_chunk(
        world_generation_params: &WorldGeneration,
        chunk_pos: IVec2,
        seed: u64,
    ) -> Vec<(TileMapPosition, WorldObject)> {
        let mut rng = rand::thread_rng();

        //TODO: make these come from proto, use frequencies?
        let TREES = world_generation_params.forest_params.tree_weights.clone();
        let spawn_ring_offset = if chunk_pos == IVec2::new(0, 0)
            || chunk_pos == IVec2::new(0, -1)
            || chunk_pos == IVec2::new(-1, 0)
            || chunk_pos == IVec2::new(-1, -1)
        {
            6
        } else {
            0
        };
        let num_clusters = if rng.gen_ratio(1, 2) { 3 } else { 2 } + spawn_ring_offset;
        let mut trees: Vec<(TileMapPosition, WorldObject)> = vec![];
        for _ in 0..num_clusters {
            let mut picked_trees = TREES
                .iter()
                .collect_vec()
                .choose_multiple_weighted(&mut rng.clone(), 2, |item| *item.1 as f64)
                .unwrap()
                .map(|x| x.0)
                .collect_vec();
            if picked_trees.contains(&&WorldObject::RedTree) {
                picked_trees = vec![&WorldObject::RedTree];
            }
            // for now, every chunk will get 1 forest startt point
            let rand_x = rng.gen_range(0..CHUNK_SIZE) as f32;
            let rand_y = rng.gen_range(0..CHUNK_SIZE) as f32;
            let forest_nucleous = Vec2::new(rand_x, rand_y);
            let noise_points = _poisson_disk_sampling(
                world_generation_params.forest_params.tree_spacing_radius,
                30,
                f32::min(
                    world_generation_params.forest_params.tree_density * 100.
                        + spawn_ring_offset as f32,
                    1.,
                ),
                world_generation_params.forest_params.forest_radius * TILE_SIZE.x,
                world_generation_params.forest_params.max_trees_per_forest,
                forest_nucleous,
                rng.clone(),
            );
            for point in noise_points {
                let x = point.0;
                let y = point.1;
                let updated_pos = Vec2::new(
                    x + (chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                    y + (chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.y),
                );
                let pos = world_pos_to_tile_pos(updated_pos);
                trees.push((pos, **picked_trees.iter().choose(&mut rng.clone()).unwrap()));
            }
        }

        trees
    }
    // Use chunk manager as source of truth for index

    //TODO: update this to use new constants at top of file
    fn _smooth_terrain(
        k: i8,
        tile_storage: &mut TileStorage,
        tile_index_grid: [[u32; 16]; 16],
        commands: &mut Commands,
    ) {
        // Create a new grid to hold the smoothed terrain
        let mut smooth_grid = [[10000; 16_usize]; 16_usize];

        // Loop over each tile in the grid
        for y in 0..16 {
            for x in 0..16 {
                let current_tile = tile_index_grid[x as usize][y as usize];
                // Count the number of adjacent tiles that are the same type as the current tile
                let mut adjacent_count = 0;
                let mut previous_tile: u32 = 10000;
                let mut smooth_tile: u32 = 10000;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if x + dx >= 0 && x + dx < 16 && y + dy >= 0 && y + dy < 16 {
                            let adj_tile = tile_index_grid[i32::abs(x + dx) as usize]
                                [i32::abs(y + dy) as usize];
                            if adj_tile == current_tile {
                                continue;
                            }
                            if adj_tile == previous_tile {
                                adjacent_count += 1;
                                if adjacent_count >= k {
                                    smooth_tile = adj_tile;
                                }
                            } else {
                                previous_tile = adj_tile;
                            }
                        }
                    }
                }
                // If at least 5 adjacent tiles are the same type, set the smooth_grid value to 1
                // (indicating that this tile should be the same type as the current tile)
                if adjacent_count >= k {
                    smooth_grid[y as usize][x as usize] = smooth_tile;
                }
            }
        }

        // Use the smooth_grid to set the tile types in the tile_storage
        for y in 0..16 {
            for x in 0..16 {
                let tile_pos = TilePos {
                    x: x.try_into().unwrap(),
                    y: y.try_into().unwrap(),
                };
                if smooth_grid[y][x] < 1000 {
                    // tile_storage.get(&tile_pos, smoothed_tile);
                    commands
                        .entity(tile_storage.get(&tile_pos).unwrap())
                        .insert(TileTextureIndex(smooth_grid[y][x]));
                }
            }
        }
    }

    //TODO: do the same shit w graphcis resource loading, but w GameData and pkvStore
    pub fn generate_unique_objects_for_new_world(
        mut game: GameParam,
        new_dim: Query<Entity, Added<ActiveDimension>>,
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        dungeon_check: Query<&Dungeon>,
    ) {
        if new_dim.is_empty() {
            return;
        }
        let max_obj_spawn_radius = ((ISLAND_SIZE / CHUNK_SIZE as f32) - 2.) as i32;
        for (obj_to_clear, _size, _) in UNIQUE_OBJECTS_DATA {
            if !game.world_obj_cache.unique_objs.contains_key(&obj_to_clear) {
                debug!("NEW UNIQUE OBJ: {obj_to_clear:?}");

                let mut rng = rand::thread_rng();

                let mut pos = TileMapPosition::new(
                    IVec2::new(
                        rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                        rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                    ),
                    TilePos::new(rng.gen_range(0..15), rng.gen_range(0..15)),
                );
                if pos.chunk_pos == IVec2::ZERO {
                    pos.chunk_pos = IVec2::new(2, 2);
                }
                if pos.chunk_pos.x == 1 {
                    pos.chunk_pos.x = 2;
                }
                if pos.chunk_pos.y == 1 {
                    pos.chunk_pos.y = 2;
                }
                if pos.chunk_pos.x == -1 {
                    pos.chunk_pos.x = -2;
                }
                if pos.chunk_pos.y == -1 {
                    pos.chunk_pos.y = -2;
                }
                debug!("set up a {obj_to_clear:?} at {pos:?}");
                game.world_obj_cache.unique_objs.insert(obj_to_clear, pos);
            }
        }
        if dungeon_check.get_single().is_err() {
            // summon portal
            commands
                .spawn(VisibilityBundle::default())
                .insert(YSort(0.))
                .insert(TimePortal)
                .insert(WorldObject::TimePortal)
                .insert(SpriteAnchor(Vec2::new(0., 10.)))
                .insert(InteractionGuideTrigger {
                    key: None,
                    text: Some("???".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                })
                .insert(Collider::capsule(
                    Vec2::new(0., 10.),
                    Vec2::new(0., -18.),
                    11.,
                ))
                .insert(AsepriteBundle {
                    aseprite: asset_server.load(Portal::PATH),
                    animation: AsepriteAnimation::from(Portal::tags::IDLE),
                    transform: Transform::from_translation(Vec3::new(0., 50., 0.)),
                    ..Default::default()
                })
                .insert(Name::new("Time Portal"));
        }
    }
    pub fn generate_and_cache_objects(
        mut commands: Commands,
        mut game: GameParam,
        mut chunk_spawn_event: EventReader<GenerateObjectsEvent>,
        dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
        seed: Res<GenerationSeed>,
        mut minimap_update: EventWriter<UpdateMiniMapEvent>,
        mut chunk_wall_cache: Query<&mut ChunkWallCache>,
        mut proto_commands: ProtoCommands,
        prototypes: Prototypes,
        mut proto_param: ProtoParam,
        container_reg: Res<ContainerRegistry>,
        water_colliders: Query<
            (Entity, &Collider, &GlobalTransform),
            (Without<WorldObject>, Without<Mob>, Without<Player>),
        >,
        mut done_event: EventWriter<DoneGeneratingEvent>,
        mut schematic_spawn_event: EventWriter<SchematicSpawnEvent>,
    ) {
        if *NO_GEN {
            return;
        }
        for chunk in chunk_spawn_event.iter() {
            let chunk_pos = chunk.chunk_pos;
            let chunk_e = game.get_chunk_entity(chunk_pos).unwrap();
            let dungeon_check = dungeon_check.get_single();
            let is_chunk_generated = if dungeon_check.is_ok() {
                game.is_dungeon_chunk_generated(chunk_pos)
            } else {
                game.is_chunk_generated(chunk_pos)
            };
            if !is_chunk_generated {
                debug!("Generating new objects for {chunk_pos:?}");
                // generate stone walls for dungeons
                let stone = Self::generate_stone_for_chunk(
                    &game.world_generation_params,
                    chunk_pos,
                    seed.seed,
                );
                // generate forest walls trees for chunk
                let mut trees = Self::generate_forest_for_chunk(
                    &game.world_generation_params,
                    chunk_pos,
                    seed.seed,
                );

                // random size forest clearings
                if chunk_pos.x.abs() > 1 || chunk_pos.y.abs() > 1 {
                    let mut rng = rand::thread_rng();
                    let rng_x = rng.gen_range(0..CHUNK_SIZE);
                    let rng_y = rng.gen_range(0..CHUNK_SIZE);
                    let clear_tiles = get_radial_tile_positions(
                        TileMapPosition::new(chunk_pos, TilePos::new(rng_x, rng_y)),
                        rng.gen_range(4..8),
                    );
                    trees = trees
                        .into_iter()
                        .filter(|tp| !clear_tiles.contains(&tp.0))
                        .collect_vec();
                }

                // generate all objs
                let mut objs_to_spawn: Box<dyn Iterator<Item = (TileMapPosition, WorldObject)>> =
                    Box::new(stone.clone().into_iter().chain(trees.clone().into_iter()));
                let mut occupied_tiles: HashMap<TileMapPosition, WorldObject> =
                    stone.into_iter().chain(trees.into_iter()).collect();

                for (obj_to_clear, frequency) in game
                    .world_generation_params
                    .object_generation_frequencies
                    .iter()
                {
                    let mut validated_objs: Vec<(TileMapPosition, WorldObject)> = vec![];
                    let raw_points = get_object_points_for_chunk(seed.seed, *frequency);
                    let points = raw_points
                        .iter()
                        .map(|tp| {
                            let tp_vec = Vec2::new(
                                tp.0 + (chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                                tp.1 + (chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                            );

                            let relative_tp = world_helpers::world_pos_to_tile_pos(tp_vec);
                            (relative_tp, *obj_to_clear)
                        })
                        .collect::<Vec<(TileMapPosition, WorldObject)>>();
                    for (pos, obj_to_clear) in points.iter() {
                        // check if tile(s) already occupied by another object waiting to spawn
                        let is_medium = obj_to_clear.is_medium_size(&proto_param);
                        let tiles_obj_wants_to_take_up = if is_medium {
                            pos.get_neighbour_tiles_for_medium_objects()
                                .into_iter()
                                .chain(vec![*pos])
                                .collect_vec()
                        } else {
                            vec![*pos]
                        };
                        if tiles_obj_wants_to_take_up
                            .iter()
                            .any(|p| occupied_tiles.contains_key(p))
                        {
                            // override chests and dungeon exits, skip anything else
                            if obj_to_clear == &WorldObject::DungeonExit
                                || obj_to_clear == &WorldObject::Chest
                                || obj_to_clear == &WorldObject::DungeonEntrance
                            {
                                occupied_tiles.remove(pos);
                                occupied_tiles.insert(*pos, *obj_to_clear);
                            } else {
                                continue;
                            }
                        }

                        // mark tiles as occupied for future objects
                        tiles_obj_wants_to_take_up.iter().for_each(|p| {
                            occupied_tiles.insert(*p, *obj_to_clear);
                        });
                        validated_objs.push((*pos, *obj_to_clear));
                    }

                    objs_to_spawn = Box::new(objs_to_spawn.chain(validated_objs.into_iter()));
                }

                // generate starting area objs to ensure player has enough pebbles/sticks
                if chunk_pos == IVec2::ZERO
                    || chunk_pos == IVec2::new(-1, 0)
                    || chunk_pos == IVec2::new(0, -1)
                    || chunk_pos == IVec2::new(-1, -1)
                {
                    let mut starting_objs = vec![];
                    for (obj_to_clear, num) in STARTING_ZONE_OBJS.iter() {
                        let x_range = if chunk_pos.x == 0 {
                            0..CHUNK_SIZE / 2
                        } else {
                            CHUNK_SIZE / 2..CHUNK_SIZE
                        };
                        let y_range = if chunk_pos.y == 0 {
                            0..CHUNK_SIZE / 2
                        } else {
                            10..CHUNK_SIZE
                        };
                        for _ in 0..*num {
                            starting_objs.push((
                                TileMapPosition::new(
                                    chunk_pos,
                                    TilePos::new(
                                        rand::thread_rng().gen_range(x_range.clone()),
                                        rand::thread_rng().gen_range(y_range.clone()),
                                    ),
                                ),
                                *obj_to_clear,
                            ));
                        }
                    }
                    objs_to_spawn = Box::new(objs_to_spawn.chain(starting_objs.into_iter()));
                }
                let mut objs_to_spawn =
                    objs_to_spawn.collect::<Vec<(TileMapPosition, WorldObject)>>();
                if dungeon_check.is_err() {
                    let cached_objs = game.get_objects_from_chunk_cache(chunk_pos);
                    objs_to_spawn = objs_to_spawn
                        .into_iter()
                        .chain(cached_objs.to_owned().into_iter())
                        .collect::<Vec<(TileMapPosition, WorldObject)>>();
                } else {
                    let cached_objs = game.get_objects_from_dungeon_cache(chunk_pos);
                    objs_to_spawn = objs_to_spawn
                        .into_iter()
                        .chain(cached_objs.to_owned().into_iter())
                        .collect::<Vec<(TileMapPosition, WorldObject)>>();
                }
                let mut objs = objs_to_spawn
                    .iter()
                    .filter(|tp| {
                        if tp.1 == WorldObject::None {
                            return false;
                        }
                        // spawn walls in dungeon according to the generated grid layout
                        if let Ok(dungeon) = dungeon_check {
                            let mut wall_cache = chunk_wall_cache.get_mut(chunk_e).unwrap();
                            if chunk_pos.x < -3
                                || chunk_pos.x > 4
                                || chunk_pos.y < -4
                                || chunk_pos.y > 3
                            {
                                if tp.1.is_wall() {
                                    wall_cache.walls.insert(tp.0, true);
                                    return true;
                                } else {
                                    return false;
                                }
                            }

                            if dungeon.grid[(CHUNK_SIZE as i32 * (4 - chunk_pos.y)
                                - 1
                                - (tp.0.tile_pos.y as i32))
                                as usize][(3 * CHUNK_SIZE as i32
                                + (chunk_pos.x * CHUNK_SIZE as i32)
                                + tp.0.tile_pos.x as i32)
                                as usize]
                                == 1
                            {
                                if tp.1.is_wall() {
                                    wall_cache.walls.insert(tp.0, false);
                                    return false;
                                }
                            } else if !tp.1.is_wall() {
                                return false;
                            }
                        }
                        let tile = if let Some(tile_data) = game.get_tile_data(tp.0) {
                            tile_data.block_type
                        } else {
                            return false;
                        };

                        let filter = game
                            .world_generation_params
                            .obj_allowed_tiles_map
                            .get(&tp.1)
                            .unwrap_or_else(|| {
                                panic!("no allowed tiles for obj_to_clear {:?}", &tp.1)
                            });
                        for allowed_tile in filter.iter() {
                            if tile.iter().filter(|t| *t == allowed_tile).count() == 4 {
                                return true;
                            }
                        }
                        false
                    })
                    .copied()
                    .collect::<HashMap<_, _>>();
                // clear out spawn area
                let clear_tiles = get_radial_tile_positions(
                    TileMapPosition::new(IVec2::ZERO, TilePos::new(0, 0)),
                    10,
                );
                for pos in clear_tiles {
                    if let Some(obj_to_clear) = objs.get(&pos) {
                        if obj_to_clear.is_tree() {
                            objs.remove(&pos);
                        }
                    }
                }
                // clear out portal area
                let clear_tiles = get_radial_tile_positions(
                    TileMapPosition::new(IVec2::ZERO, TilePos::new(0, 2)),
                    2,
                );
                for pos in clear_tiles {
                    if let Some(_obj) = objs.get(&pos) {
                        objs.remove(&pos);
                    }
                }
                // UNIQUE OBJECTS
                if dungeon_check.is_err() {
                    for (unique_obj, pos) in game.world_obj_cache.unique_objs.clone() {
                        if pos.chunk_pos == chunk_pos {
                            //TODO: this will be funky if size is not even integers
                            let x_halfsize = (UNIQUE_OBJECTS_DATA
                                .iter()
                                .find(|(o, _, _)| o == &unique_obj)
                                .map(|(_, s, _)| s)
                                .unwrap()
                                .x
                                / 2.) as i32;
                            let y_halfsize = (UNIQUE_OBJECTS_DATA
                                .iter()
                                .find(|(o, _, _)| o == &unique_obj)
                                .map(|(_, s, _)| s)
                                .unwrap()
                                .y
                                / 2.) as i32;

                            let mut pos = pos;
                            let mut found_non_water_location = false;
                            'repeat: while !found_non_water_location {
                                for x in (-x_halfsize)..=x_halfsize {
                                    for y in (-y_halfsize)..=y_halfsize {
                                        let n_pos = tile_pos_to_world_pos(
                                            get_neighbour_tile(pos, (x as i8, y as i8)),
                                            false,
                                        );
                                        if is_tile_water(n_pos, &game).is_ok_and(|x| x) {
                                            let mut rng = rand::thread_rng();

                                            pos = TileMapPosition::new(
                                                chunk_pos,
                                                TilePos::new(
                                                    rng.gen_range(0..15),
                                                    rng.gen_range(0..15),
                                                ),
                                            );
                                            debug!("relocating {unique_obj:?} to {pos:?}");
                                            continue 'repeat;
                                        }
                                    }
                                }
                                found_non_water_location = true;
                            }
                            game.world_obj_cache.unique_objs.insert(unique_obj, pos);

                            objs.insert(pos, unique_obj);
                            debug!("SPAWNING UNIQUE {unique_obj:?} at {pos:?} {x_halfsize:?}");
                        }
                        // clear out area
                        let clear_tiles = get_radial_tile_positions(
                            pos,
                            *UNIQUE_OBJECTS_DATA
                                .iter()
                                .find(|(o, _, _)| o == &unique_obj)
                                .map(|(_, _, r)| r)
                                .unwrap() as i8,
                        );
                        for pos_to_clear in clear_tiles {
                            if let Some(obj_to_clear) = objs.get(&pos_to_clear) {
                                if (obj_to_clear.is_tree()
                                    || obj_to_clear.is_medium_size(&proto_param))
                                    && !obj_to_clear.is_unique_object()
                                {
                                    objs.remove(&pos_to_clear);
                                    if let Some((entity_to_despawn, obj_to_despawn)) =
                                        game.get_obj_entity_at_tile(pos_to_clear, &proto_param)
                                    {
                                        if (obj_to_despawn.is_tree()
                                            || obj_to_despawn.is_medium_size(&proto_param))
                                            && !obj_to_despawn.is_unique_object()
                                        {
                                            commands.entity(entity_to_despawn).despawn_recursive();
                                        }
                                    }
                                    game.remove_object_from_chunk_cache(pos_to_clear);
                                }
                            }
                        }
                    }
                }

                for (pos, obj_to_spawn) in objs.iter() {
                    let mut is_touching_air = false;
                    if let Ok(dungeon) = dungeon_check {
                        for x in -1_i32..2 {
                            for y in -1_i32..2 {
                                let original_y = ((CHUNK_SIZE) as i32 * (4 - pos.chunk_pos.y)
                                    - 1
                                    - (pos.tile_pos.y as i32))
                                    as usize;
                                let original_x = ((3 * CHUNK_SIZE) as i32
                                    + (pos.chunk_pos.x * CHUNK_SIZE as i32)
                                    + pos.tile_pos.x as i32)
                                    as usize;
                                if dungeon.grid[(original_y + y as usize).clamp(0, 127)]
                                    [(original_x + x as usize).clamp(0, 127)]
                                    == 1
                                {
                                    is_touching_air = true
                                }
                            }
                        }
                    }
                    // only spawn if generated obj_to_clear is in our chunk or a previously genereated chunk,
                    // otherwise cache it for the correct chunk to spawn
                    let obj_e = if pos.chunk_pos == chunk_pos
                        || game.is_chunk_generated(pos.chunk_pos)
                    {
                        proto_commands.spawn_object_from_proto(
                            *obj_to_spawn,
                            tile_pos_to_world_pos(*pos, obj_to_spawn.is_medium_size(&proto_param)),
                            &prototypes,
                            &mut proto_param,
                            is_touching_air,
                        )
                    } else {
                        game.add_object_to_chunk_cache(*pos, *obj_to_spawn);
                        None
                    };

                    if let Some(spawned_obj) = obj_e {
                        if obj_to_spawn.is_medium_size(&proto_param) {
                            minimap_update.send(UpdateMiniMapEvent {
                                pos: Some(*pos),
                                new_tile: Some(*obj_to_spawn),
                            });
                            for q in 0..3 {
                                minimap_update.send(UpdateMiniMapEvent {
                                    pos: Some(pos.get_neighbour_tiles_for_medium_objects()[q]),
                                    new_tile: Some(*obj_to_spawn),
                                });
                            }
                        } else {
                            minimap_update.send(UpdateMiniMapEvent {
                                pos: Some(*pos),
                                new_tile: Some(*obj_to_spawn),
                            });
                        }

                        if obj_to_spawn == &WorldObject::Chest
                            && container_reg.containers.get(pos).is_none()
                        {
                            commands
                                .entity(spawned_obj)
                                .insert(get_random_loot_chest_type(rand::thread_rng()));
                        } else if obj_to_spawn == &WorldObject::Bridge {
                            for (e, _c, t) in water_colliders.iter() {
                                if t.translation()
                                    .truncate()
                                    .distance(tile_pos_to_world_pos(*pos, false))
                                    <= 6.
                                {
                                    commands.entity(e).despawn();
                                }
                            }
                        }
                        commands
                            .entity(spawned_obj)
                            .set_parent(game.get_chunk_entity(pos.chunk_pos).unwrap());

                        if dungeon_check.is_ok() {
                            let mut wall_cache = chunk_wall_cache.get_mut(chunk_e).unwrap();
                            if obj_to_spawn.is_wall() {
                                wall_cache.walls.insert(*pos, true);
                            }
                            game.add_object_to_dungeon_cache(*pos, *obj_to_spawn);
                        } else {
                            game.add_object_to_chunk_cache(*pos, *obj_to_spawn);
                        }
                    }
                }
                if dungeon_check.is_err() {
                    game.set_chunk_generated(chunk_pos);
                } else {
                    game.set_dungeon_chunk_generated(chunk_pos);
                }

                // send schematic event to spawn structures
                schematic_spawn_event.send(SchematicSpawnEvent(chunk_pos));
            } else {
                let objs = if dungeon_check.is_ok() {
                    game.get_objects_from_dungeon_cache(chunk_pos)
                } else {
                    game.get_objects_from_chunk_cache(chunk_pos)
                };
                for (pos, obj_to_clear) in objs {
                    let spawned_obj = proto_commands.spawn_object_from_proto(
                        obj_to_clear,
                        tile_pos_to_world_pos(pos, obj_to_clear.is_medium_size(&proto_param)),
                        &prototypes,
                        &mut proto_param,
                        true,
                    );

                    if let Some(spawned_obj) = spawned_obj {
                        let mut wall_cache = chunk_wall_cache.get_mut(chunk_e).unwrap();
                        if obj_to_clear.is_wall() {
                            wall_cache.walls.insert(pos, true);
                        } else if obj_to_clear == WorldObject::Chest
                            && container_reg.containers.get(&pos).is_none()
                        {
                            commands
                                .entity(spawned_obj)
                                .insert(get_random_loot_chest_type(rand::thread_rng()));
                        } else if obj_to_clear == WorldObject::Bridge {
                            for (e, _c, t) in water_colliders.iter() {
                                if t.translation()
                                    .truncate()
                                    .distance(tile_pos_to_world_pos(pos, false))
                                    <= 6.
                                {
                                    commands.entity(e).despawn();
                                }
                            }
                        }
                        minimap_update.send(UpdateMiniMapEvent {
                            pos: Some(pos),
                            new_tile: Some(obj_to_clear),
                        });

                        commands
                            .entity(spawned_obj)
                            .set_parent(game.get_chunk_entity(chunk_pos).unwrap());
                    }
                }
            }

            done_event.send(DoneGeneratingEvent { chunk_pos });
        }
    }
}

fn spawn_debug_chunk_borders(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !*DEBUG {
        return;
    }
    let offset = Vec2::new(-8., -8.);
    //vertical
    for i in -10..10 {
        commands
            .spawn(MaterialMesh2dBundle {
                mesh: meshes
                    .add(
                        shape::Quad {
                            size: Vec2::new(1.0, 100000.0),
                            ..Default::default()
                        }
                        .into(),
                    )
                    .into(),
                transform: Transform::from_translation(Vec3::new(
                    i as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x + offset.x,
                    0. + offset.y,
                    900.,
                )),
                material: materials.add(Color::RED.into()),
                ..default()
            })
            .insert(Name::new("debug chunk border y"));
    }

    //horizontal
    for i in -10..10 {
        commands
            .spawn(MaterialMesh2dBundle {
                mesh: meshes
                    .add(
                        shape::Quad {
                            size: Vec2::new(100000.0, 1.0),
                            ..Default::default()
                        }
                        .into(),
                    )
                    .into(),
                transform: Transform::from_translation(Vec3::new(
                    offset.x,
                    i as f32 * CHUNK_SIZE as f32 * TILE_SIZE.y + offset.y,
                    900.,
                )),
                material: materials.add(Color::RED.into()),
                ..default()
            })
            .insert(Name::new("debug chunk border x"));
    }
}

pub fn get_radial_tile_positions(origin: TileMapPosition, radius: i8) -> Vec<TileMapPosition> {
    //TODO: add rng padding around edges
    let mut positions = vec![];
    let origin_pos = tile_pos_to_world_pos(origin, false);
    let max_dist = radius as i32 * (TILE_SIZE.x as i32);
    for x in -radius..=radius {
        for y in -radius..=radius {
            let pos = get_neighbour_tile(origin, (x, y));
            let dist = tile_pos_to_world_pos(pos, false).distance(origin_pos);
            if dist <= max_dist as f32 {
                positions.push(pos);
            }
        }
    }

    positions
}
