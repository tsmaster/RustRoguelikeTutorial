// terrain.rs

use grid_2d::{Coord, Grid, Size};
use rand::{seq::IteratorRandom, seq::SliceRandom, Rng};

use crate::world::{ItemType, NpcType};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TerrainTile {
    Player,
    Floor,
    Wall,
    Npc(NpcType),
    Item(ItemType),
    Stairs,
}

pub fn generate_dungeon<R: Rng>(size: Size, level: u32, rng: &mut R) -> Grid<TerrainTile> {
    let mut grid = Grid::new_copy(size, None);
    let mut room_centers = Vec::new();

    const NPCS_PER_ROOM_DISTRIBUTION: &[usize] =
        &[0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 3, 3, 4];

    const ITEMS_PER_ROOM_DISTRIBUTION: &[usize] =
        &[0, 0, 1, 1, 1, 1, 1, 2, 2];

    let npc_probability_distribution = make_npc_probability_distribution(level);
    let item_probability_distribution = make_item_probability_distribution(level);

    // attempt to add a room a constant number of times
    const NUM_ATTEMPTS: usize = 100;
    for _ in 0..NUM_ATTEMPTS {
        let room = Room::choose(size, rng);

        if room.only_intersects_empty(&grid) {
            room.carve_out(&mut grid);

            let room_center = room.center();

            if room_centers.is_empty() {
                *grid.get_checked_mut(room_center) = Some(TerrainTile::Player);
            }

            room_centers.push(room_center);

            // add NPCs to the room
            let &num_npcs = NPCS_PER_ROOM_DISTRIBUTION.choose(rng).unwrap();
            room.place_npcs(num_npcs, &npc_probability_distribution, &mut grid, rng);

            // Add items to the room
            let &num_items = ITEMS_PER_ROOM_DISTRIBUTION.choose(rng).unwrap();
            room.place_items(num_items, &item_probability_distribution, &mut grid, rng);
        }
    }

    for window in room_centers.windows(2) {
        carve_corridor(window[0], window[1], &mut grid);
    }

    *grid.get_checked_mut(*room_centers.last().unwrap()) = Some(TerrainTile::Stairs);
    
    grid.map(|t| t.unwrap_or(TerrainTile::Wall))
}

fn carve_corridor(start: Coord, end: Coord, grid: &mut Grid<Option<TerrainTile>>) {
    for i in start.x.min(end.x)..=start.x.max(end.x) {
        let cell = grid.get_checked_mut(Coord { x:i, ..start });
        if *cell == None || *cell == Some(TerrainTile::Wall) {
            *cell = Some(TerrainTile::Floor);
        }
    }

    for i in start.y.min(end.y)..start.y.max(end.y) {
        let cell = grid.get_checked_mut(Coord { y:i, ..end });
        if *cell == None || *cell == Some(TerrainTile::Wall) {
            *cell = Some(TerrainTile::Floor);
        }
    }
}


struct Room {
    top_left: Coord,
    size: Size,
}

impl Room {
    fn choose<R: Rng>(bounds: Size, rng: &mut R) -> Self {
        let width = rng.gen_range(5..11);
        let height = rng.gen_range(5..9);
        let size = Size::new(width, height);
        let top_left_bounds = bounds - size;
        let left = rng.gen_range(0..top_left_bounds.width());
        let top = rng.gen_range(0..top_left_bounds.height());
        let top_left = Coord::new(left as i32, top as i32);
        Self { top_left, size}
    }

    fn center(&self) -> Coord {
        self.top_left + self.size.to_coord().unwrap() / 2
    }

    fn coords<'a>(&'a self) -> impl 'a + Iterator<Item = Coord> {
        self.size
            .coord_iter_row_major()
            .map(move |coord| self.top_left + coord)
    }

    fn only_intersects_empty(&self, grid: &Grid<Option<TerrainTile>>) -> bool {
        self.coords().all(|coord| grid.get_checked(coord).is_none())
    }

    // Updates `grid`, setting each cell overlapping this room to
    // `Some(TerrainTile::Floor)`.  The top and left sides of the room
    // are set to `Some(TerrainTile::Wall)` instead.  This prevents a
    // pair of rooms being placed immediately adjacent to one another.
    fn carve_out(&self, grid: &mut Grid<Option<TerrainTile>>) {
        for coord in self.coords() {
            let cell = grid.get_checked_mut(coord);
            if coord.x == self.top_left.x || coord.y == self.top_left.y {
                *cell = Some(TerrainTile::Wall);
            } else {
                *cell = Some(TerrainTile::Floor);
            }
        }
    }

    fn place_npcs<R: Rng>(
        &self,
        n: usize,
        probability_distribution: &[(NpcType, u32)],
        grid: &mut Grid<Option<TerrainTile>>,
        rng: &mut R
    ) {
        for coord in self
            .coords()
            .filter(|&coord| grid.get_checked(coord).unwrap() == TerrainTile::Floor)
            .choose_multiple(rng, n)
        {
            let &npc_type = choose_from_probability_distribution(probability_distribution, rng);
            *grid.get_checked_mut(coord) = Some(TerrainTile::Npc(npc_type));
        }
    }

    fn place_items<R: Rng>(
        &self,
        n: usize,
        probability_distribution: &[(ItemType, u32)],
        grid: &mut Grid<Option<TerrainTile>>,
        rng: &mut R,
    ) {
        for coord in self
            .coords()
            .filter(|&coord| grid.get_checked(coord).unwrap() == TerrainTile::Floor)
            .choose_multiple(rng, n)
        {
            let &item = choose_from_probability_distribution(probability_distribution, rng);
            *grid.get_checked_mut(coord) = Some(TerrainTile::Item(item));
        }
    }
    
}

fn choose_from_probability_distribution<'a, T, R: Rng>(
    probability_distribution: &'a [(T, u32)],
    rng: &mut R,
) -> &'a T {
    let sum = probability_distribution.iter().map(|(_, p)| p).sum::<u32>();
    let mut choice = rng.gen_range(0..sum);
    for (value, probability) in probability_distribution.iter() {
        if let Some(remaining_choice) = choice.checked_sub(*probability) {
            choice = remaining_choice;
        } else {
            return value;
        }
    }
    unreachable!()
}
                                   
fn make_npc_probability_distribution(level: u32) -> Vec<(NpcType, u32)> {
    use NpcType::*;
    vec![(Orc, 20), (Troll, level)]
}

fn make_item_probability_distribution(level: u32) -> Vec<(ItemType, u32)> {
    use ItemType::*;
    let item_chance = match level {
        0..=1 => 5,
        2..=3 => 10,
        _ => 20,
    };
    
    vec![
        (HealthPotion, 200),
        (FireballScroll,
         match level {
             0..=1 => 10,
             2..=4 => 50,
             _ => 100,
         },
        ),
        (ConfusionScroll,
         match level {
             0..=1 => 10,
             2..=4 => 30,
             _ => 50,
         },
        ),
        (Sword, item_chance),
        (Staff, item_chance),
        (Armor, item_chance),
        (Robe, item_chance),
    ]
}
