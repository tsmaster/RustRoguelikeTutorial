// game.rs

use coord_2d::{Coord, Size};
use direction::CardinalDirection;
use entity_table::{ComponentTable, Entity};
use rand::SeedableRng;
use rand_isaac::Isaac64Rng;
use serde::{Deserialize, Serialize};

use crate::behavior::{Agent, BehaviorContext, NpcAction};
use crate::visibility::{CellVisibility, VisibilityAlgorithm, VisibilityGrid};
use crate::world::{EquippedInventoryIndices, HitPoints, Inventory,
                   ItemType, ItemUsage, Location, NpcType, Populate,
                   ProjectileType, Tile, World};


pub struct EntityToRender {
    pub tile: Tile,
    pub location: Location,
    pub visibility: CellVisibility,
}

#[derive(Serialize, Deserialize)]
pub struct GameState {
    world: World,
    player_entity: Entity,
    shadowcast_context: shadowcast::Context<u8>,
    visibility_grid: VisibilityGrid,
    ai_state: ComponentTable<Agent>,
    behavior_context: BehaviorContext,
    message_log: Vec<LogMessage>,
    rng: Isaac64Rng,
    dungeon_level: u32,
}

impl GameState {
    pub fn new(
        screen_size: Size,
        rng_seed: u64,
        initial_visibility_algorithm: VisibilityAlgorithm,
    ) -> Self {
        let mut world = World::new(screen_size);
        let mut rng = Isaac64Rng::seed_from_u64(rng_seed);
        println!("RNG: {:?}", rng);
        let dungeon_level = 1;
        let Populate {
            player_entity,
            ai_state,
        } = world.populate(dungeon_level, &mut rng);
        let shadowcast_context = shadowcast::Context::default();
        let visibility_grid = VisibilityGrid::new(screen_size);
        let behavior_context = BehaviorContext::new(screen_size);
        let mut game_state = Self {
            world,
            player_entity,
            shadowcast_context,
            visibility_grid,
            ai_state,
            behavior_context,
            message_log: Vec::new(),
            rng,
            dungeon_level,
        };
        game_state.update_visibility(initial_visibility_algorithm);
        game_state
    }

    pub fn tick_animations(&mut self) {
        self.world.move_projectiles(&mut self.message_log)
    }

    fn has_animations(&self) -> bool {
        self.world.has_projectiles()
    }

    pub fn update_visibility(&mut self, visibility_algorithm: VisibilityAlgorithm) {
        let player_coord = self
            .world
            .spatial_table
            .coord_of(self.player_entity)
            .unwrap();
        self.visibility_grid
            .update(
                player_coord,
                &self.world,
                &mut self.shadowcast_context,
                visibility_algorithm,
            );
    }

    pub fn maybe_move_player(&mut self, direction: CardinalDirection) {
        if self.has_animations() {
            return;
        }
        self.world
            .maybe_move_character(
                self.player_entity,
                direction,
                &mut self.message_log,
                &mut self.rng);
        self.ai_turn();
    }

    pub fn maybe_player_get_item(&mut self) {
        if self.has_animations() {
            return;
        }
        if self
            .world
            .maybe_get_item(self.player_entity, &mut self.message_log)
            .is_ok()
        {
            self.ai_turn();
        }
    }

    pub fn maybe_player_use_item(&mut self, inventory_index: usize) -> Result<ItemUsage, ()> {
        if self.has_animations() {
            return Err(());
        }
        let result =
            self
              .world
              .maybe_use_item(self.player_entity, inventory_index, &mut self.message_log);
        if let Ok(usage) = result {
            match usage {
                ItemUsage::Immediate => self.ai_turn(),
                ItemUsage::Aim => (),
            }
        }
        result
    }

    pub fn maybe_player_use_item_aim(
        &mut self,
        inventory_index: usize,
        target: Coord,
    ) -> Result<(), ()> {
        self.world.maybe_use_item_aim(
            self.player_entity,
            inventory_index,
            target,
            &mut self.message_log,
        )
    }

    pub fn maybe_player_drop_item(&mut self, inventory_index: usize) -> Result<(), ()> {
        let result =
            self.world
              .maybe_drop_item(self.player_entity, inventory_index, &mut self.message_log);
        if result.is_ok() {
            self.ai_turn();
        }
        result
    }

    pub fn player_level_up_and_descend(&mut self, level_up: LevelUp) {
        assert!(self.is_player_on_stairs());
        self.world.level_up_character(self.player_entity, level_up);
        let player_data = self.world.remove_character(self.player_entity);
        self.world.clear();
        self.visibility_grid.clear();
        self.dungeon_level += 1;

        let Populate {
            player_entity,
            ai_state,
        } = self.world.populate(self.dungeon_level, &mut self.rng);

        self.world.replace_character(player_entity, player_data);
        self.player_entity = player_entity;
        self.ai_state = ai_state;
    }

    pub fn is_player_on_stairs(&self) -> bool {
        self.world.coord_contains_stairs(self.player_coord())
    }

    pub fn player_strength(&self) -> i32 {
        self.world
            .strength(self.player_entity)
            .expect("player missing strength")
    }

    pub fn player_dexterity(&self) -> i32 {
        self.world
            .dexterity(self.player_entity)
            .expect("player missing strength")
    }

    pub fn player_intelligence(&self) -> i32 {
        self.world
            .intelligence(self.player_entity)
            .expect("player missing strength")
    }

    pub fn player_inventory(&self) -> &Inventory {
        self.world
            .inventory(self.player_entity)
            .expect("player has no inventory")
    }

    pub fn dungeon_level(&self) -> u32 {
        self.dungeon_level
    }

    pub fn item_type(&self, entity: Entity) -> Option<ItemType> {
        self.world.item_type(entity)
    }

    pub fn size(&self) -> Size {
        self.world.size()
    }

    fn ai_turn(&mut self) {
        self.behavior_context
            .update(self.player_entity, &self.world);
        let dead_entities = self
            .ai_state
            .entities()
            .filter(|&entity| !self.world.is_living_character(entity))
            .collect::<Vec<_>>();
        for dead_entity in dead_entities {
            self.ai_state.remove(dead_entity);
        }
        for (entity, agent) in self.ai_state.iter_mut() {
            let npc_action = agent.act(
                entity,
                self.player_entity,
                &self.world,
                &mut self.behavior_context);
            match npc_action {
                NpcAction::Wait => (),
                NpcAction::Move(direction) => self.world.maybe_move_character(
                    entity,
                    direction,
                    &mut self.message_log,
                    &mut self.rng,
                ),
            }
        }
    }

    pub fn message_log(&self) -> &[LogMessage] {
        &self.message_log
    }

    pub fn entities_to_render<'a>(&'a self) -> impl 'a + Iterator<Item = EntityToRender> {
        let tile_component = &self.world.components.tile;
        let spatial_table = &self.world.spatial_table;
        let visibility_grid = &self.visibility_grid;
        tile_component.iter().filter_map(move |(entity, &tile)| {
            let &location = spatial_table.location_of(entity)?;
            let visibility = visibility_grid.cell_visibility(location.coord);
            Some(EntityToRender {
                tile,
                location,
                visibility,
            })
        })
    }

    pub fn wait_player(&mut self) {
        if self.has_animations() {
            return;
        }

        self.ai_turn();
    }

    pub fn is_player_alive(&self) -> bool {
        self.world.is_living_character(self.player_entity)
    }

    pub fn player_hit_points(&self) -> HitPoints {
        self.world
            .hit_points(self.player_entity)
            .expect("player has no hit points")
    }

    pub fn player_coord(&self) -> Coord {
        self.world
            .entity_coord(self.player_entity)
            .expect("player has no coord")
    }

    pub fn examine_cell(&self, coord: Coord) -> Option<ExamineCell> {
        match self.visibility_grid.cell_visibility(coord) {
            CellVisibility::Currently => self.world.examine_cell(coord),
            _ => None,
        }
    }

    pub fn player_equipped_inventory_indices(&self) -> EquippedInventoryIndices {
        self.world.equipped_inventory_indices(self.player_entity)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum LogMessage {
    PlayerAttacksNpc(NpcType),
    NpcAttacksPlayer(NpcType),
    PlayerKillsNpc(NpcType),
    NpcKillsPlayer(NpcType),
    PlayerGets(ItemType),
    PlayerInventoryIsFull,
    NoItemUnderPlayer,
    NoItemInInventorySlot,
    PlayerHeals,
    PlayerDrops(ItemType),
    NoSpaceToDropItem,
    PlayerLaunchesProjectile(ProjectileType),
    NpcDies(NpcType),
    NpcBecomesConfused(NpcType),
    NpcIsNoLongerConfused(NpcType),
    PlayerDodges(NpcType),
    NpcDodges(NpcType),
    PlayerEquips(ItemType),
}

#[derive(Clone, Copy, Debug)]
pub enum ExamineCell {
    Npc(NpcType),
    NpcCorpse(NpcType),
    Item(ItemType),
    Player,
}

#[derive(Clone, Copy, Debug)]
pub enum LevelUp {
    Strength,
    Dexterity,
    Intelligence,
    Health,
}
