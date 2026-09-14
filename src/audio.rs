//! One-shot sound effects, triggered by name.

use crate::assets::GameAssets;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_message::<PlaySound>().add_systems(
        Update,
        play_sounds
            .after(crate::play::Playback)
            .run_if(resource_exists::<GameAssets>),
    );
}

#[derive(Message)]
pub struct PlaySound(pub &'static str);

#[derive(Component)]
struct Effect;

fn play_sounds(
    mut commands: Commands,
    mut requests: MessageReader<PlaySound>,
    assets: Res<GameAssets>,
    active: Query<Option<&AudioSink>, With<Effect>>,
) {
    // CODE 1 $314: use the first free one of four asynchronous effect channels.
    let mut channels = active
        .iter()
        .filter(|sink| sink.is_none_or(|sink| !sink.empty()))
        .count();
    for PlaySound(name) in requests.read() {
        if let Some(source) = assets.sound(name) {
            let effect = is_effect(name);
            if effect && channels >= 4 {
                continue;
            }
            let mut entity = commands.spawn((AudioPlayer::new(source), PlaybackSettings::DESPAWN));
            if effect {
                entity.insert(Effect);
                channels += 1;
            }
        } else {
            warn!("no sound named {name}");
        }
    }
}

fn is_effect(name: &str) -> bool {
    matches!(
        name,
        "land"
            | "magic_blue"
            | "magic_green"
            | "magic_dud"
            | "g_bye_block"
            | "g_bye_greenwall"
            | "g_bye_warppocket"
            | "g_bye_elevator"
            | "g_bye_fallwall"
            | "death_by_warppocket"
            | "death_by_the_coals"
            | "first_entrance_portal"
            | "other_entrance_portal"
            | "end_of_level_portal"
            | "warp"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::{message::Messages, system::RunSystemOnce};

    #[test]
    fn four_effect_channels_skip_overflow_and_leave_ui_sound_independent() {
        let mut world = World::new();
        world.insert_resource(GameAssets {
            sounds: vec![
                ("magic_blue", Handle::default()),
                ("level_screen", Handle::default()),
            ],
            ..default()
        });
        let mut messages = Messages::default();
        for _ in 0..5 {
            messages.write(PlaySound("magic_blue"));
        }
        messages.write(PlaySound("level_screen"));
        world.insert_resource(messages);
        world.run_system_once(play_sounds).unwrap();
        assert_eq!(world.query::<&Effect>().iter(&world).count(), 4);
        assert_eq!(world.query::<&AudioPlayer>().iter(&world).count(), 5);
    }
}
