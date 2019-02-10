extern crate amethyst;
extern crate genmesh;
extern crate tiled;
//extern crate gfx;
#[macro_use]
extern crate derivative;
extern crate amethyst_extensions;

use amethyst::core::{Transform, TransformBundle};
use amethyst::prelude::*;
use amethyst::renderer::{
    Camera, DisplayConfig, Pipeline, PosTex, Projection, RenderBundle, Stage,
};
use amethyst::utils::application_root_dir;
use amethyst::winit::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};

use amethyst_extensions::tilemap::*;

pub struct PlayState;

impl SimpleState for PlayState {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        let world = data.world;
        world.register::<TilemapDimensions>();
        world.register::<TilesheetDimensions>();
        world.register::<TilemapTiles>();
        initialise_camera(world);
        initialise_tilemap(
            world,
            application_root_dir()
                .unwrap()
                .join("examples/tilemap/resources")
                .to_str()
                .unwrap(),
            "map.tmx",
        );
    }

    fn handle_event(
        &mut self,
        data: StateData<'_, GameData<'_, '_>>,
        event: StateEvent,
    ) -> SimpleTrans {
        match event {
            _ => Trans::None,
        }
    }
}

fn initialise_camera(world: &mut World) {
    let mut transform = Transform::default();
    transform.set_z(1.0);
    world
        .create_entity()
        .with(Camera::from(Projection::orthographic(
            0.0, 500.0, 0.0, 500.0,
        )))
        .with(transform)
        .build();
}

const BACKGROUND_COLOUR: [f32; 4] = [0.0, 0.0, 0.0, 0.0]; // black

fn run() -> Result<(), amethyst::Error> {
    amethyst::start_logger(Default::default());
    let root = application_root_dir()?.join("examples/tilemap/resources");
    let config = DisplayConfig::load(root.join("display_config.ron"));

    let pipe = {
        Pipeline::build().with_stage(
            Stage::with_backbuffer()
                .clear_target(BACKGROUND_COLOUR, 1.0)
                .with_pass(DrawTilemap::<PosTex>::new()),
        )
    };
    let game_data = GameDataBuilder::default()
        .with_bundle(TransformBundle::new())?
        .with_bundle(RenderBundle::new(pipe, Some(config)))?;
    let mut game = Application::build(root, PlayState)?.build(game_data)?;
    game.run();
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Failed to execute example: {}", e);
        ::std::process::exit(1);
    }
}
