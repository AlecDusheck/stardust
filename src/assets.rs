//! Asset handles plus the RON level/campaign loaders.

use crate::AppState;
use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;
use stardust_level::{Campaign, Level, ParseError};

pub fn plugin(app: &mut App) {
    app.init_asset::<LevelAsset>()
        .init_asset::<CampaignAsset>()
        .init_asset_loader::<LevelLoader>()
        .init_asset_loader::<CampaignLoader>()
        .add_systems(Startup, start_loading)
        .add_systems(Update, finish_loading.run_if(in_state(AppState::Loading)));
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct LevelAsset(pub Level);

#[derive(Asset, TypePath, Debug, Clone)]
pub struct CampaignAsset(pub Campaign);

#[derive(Default, TypePath)]
struct LevelLoader;

#[derive(Default, TypePath)]
struct CampaignLoader;

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse(ParseError),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<ParseError> for LoadError {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}

async fn read_string(reader: &mut dyn Reader) -> Result<String, LoadError> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    String::from_utf8(bytes).map_err(|e| LoadError::Parse(ParseError::Ron(e.to_string())))
}

impl AssetLoader for LevelLoader {
    type Asset = LevelAsset;
    type Settings = ();
    type Error = LoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        (): &(),
        _: &mut LoadContext<'_>,
    ) -> Result<LevelAsset, LoadError> {
        Ok(LevelAsset(Level::from_ron(&read_string(reader).await?)?))
    }

    fn extensions(&self) -> &[&str] {
        &["level.ron"]
    }
}

impl AssetLoader for CampaignLoader {
    type Asset = CampaignAsset;
    type Settings = ();
    type Error = LoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        (): &(),
        _: &mut LoadContext<'_>,
    ) -> Result<CampaignAsset, LoadError> {
        Ok(CampaignAsset(Campaign::from_ron(
            &read_string(reader).await?,
        )?))
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

/// Everything the game needs, loaded up front.
#[derive(Resource, Default)]
pub struct GameAssets {
    pub tiles: [Handle<Image>; 2],
    pub heroes: [Handle<Image>; 2],
    pub tile_layout: Handle<TextureAtlasLayout>,
    pub logo: Handle<Image>,
    pub story: Handle<Image>,
    pub instructions: Handle<Image>,
    pub victory: Handle<Image>,
    pub companions: Handle<Image>,
    pub campaign: Handle<CampaignAsset>,
    pub levels: Vec<Handle<LevelAsset>>,
    pub sounds: Vec<(&'static str, Handle<AudioSource>)>,
}

impl GameAssets {
    pub fn sound(&self, name: &str) -> Option<Handle<AudioSource>> {
        self.sounds
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, h)| h.clone())
    }
}

pub const SOUNDS: &[&str] = &[
    "program_s_begun",
    "level_screen",
    "begin_playing",
    "land",
    "magic_blue",
    "magic_green",
    "magic_dud",
    "g_bye_block",
    "g_bye_greenwall",
    "g_bye_warppocket",
    "g_bye_elevator",
    "g_bye_fallwall",
    "death_by_falling",
    "death_by_warppocket",
    "death_by_the_coals",
    "first_entrance_portal",
    "other_entrance_portal",
    "end_of_level_portal",
    "victory",
    "password_no_good",
    "information",
    "warp",
];

fn start_loading(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.insert_resource(GameAssets {
        tiles: [
            server.load("art/tiles_a.png"),
            server.load("art/tiles_b.png"),
        ],
        heroes: [server.load("art/hero_a.png"), server.load("art/hero_b.png")],
        tile_layout: layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(40),
            6,
            11,
            None,
            None,
        )),
        logo: server.load("art/logo.png"),
        story: server.load("art/story.png"),
        instructions: server.load("art/instructions.png"),
        victory: server.load("art/victory.png"),
        companions: server.load("art/companions.png"),
        campaign: server.load("levels/campaign.ron"),
        levels: Vec::new(),
        sounds: SOUNDS
            .iter()
            .map(|name| (*name, server.load(format!("sfx/{name}.wav"))))
            .collect(),
    });
}

/// Once the campaign manifest is in, queue its levels; once those are in,
/// show the title.
fn finish_loading(
    mut assets: ResMut<GameAssets>,
    server: Res<AssetServer>,
    campaigns: Res<Assets<CampaignAsset>>,
    mut next: ResMut<NextState<AppState>>,
) {
    if assets.levels.is_empty() {
        if let Some(campaign) = campaigns.get(&assets.campaign) {
            assets.levels = campaign
                .0
                .levels
                .iter()
                .map(|file| server.load(format!("levels/{file}")))
                .collect();
        }
        return;
    }
    let mut images = assets.tiles.iter().chain(&assets.heroes).map(Handle::id);
    let all_loaded = images.all(|id| server.is_loaded_with_dependencies(id))
        && server.is_loaded_with_dependencies(assets.companions.id())
        && assets
            .levels
            .iter()
            .all(|h| server.is_loaded_with_dependencies(h.id()))
        && assets
            .sounds
            .iter()
            .all(|(_, h)| server.is_loaded_with_dependencies(h.id()));
    if all_loaded {
        next.set(AppState::Title);
    }
}
