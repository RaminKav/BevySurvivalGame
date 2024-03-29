use super::SchematicType;
use crate::{
    world::{chunk::Chunk, world_helpers::tile_pos_to_world_pos, TileMapPosition, CHUNK_SIZE},
    GameParam,
};
use bevy_ecs_tilemap::tiles::TilePos;
use rand::Rng;

use bevy::prelude::*;

#[derive(Component, Debug, Clone, Reflect, Default)]
pub struct SchematicSpawner {
    schematic: SchematicType,
}

pub fn attempt_to_spawn_schematic_in_chunk(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    chunks: Query<(Entity, &Chunk, &SchematicSpawner)>,
) {
    for (e, chunk, schematic) in chunks.iter() {
        let mut rng = rand::thread_rng();
        let rng_x = rng.gen_range(0..CHUNK_SIZE);
        let rng_y = rng.gen_range(0..CHUNK_SIZE);
        let target_pos = tile_pos_to_world_pos(
            TileMapPosition::new(chunk.chunk_pos, TilePos::new(rng_x, rng_y)),
            true,
        );
        commands
            .spawn(DynamicSceneBundle {
                scene: asset_server.load(format!("scenes/{}.scn.ron", schematic.schematic)),
                transform: Transform::from_translation(target_pos.extend(0.)),
                ..default()
            })
            .insert(Name::new("Schematic"));
        commands.entity(e).remove::<SchematicSpawner>();
    }
}

pub fn give_chunks_schematic_spawners(
    mut commands: Commands,
    new_chunks: Query<(Entity, &Chunk), Added<Chunk>>,
    game: GameParam,
) {
    for (e, chunk) in new_chunks.iter() {
        if game.is_chunk_generated(chunk.chunk_pos) {
            continue;
        }
        let mut rng = rand::thread_rng();
        for (schematic, frequency) in game.world_generation_params.schematic_frequencies.iter() {
            if rng.gen_ratio((100. * frequency) as u32, 100) {
                commands.entity(e).insert(SchematicSpawner {
                    schematic: schematic.clone(),
                });
            }
        }
    }
}
