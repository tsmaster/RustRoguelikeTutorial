// src/main.rs

use app::app;
use chargrid_graphical::{Config, Context, Dimensions, FontBytes};
use coord_2d::Size;
use meap;
use rand::Rng;

use crate::visibility::VisibilityAlgorithm;

mod app;
mod behavior;
mod game;
mod terrain;
mod ui;
mod visibility;
mod world;

fn main() {
    use meap::Parser;
    let Args {
        rng_seed,
        visibility_algorithm,
    } = Args::parser().with_help_default().parse_env_or_exit();
    println!("RNG Seed: {}", rng_seed);
    
    const CELL_SIZE_PX: f64 = 24.0;
    let context = Context::new(Config {
        font_bytes: FontBytes {
            normal: include_bytes!("./fonts/PxPlus_IBM_CGAthin.ttf").to_vec(),
            bold: include_bytes!("./fonts/PxPlus_IBM_CGA.ttf").to_vec(),
        },
        title: "Chargrid Tutorial".to_string(),
        window_dimensions_px: Dimensions {
            width: 960.0,
            height: 720.0,
        },
        cell_dimensions_px: Dimensions {
            width: CELL_SIZE_PX,
            height: CELL_SIZE_PX,
        },
        font_scale: Dimensions {
            width: CELL_SIZE_PX,
            height: CELL_SIZE_PX,
        },
        underline_width_cell_ratio: 0.1,
        underline_top_offset_cell_ratio: 0.8,
        resizable: false,
    });
    let screen_size = Size::new(40, 30);
    let app = app(screen_size, rng_seed, visibility_algorithm);
    context.run_app(app);        
}

struct Args {
    rng_seed: u64,
    visibility_algorithm: VisibilityAlgorithm,
}

impl Args {
    fn parser() -> impl meap::Parser<Item = Self> {
        meap::let_map! {
            let {
                rng_seed = opt_opt::<u64, _>("INT", "r")
                    .name("rng-seed")
                    .desc("seed for random number generator")
                    .with_default_lazy("randomly chosen seed", || rand::thread_rng().gen());
                visibility_algorithm = flag("debug-omniscient").some_if(VisibilityAlgorithm::Omniscient)
                    .with_default_general(VisibilityAlgorithm::Shadowcast);
            } in {
                Self { rng_seed, visibility_algorithm }
            }
        }
    }
}

