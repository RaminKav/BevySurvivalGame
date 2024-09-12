use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, Aseprite, AsepriteBundle};
use bevy_rapier2d::prelude::{ActiveCollisionTypes, ActiveEvents, Collider, Sensor};

use crate::{
    animations::DoneAnimation,
    attributes::{modifiers::ModifyHealthEvent, Attack, Lifesteal},
    collisions::PlayerAttackCollider,
    combat_helpers::{spawn_one_time_aseprite_collider, spawn_temp_collider},
    enemy::Mob,
    item::{
        projectile::{Projectile, ProjectileState},
        WorldObject,
    },
    status_effects::Frail,
    GameParam, HitEvent, InvincibilityTimer, WasHitWithCrit,
};

use super::{skills, sprint::SprintState, Player, PlayerSkills, Skill};
aseprite!(pub OnHitAoe, "textures/effects/OnHitAoe.aseprite");

#[derive(Component)]
pub struct SecondHitDelay {
    pub delay: Timer,
    pub dir: Vec2,
    pub weapon_obj: WorldObject,
}

pub fn handle_on_hit_skills(
    mut hit_events: EventReader<HitEvent>,
    mut commands: Commands,
    player_q: Query<(Entity, &Attack, &PlayerSkills), With<Player>>,
    asset_server: Res<AssetServer>,
    in_i_frame: Query<&InvincibilityTimer>,
) {
    let (player, attack, skills) = player_q.single();
    if !skills.has(Skill::OnHitAoEBurst) {
        return;
    }
    for hit in hit_events.iter() {
        if hit.hit_entity == player {
            // is in invincibility frames from a previous hit
            if in_i_frame.get(hit.hit_entity).is_ok() {
                continue;
            }
            let hitbox_e = spawn_one_time_aseprite_collider(
                &mut commands,
                Transform::from_translation(Vec3::ZERO),
                10.5,
                attack.0,
                Collider::capsule(Vec2::ZERO, Vec2::ZERO, 19.),
                asset_server.load::<Aseprite, _>(OnHitAoe::PATH),
                AsepriteAnimation::from(OnHitAoe::tags::AO_E),
            );

            commands.entity(hitbox_e).set_parent(player);
        }
    }
}

pub fn handle_second_split_attack(
    mobs: Query<Option<&Frail>, With<Mob>>,
    game: GameParam,
    lifesteal: Query<&Lifesteal>,
    mut second_hit_query: Query<(Entity, &mut SecondHitDelay)>,
    mut modify_health_events: EventWriter<ModifyHealthEvent>,
    mut hit_event: EventWriter<HitEvent>,
    time: Res<Time>,
    mut commands: Commands,
    player: Query<(&PlayerSkills, Option<&SprintState>)>,
) {
    for (e, mut second_hit) in second_hit_query.iter_mut() {
        if !second_hit.delay.tick(time.delta()).just_finished() {
            continue;
        }
        let Ok(frail_option) = mobs.get(e) else {
            continue;
        };
        let (skills, maybe_sprint) = player.single();

        let (damage, was_crit) = game.calculate_player_damage(
            (frail_option.map(|f| f.num_stacks).unwrap_or(0) * 5) as u32,
            if skills.has(Skill::SprintLungeDamage)
                && maybe_sprint.unwrap().lunge_duration.percent() != 0.
            {
                Some(1.25)
            } else {
                None
            },
            0,
            None,
        );
        let split_damage = f32::floor(damage as f32 / 2.) as i32;
        if let Ok(lifesteal) = lifesteal.get(game.game.player) {
            modify_health_events.send(ModifyHealthEvent(f32::floor(
                split_damage as f32 * lifesteal.0 as f32 / 100.,
            ) as i32));
        }
        info!("SPLIT 2 {} {:?}", split_damage, was_crit);

        hit_event.send(HitEvent {
            hit_entity: e,
            damage: split_damage,
            dir: second_hit.dir,
            hit_with_melee: Some(second_hit.weapon_obj),
            hit_with_projectile: None,
            was_crit,
            hit_by_mob: None,
            ignore_tool: false,
        });
        if was_crit {
            commands.entity(e).insert(WasHitWithCrit);
        }
        commands.entity(e).remove::<SecondHitDelay>();
    }
}