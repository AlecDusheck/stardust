//! Easter egg from the original: any key pressed on the title screen while
//! J, I and M are all held shows Shakespeare's Sonnet 27 (CODE 1 $50f6,
//! $51e8) and plays the "J-I-M" sound. Click, space, return or Q leaves.

use crate::AppState;
use crate::audio::PlaySound;
use crate::grid::Z_OVERLAY;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        trigger
            .after(crate::menu::title_keys)
            .run_if(in_state(AppState::Title)),
    )
    .add_systems(OnEnter(AppState::Sonnet), show)
    .add_systems(Update, leave.run_if(in_state(AppState::Sonnet)));
}

const SONNET_27: &str = "Weary with toil, I haste me to my bed,
The dear repose for limbs with travel tired;
But then begins a journey in my head,
To work my mind, when body\u{2019}s work\u{2019}s expired:
For then my thoughts, far from where I abide,
Intend a zealous pilgrimage to thee
And keep my drooping eyelids open wide,
Looking on darkness which the blind do see:
Save that my soul\u{2019}s imaginary sight
Presents thy shadow to my sightless view,
Which, like a jewel hung in ghastly night,
Makes black night beauteous and her old face new.
Lo!  thus, by day my limbs, by night my mind,
For thee and for myself no quiet find.";

fn trigger(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.get_just_pressed().next().is_some()
        && keys.all_pressed([KeyCode::KeyJ, KeyCode::KeyI, KeyCode::KeyM])
    {
        next.set(AppState::Sonnet);
    }
}

fn show(mut commands: Commands, mut sounds: MessageWriter<PlaySound>) {
    // Black 18pt text on a white screen, 14 lines 21px apart from y=103.
    commands.spawn((
        Sprite::from_color(Color::WHITE, Vec2::new(640.0, 480.0)),
        Transform::from_xyz(0.0, 0.0, Z_OVERLAY),
        DespawnOnExit(AppState::Sonnet),
    ));
    commands.spawn((
        Text2d::new(SONNET_27),
        TextFont::from_font_size(18.0),
        bevy::text::LineHeight::Px(21.0),
        TextColor(Color::BLACK),
        TextLayout::justify(Justify::Center),
        Transform::from_xyz(0.0, 240.0 - 103.0 - 21.0 * 7.0, Z_OVERLAY + 1.0),
        DespawnOnExit(AppState::Sonnet),
    ));
    sounds.write(PlaySound("j_i_m"));
}

fn leave(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter, KeyCode::KeyQ])
        || mouse.just_pressed(MouseButton::Left)
    {
        next.set(AppState::Title);
    }
}
