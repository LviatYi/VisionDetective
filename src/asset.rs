pub mod font {
    use bevy::asset::{AssetServer, Handle};
    use bevy::prelude::{Font, Res};

    pub enum FontType {
        Default,
    }

    /// Register all font assets here.
    impl FontType {
        fn get_asset_path(&self) -> String {
            match self {
                FontType::Default => "fonts/LXGWWenKai/LXGWWenKai-Regular.ttf".to_string(),
            }
        }
    }

    pub fn load_assets(asset_server: Res<AssetServer>, font: FontType) -> Handle<Font> {
        asset_server.load(font.get_asset_path())
    }
}