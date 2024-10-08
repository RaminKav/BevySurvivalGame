pub use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use serde::{Deserialize, Serialize};

use crate::{container::Container, item::CraftingTracker};

use super::UIState;

#[derive(Resource, Default, Debug, Clone)]
pub struct CraftingContainer {
    pub items: Container,
}

#[derive(
    Default,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Reflect,
    FromReflect,
    TypeUuid,
)]
#[uuid = "413be529-bfeb-41d3-9db0-4b8b380a2c36"]

pub enum CraftingContainerType {
    #[default]
    Inventory,
    CraftingTable,
    Anvil,
    Cauldron,
    AlchemyTable,
}

pub fn change_ui_state_to_crafting_when_resource_added(
    mut inv_ui_state: ResMut<NextState<UIState>>,
    mut tracker: ResMut<CraftingTracker>,
) {
    if !tracker
        .discovered_crafting_types
        .contains(&CraftingContainerType::CraftingTable)
    {
        tracker
            .discovered_crafting_types
            .push(CraftingContainerType::CraftingTable);
    }
    inv_ui_state.set(UIState::Crafting);
}
