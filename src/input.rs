use bevy::prelude::*;
use std::collections::HashSet;

#[derive(Resource, Default)]
pub struct GameplayInputBlocker {
    sources: HashSet<&'static str>,
}

impl GameplayInputBlocker {
    pub fn block(&mut self, source: &'static str) {
        self.sources.insert(source);
    }

    pub fn unblock(&mut self, source: &'static str) {
        self.sources.remove(source);
    }

    pub fn is_blocked(&self) -> bool {
        !self.sources.is_empty()
    }
}

pub fn player_input_allowed(blocker: Res<GameplayInputBlocker>) -> bool {
    !blocker.is_blocked()
}
