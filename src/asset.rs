pub mod font {
    use crate::config::GameConfig;
    use bevy::asset::{AssetServer, Handle};
    use bevy::prelude::{Font, Res};

    pub enum FontType {
        Default,
    }

    /// Register all font assets here.
    impl FontType {
        fn get_asset_path(&self, config: &GameConfig) -> String {
            match self {
                FontType::Default => config.assets.default_font.clone(),
            }
        }
    }

    pub fn load_assets(
        asset_server: &Res<AssetServer>,
        config: &GameConfig,
        font: FontType,
    ) -> Handle<Font> {
        asset_server.load(font.get_asset_path(config))
    }
}
