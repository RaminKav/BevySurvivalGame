use bevy::{app::AppExit, ecs::system::SystemState, prelude::*, utils::HashMap};
use bevy_ecs_tilemap::{
    prelude::{
        TilemapGridSize, TilemapId, TilemapSize, TilemapSpacing, TilemapTexture, TilemapTileSize,
        TilemapType,
    },
    tiles::{TileColor, TileFlip, TilePos, TilePosOld, TileStorage, TileTextureIndex, TileVisible},
    FrustumCulling,
};
use bevy_save::prelude::*;
use rand::Rng;

use crate::{
    attributes::CurrentHealth,
    item::{Foliage, Wall, WorldObject},
    ui::minimap::Minimap,
    world::{
        chunk::{
            Chunk, CreateChunkEvent, DespawnChunkEvent, ReflectedPos, SpawnChunkEvent,
            TileEntityCollection, TileSpriteData,
        },
        dimension::{ActiveDimension, ChunkCache, Dimension, DimensionSpawnEvent, GenerationSeed},
        ChunkManager, TileMapPosition, WallTextureData, WorldGeneration,
    },
    CustomFlush, GameParam, YSort,
};

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct ColliderReflect {
    collider: Vec2,
}
pub struct ClientPlugin;
//TODO: Temp does not work, Save/Load WIP
impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SavePlugins)
            .register_saveable::<GenerationSeed>()
            .register_saveable::<Dimension>()
            .register_saveable::<ActiveDimension>()
            .register_saveable::<ChunkManager>()
            // register tile bundle types
            .register_saveable::<TileSpriteData>()
            .register_saveable::<TilePos>()
            .register_saveable::<TileTextureIndex>()
            .register_saveable::<TilemapId>()
            .register_saveable::<TileVisible>()
            .register_saveable::<TileFlip>()
            .register_saveable::<TileColor>()
            .register_saveable::<TilePosOld>()
            // register chunk bundle types
            .register_saveable::<Chunk>()
            .register_saveable::<TilemapGridSize>()
            .register_saveable::<TilemapType>()
            .register_saveable::<TilemapSize>()
            .register_saveable::<TilemapSpacing>()
            .register_saveable::<TileStorage>()
            .register_saveable::<TilemapTexture>()
            .register_saveable::<TilemapTileSize>()
            .register_saveable::<FrustumCulling>()
            .register_saveable::<GlobalTransform>()
            .register_saveable::<ComputedVisibility>()
            .register_saveable::<TileEntityCollection>()
            // register obj types
            .register_saveable::<WorldObject>()
            .register_saveable::<Foliage>()
            .register_saveable::<Wall>()
            // .register_saveable::<Mesh2dHandle>()
            // .register_saveable::<Handle<FoliageMaterial>>()
            // .register_saveable::<Handle<TextureAtlas>>()
            .register_saveable::<TextureAtlasSprite>()
            .register_saveable::<CurrentHealth>()
            .register_saveable::<WallTextureData>()
            .register_saveable::<YSort>()
            .register_saveable::<TileMapPosition>()
            .register_saveable::<ColliderReflect>()
            .register_saveable::<Name>()
            .register_saveable::<Parent>()
            .register_saveable::<Children>()
            .register_type::<Option<Entity>>()
            .register_type::<Vec<Option<Entity>>>()
            .register_type::<WorldObject>()
            .register_type::<ReflectedPos>()
            .register_type::<WorldGeneration>()
            .register_type_data::<ReflectedPos, ReflectSerialize>()
            .register_type_data::<ReflectedPos, ReflectDeserialize>()
            .register_type::<HashMap<ReflectedPos, Entity>>()
            .register_type::<[WorldObject; 4]>()
            .insert_resource(AppDespawnMode::new(DespawnMode::None))
            .insert_resource(AppMappingMode::new(MappingMode::Strict))
            .add_system(
                Self::load_on_start
                    .run_if(run_once())
                    .in_schedule(CoreSchedule::Startup),
            )
            // .add_systems(
            //     (
            //         // Self::save_chunk,
            //         // Self::despawn_saved_chunks,
            //         // Self::despawn_non_saveable_entities.before(CustomFlush),
            //         // Self::close_and_save_on_esc.after(CustomFlush),
            //         // Self::load_chunk.before(CustomFlush),
            //     )
            //         .in_set(OnUpdate(GameState::Main)),
            // )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

impl ClientPlugin {
    fn load_chunk(world: &mut World) {
        let mut state: SystemState<(
            Query<&ChunkCache, With<ActiveDimension>>,
            EventReader<SpawnChunkEvent>,
        )> = SystemState::new(world);
        let (_dim_query, mut spawn_events) = state.get_mut(world);
        let mut chunks = vec![];
        for event in spawn_events.iter() {
            println!("attempting load chunk {:?}", event.chunk_pos);
            chunks.push(event.chunk_pos);
            // if let Some(snapshot) = dim_query.single().snapshots.get(&event.chunk_pos) {
            //     snapshots.push(snapshot.clone_value());
            //     println!("Loading chunk from cache {:?}.", event.chunk_pos);
            // }
        }
        let mut new_chunks = vec![];
        for chunk in chunks.iter() {
            if let Ok(reader) = world
                .resource::<AppBackend>()
                .reader(&format!("{}", chunk))
                .map_err(SaveableError::other)
            {
                print!(" LOADING CHUNK ");
                let loader = world.resource::<AppLoader>();
                let deser = world.deserialize_applier(&mut loader.deserializer(reader));
                if let Err(e) = deser {
                    new_chunks.push(chunk);
                    println!("{e}");
                } else {
                    // deser.unwrap().map(EntityMap::new())
                }
            } else {
                new_chunks.push(chunk);
            }
        }
        let mut state: SystemState<EventWriter<CreateChunkEvent>> = SystemState::new(world);
        for chunk_pos in new_chunks.iter() {
            println!("          NO LOAD {chunk_pos:?}");
            state.get_mut(world).send(CreateChunkEvent {
                chunk_pos: **chunk_pos,
            });
        }
        // for snapshot in snapshots.iter() {
        //     snapshot
        //         .applier(world)
        //         .despawn(DespawnMode::None)
        //         .mapping(MappingMode::Strict)
        //         .apply()
        //         .expect("Failed to Load snapshot.");
        // }
    }

    // fn handle_add_visuals_to_loaded_objects(
    //     game: GameParam,
    //     mut commands: Commands,
    //     mut meshes: ResMut<Assets<Mesh>>,
    //     loaded_entities: Query<(Entity, &WorldObject), Added<WorldObject>>,
    // ) {

    //     // let foliage_material = &game
    //     //     .graphics
    //     //     .foliage_material_map
    //     //     .as_ref()
    //     //     .unwrap()
    //     //     .get(&Foliage::Tree)
    //     //     .unwrap();

    //     // for (e, obj) in loaded_entities.iter() {
    //     //     match obj {
    //     //          if let Some(foliage) = proto_param.get_component::<Foliage, _>(obj) {
    //     //             commands
    //     //                 .entity(e)
    //     //                 .insert(Mesh2dHandle::from(meshes.add(Mesh::from(shape::Quad {
    //     //                     size: Vec2::new(32., 40.),
    //     //                     ..Default::default()
    //     //                 }))))
    //     //                 .insert((*foliage_material).clone());
    //     //         }
    //     //          if let Some(wall) = proto_param.get_component::<Wall, _>(obj) {
    //     //             println!("ADDING WALL VISUALS");
    //     //             commands
    //     //                 .entity(e)
    //     //                 .insert(game.graphics.wall_texture_atlas.as_ref().unwrap().clone())
    //     //                 .insert(TextureAtlasSprite::default());
    //     //         }
    //     //         _ => {}
    //     //     }
    //     // }
    // }
    //TODO: make this work with Spawn events too ,change event name
    fn save_chunk(
        world: &mut World,
        mut local: Local<
            SystemState<(
                Res<ChunkManager>,
                EventReader<DespawnChunkEvent>,
                Query<&Children>,
            )>,
        >,
    ) {
        let (chunk_manager, mut save_events, children) = local.get_mut(world);
        let mut saved_chunks = HashMap::default();
        for saves in save_events.iter() {
            let chunk_e = *chunk_manager.chunks.get(&saves.chunk_pos.into()).unwrap();

            let mut entities = children.iter_descendants(chunk_e).collect::<Vec<_>>();
            entities.push(chunk_e);
            saved_chunks.insert(saves.chunk_pos, entities);
        }

        for (chunk_pos, entities) in saved_chunks.iter() {
            let snapshot = Snapshot::builder(world)
                .extract_entities(entities.clone().into_iter())
                .build();
            if let Ok(writer) = world
                .resource::<AppBackend>()
                .writer(&format!("{}", chunk_pos))
                .map_err(SaveableError::other)
            {
                let saver = world.resource::<AppSaver>();
                if let Err(e) = saver.serialize(
                    &SnapshotSerializer::new(&snapshot, world.resource::<AppTypeRegistry>()),
                    writer,
                ) {
                    println!("{e}")
                };
            }
            // snapshots.insert(*chunk_pos, snapshot);
        }
        let mut state: SystemState<(GameParam, Commands)> = SystemState::new(world);
        for (chunk_pos, _) in saved_chunks.iter() {
            let (game, mut commands) = state.get_mut(world);
            commands
                .entity(game.get_chunk_entity(*chunk_pos).unwrap())
                .despawn_recursive();
        }

        // let (mut game, mut commands, mut dim_query) = state.get_mut(world);

        // for (chunk_pos, snapshot) in snapshots.iter() {
        //     // println!("Inserting new snapshot for {chunk_pos:?} and despawning it");
        //     dim_query
        //         .single_mut()
        //         .snapshots
        //         .insert(*chunk_pos, snapshot.clone_value());
        //     commands
        //         .entity(*game.get_chunk_entity(*chunk_pos).unwrap())
        //         .despawn_recursive();
        //     game.remove_chunk_entity(*chunk_pos);
        // }
    }
    fn despawn_saved_chunks(
        mut commands: Commands,
        game: GameParam,
        mut events: EventReader<DespawnChunkEvent>,
    ) {
        for event in events.iter() {
            print!("DESPAWNING {:?} ", event.chunk_pos);
            commands
                .entity(game.get_chunk_entity(event.chunk_pos).unwrap())
                .despawn_recursive();
            // game.remove_chunk_entity(event.chunk_pos);
            println!(" ... Done");
        }
    }
    pub fn despawn_non_saveable_entities(
        _commands: Commands,
        _minimap: Query<Entity, With<Minimap>>,
        key_input: ResMut<Input<KeyCode>>,
    ) {
        if key_input.just_pressed(KeyCode::Escape) {
            // println!("DESPAWNED MAP");
            // let map = minimap.single();
            // commands.entity(map).despawn_recursive();
        }
    }
    pub fn close_and_save_on_esc(world: &mut World) {
        let input = world.resource::<Input<KeyCode>>();
        if input.just_pressed(KeyCode::Escape) {
            // const PATH: &str = "example2.json";

            // let file = File::create(PATH).expect("Could not open file for serialization");

            // let mut ser = serde_json::Serializer::pretty(file);

            // world
            //     .serialize(&mut ser)
            //     .expect("Could not serialize World");
            world.save("game").expect("Failed to save");
            let mut state: SystemState<EventWriter<AppExit>> = SystemState::new(world);
            let mut exit = state.get_mut(world);
            exit.send(AppExit);
        }
    }
    pub fn load_on_start(world: &mut World) {
        println!("TRYING TO LOAD GAME");
        // world.load("game").map_err(|c| {
        // println!("{c:?}");
        let mut rng = rand::thread_rng();

        let seed = rng.gen_range(0..100000);
        let params = WorldGeneration {
            sand_frequency: 0.32,
            water_frequency: 0.15,
            obj_allowed_tiles_map: HashMap::default(),
            ..default()
        };
        world.insert_resource(GenerationSeed { seed });
        let mut state: SystemState<EventWriter<DimensionSpawnEvent>> = SystemState::new(world);
        let mut dim_event = state.get_mut(world);
        dim_event.send(DimensionSpawnEvent {
            generation_params: params,
            swap_to_dim_now: true,
        });
        // });
        println!("DONE LOADING");
    }
}
