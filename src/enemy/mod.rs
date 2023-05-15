use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{AsBindGroup, ShaderRef},
    sprite::{Material2d, Material2dPlugin, MaterialMesh2dBundle},
};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController};
use seldom_state::prelude::{StateMachine, Trigger};
use serde::Deserialize;
use strum_macros::Display;

use crate::{
    ai::{AttackDistance, AttackState, FollowState, IdleState, LineOfSight, MoveDirection},
    animations::{AnimationFrameTracker, AnimationTimer},
    attributes::Health,
    GameParam, YSort,
};

pub mod spawner;
use self::spawner::SpawnerPlugin;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<EnemyMaterial>::default())
            .add_plugin(SpawnerPlugin);
    }
}

#[derive(Component, Default, Deserialize, Debug, Clone, Hash, Display, Eq, PartialEq)]
pub enum Enemy {
    #[default]
    Slime,
}
impl Enemy {
    pub fn summon(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        asset_server: &AssetServer,
        materials: &mut Assets<EnemyMaterial>,
        pos: Vec2,
    ) {
        let name = self.to_string();
        let handle = asset_server.load(format!(
            "textures/{}/{}-move-0.png",
            name.to_lowercase(),
            name.to_lowercase()
        ));
        let enemy_material = materials.add(EnemyMaterial {
            source_texture: Some(handle),
            is_attacking: 0.,
        });
        let mut enemy = commands.spawn((
            MaterialMesh2dBundle {
                mesh: game
                    .meshes
                    .add(Mesh::from(shape::Quad {
                        size: Vec2::new(32., 32.),
                        ..Default::default()
                    }))
                    .into(),
                transform: Transform::from_translation(pos.extend(0.)),
                material: enemy_material,
                ..default()
            },
            AnimationTimer(Timer::from_seconds(0.20, TimerMode::Repeating)),
            AnimationFrameTracker(0, 7),
            Health(100),
            KinematicCharacterController::default(),
            Collider::cuboid(10., 6.),
            YSort,
            self.clone(),
            StateMachine::default()
                .trans::<IdleState>(
                    LineOfSight {
                        target: game.game.player,
                        range: 100.,
                    },
                    FollowState {
                        target: game.game.player,
                        speed: 0.7,
                    },
                )
                .trans::<FollowState>(
                    AttackDistance {
                        target: game.game.player,
                        range: 50.,
                    },
                    AttackState {
                        target: game.game.player,
                        attack_startup_timer: Timer::from_seconds(0.3, TimerMode::Once),
                        attack_duration_timer: Timer::from_seconds(0.3, TimerMode::Once),
                        attack_cooldown_timer: Timer::from_seconds(1., TimerMode::Once),
                        dir: None,
                        speed: 1.4,
                        damage: 10,
                    },
                )
                .trans::<FollowState>(
                    Trigger::not(LineOfSight {
                        target: game.game.player,
                        range: 100.,
                    }),
                    IdleState {
                        walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                        direction: MoveDirection::new_rand_dir(rand::thread_rng()),
                        speed: 0.5,
                    },
                )
                .trans::<AttackState>(
                    Trigger::not(AttackDistance {
                        target: game.game.player,
                        range: 50.,
                    }),
                    FollowState {
                        target: game.game.player,
                        speed: 0.7,
                    },
                ),
            IdleState {
                walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                direction: MoveDirection::new_rand_dir(rand::thread_rng()),
                speed: 0.5,
            },
            Name::new(name),
        ));
        if let Some(loot_table) = game.loot_tables.table.get(&self) {
            enemy.insert(loot_table.clone());
        }
    }
}

impl Material2d for EnemyMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/enemy_attack.wgsl".into()
    }
}

#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "a04064b6-dcdd-11ed-afa1-0242ac120002"]
pub struct EnemyMaterial {
    #[uniform(0)]
    pub is_attacking: f32,
    #[texture(1)]
    #[sampler(2)]
    pub source_texture: Option<Handle<Image>>,
}
