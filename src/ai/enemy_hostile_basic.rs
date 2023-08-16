use bevy::prelude::*;
use bevy_rapier2d::prelude::KinematicCharacterController;
use rand::{rngs::ThreadRng, Rng};
use seldom_state::prelude::*;

use crate::{
    animations::enemy_sprites::{CharacterAnimationSpriteSheetData, EnemyAnimationState},
    combat::HitEvent,
    inputs::FacingDirection,
    item::projectile::{Projectile, RangedAttackEvent},
    Game,
};

// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct LineOfSight {
    pub target: Entity,
    pub range: f32,
}
#[derive(Component)]
pub struct EnemyAttackCooldown(Timer);

impl Trigger for LineOfSight {
    type Param<'w, 's> = (Query<'w, 's, &'static Transform>, Res<'w, Time>);
    type Ok = f32;
    type Err = f32;

    // Return `Ok` to trigger and `Err` to not trigger
    fn trigger(
        &self,
        entity: Entity,
        (transforms, _time): Self::Param<'_, '_>,
    ) -> Result<f32, f32> {
        if let Ok(tfxm) = transforms.get(entity) {
            let delta = transforms.get(self.target).unwrap().translation.truncate()
                - tfxm.translation.truncate();

            let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
            return (distance <= self.range).then_some(distance).ok_or(distance);
        } else {
            Err(0.)
        }
    }
}
// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct HurtByPlayer;

impl BoolTrigger for HurtByPlayer {
    type Param<'w, 's> = EventReader<'w, 's, HitEvent>;

    fn trigger(&self, entity: Entity, mut hit_events: Self::Param<'_, '_>) -> bool {
        for hit in hit_events.iter() {
            if hit.hit_entity == entity {
                return true;
            }
        }
        return false;
    }
}
// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct AttackDistance {
    pub target: Entity,
    pub range: f32,
}

impl Trigger for AttackDistance {
    type Param<'w, 's> = (
        Query<'w, 's, (&'static Transform, Option<&'static EnemyAttackCooldown>)>,
        Res<'w, Time>,
    );
    type Ok = f32;
    type Err = f32;

    // Return `Ok` to trigger and `Err` to not trigger
    fn trigger(
        &self,
        entity: Entity,
        (transforms, _time): Self::Param<'_, '_>,
    ) -> Result<f32, f32> {
        if let Some(_) = transforms.get(entity).unwrap().1 {
            return Err(0.);
        }
        let delta = transforms
            .get(self.target)
            .unwrap()
            .0
            .translation
            .truncate()
            - transforms.get(entity).unwrap().0.translation.truncate();

        let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
        (distance <= self.range).then_some(distance).ok_or(distance)
    }
}

// Entities in the `Idle` state should walk in a given direction,
// then change direction after a set timer
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct IdleState {
    pub walk_timer: Timer,
    pub direction: FacingDirection,
    pub speed: f32,
}

// Entities in the `Follow` state should move towards the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct FollowState {
    pub target: Entity,
    pub speed: f32,
}
// Entities in the `Attack` state should move towards the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct LeapAttackState {
    pub target: Entity,
    pub attack_startup_timer: Timer,
    pub attack_duration_timer: Timer,
    pub attack_cooldown_timer: Timer,
    pub speed: f32,
    pub dir: Option<Vec2>,
}

// Entities in the `Attack` state should move towards the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct ProjectileAttackState {
    pub target: Entity,
    pub attack_startup_timer: Timer,
    pub attack_cooldown_timer: Timer,
    pub dir: Option<Vec2>,
    pub projectile: Projectile,
}

pub fn follow(
    mut transforms: Query<&mut Transform>,
    mut mover: Query<&mut KinematicCharacterController>,
    follows: Query<(
        Entity,
        &FollowState,
        &TextureAtlasSprite,
        &CharacterAnimationSpriteSheetData,
        &EnemyAnimationState,
    )>,
    mut commands: Commands,
) {
    for (entity, follow, sprite, anim_data, anim_state) in &follows {
        // Get the positions of the follower and target
        let target_translation = transforms.get(follow.target).unwrap().translation;
        let follow_transform = &mut transforms.get_mut(entity).unwrap();
        let follow_translation = follow_transform.translation;
        let delta = (target_translation - follow_translation)
            .normalize_or_zero()
            .truncate();
        // Find the direction from the follower to the target and go that way
        mover.get_mut(entity).unwrap().translation = Some(delta * follow.speed);
        commands
            .entity(entity)
            .insert(FacingDirection::from_translation(delta));
        if sprite.index == anim_data.get_starting_frame_for_animation(anim_state)
            && anim_state != &EnemyAnimationState::Hit
        {
            commands.entity(entity).insert(EnemyAnimationState::Walk);
        }
    }
}

pub fn leap_attack(
    mut transforms: Query<&mut Transform>,
    mut attacks: Query<(
        Entity,
        &mut KinematicCharacterController,
        &mut LeapAttackState,
        &mut TextureAtlasSprite,
        &CharacterAnimationSpriteSheetData,
        &EnemyAnimationState,
    )>,
    mut commands: Commands,
    time: Res<Time>,
    game: Res<Game>,
) {
    for (entity, mut kcc, mut attack, sprite, anim_data, anim_state) in attacks.iter_mut() {
        // Get the positions of the attacker and target
        let target_translation = transforms.get(attack.target).unwrap().translation;
        let attack_transform = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation;

        let hit = false;
        if attack.attack_startup_timer.finished() && !attack.attack_duration_timer.finished() {
            let delta = target_translation - attack_translation;
            if attack.dir.is_none() {
                attack.dir = Some(delta.normalize_or_zero().truncate() * attack.speed);
            }
            kcc.translation = Some(attack.dir.unwrap());
            attack.attack_duration_timer.tick(time.delta());
            if anim_state != &EnemyAnimationState::Attack {
                commands.entity(entity).insert(EnemyAnimationState::Attack);
            }
        }

        if hit || attack.attack_duration_timer.finished() {
            //start attack cooldown timer
            attack.dir = None;
            if anim_data.is_done_current_animation(sprite.index) {
                commands
                    .entity(entity)
                    .insert(EnemyAnimationState::Walk)
                    .insert(FollowState {
                        target: attack.target,
                        speed: 1.,
                    })
                    .remove::<LeapAttackState>()
                    .insert(EnemyAttackCooldown(attack.attack_cooldown_timer.clone()));
            }
        } else {
            attack.attack_startup_timer.tick(time.delta());
        }
    }
}
pub fn projectile_attack(
    mut commands: Commands,
    mut transforms: Query<&mut Transform>,
    mut attacks: Query<(Entity, &mut ProjectileAttackState, &EnemyAnimationState)>,
    mut events: EventWriter<RangedAttackEvent>,
    time: Res<Time>,
) {
    for (entity, mut attack, anim_state) in attacks.iter_mut() {
        // Get the positions of the attacker and target
        let target_translation = transforms.get(attack.target).unwrap().translation;
        let attack_transform = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation;
        if anim_state != &EnemyAnimationState::Attack {
            commands.entity(entity).insert(EnemyAnimationState::Attack);
        }
        if attack.attack_startup_timer.finished() && attack.attack_cooldown_timer.percent() == 0. {
            let delta = target_translation - attack_translation;
            if attack.dir.is_none() {
                attack.dir = Some(delta.normalize_or_zero().truncate());
            }

            events.send(RangedAttackEvent {
                projectile: attack.projectile.clone(),
                direction: attack.dir.unwrap(),
                from_enemy: Some(entity),
            });
            commands
                .entity(entity)
                .insert(EnemyAnimationState::Walk)
                .insert(FollowState {
                    target: attack.target,
                    speed: 1.,
                })
                .remove::<ProjectileAttackState>()
                .insert(EnemyAttackCooldown(attack.attack_cooldown_timer.clone()));
        }

        attack.dir = None;
        attack.attack_startup_timer.tick(time.delta());
    }
}
pub fn idle(
    mut transforms: Query<&mut KinematicCharacterController>,
    mut idles: Query<(Entity, &mut IdleState)>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (entity, mut idle) in idles.iter_mut() {
        // Get the positions of the follower and target
        idle.walk_timer.tick(time.delta());
        let mut idle_transform = transforms.get_mut(entity).unwrap();

        let s = idle.speed; //* time.delta_seconds();
        match idle.direction {
            FacingDirection::Left => idle_transform.translation = Some(Vec2::new(-s, 0.)),
            FacingDirection::Right => idle_transform.translation = Some(Vec2::new(s, 0.)),
            FacingDirection::Up => idle_transform.translation = Some(Vec2::new(0., s)),
            FacingDirection::Down => idle_transform.translation = Some(Vec2::new(0., -s)),
        }

        if idle.walk_timer.just_finished() {
            let new_dir = idle.direction.get_next_rand_dir(rand::thread_rng()).clone();
            idle.direction = new_dir.clone();
            commands
                .entity(entity)
                .insert(new_dir)
                .insert(EnemyAnimationState::Walk);
        }
    }
}
pub fn tick_enemy_attack_cooldowns(
    mut commands: Commands,
    mut attacks: Query<(Entity, &mut EnemyAttackCooldown)>,
    time: Res<Time>,
) {
    for (e, mut attack) in attacks.iter_mut() {
        attack.0.tick(time.delta());
        if attack.0.finished() {
            commands.entity(e).remove::<EnemyAttackCooldown>();
        }
    }
}
