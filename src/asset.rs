use std::path::PathBuf;

pub fn runtime_root() -> PathBuf {
    let exe_root = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()));

    if let Some(root) = exe_root
        && root.join("assets").exists()
    {
        return root;
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

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
