pub mod demo_level;

#[derive(Debug, Copy, Clone)]
pub enum SceneLayer {
    Card,
    PlayerCoin,
    Coin,
}

impl SceneLayer {
    pub fn get_layer_base_z(&self) -> f32 {
        (match self {
            SceneLayer::Card => 10000,
            SceneLayer::PlayerCoin => 30000,
            SceneLayer::Coin => 30100,
        } as f32)
    }
}
