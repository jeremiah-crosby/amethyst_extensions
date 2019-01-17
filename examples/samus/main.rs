extern crate amethyst;
#[macro_use]
extern crate serde;
extern crate amethyst_extensions;

use amethyst::{
    animation::*,
    assets::{AssetStorage, Loader, PrefabLoader, PrefabLoaderSystem, ProgressCounter, RonFormat},
    core::{Parent, Transform, TransformBundle},
    ecs::Entity,
    input::{get_key, is_close_requested, is_key_down},
    prelude::*,
    renderer::{
        Camera, ColorMask, DepthMode, DisplayConfig, DrawFlat2D, ElementState, Pipeline, PngFormat,
        PosNormTex, Projection, RenderBundle, SpriteRender, SpriteSheet, SpriteSheetFormat,
        SpriteSheetHandle, Stage, Texture, TextureMetadata, VirtualKeyCode, ALPHA,
    },
    utils::application_root_dir,
};
use amethyst_extensions::prefab::sprite::*;

fn init_camera(world: &mut World, parent: Entity) {
    let mut transform = Transform::default();
    transform.set_z(1.0);
    world
        .create_entity()
        .with(Camera::from(Projection::orthographic(
            -250.0, 250.0, -250.0, 250.0,
        )))
        .with(Parent { entity: parent })
        .with(transform)
        .build();
}

fn init_player(world: &mut World, progress: &mut ProgressCounter) -> Entity {
    let prefab_handle = world.exec(
        |loader: PrefabLoader<'_, AnimatedSpritePrefab<Transform>>| {
            loader.load("samus.ron", RonFormat, (), progress)
        },
    );
    world.create_entity().with(prefab_handle).build()
}

fn start_animation(
    world: &mut World,
    entity: Entity,
    id: u64,
    rate: f32,
    defer: Option<(u64, DeferStartRelation)>,
) {
    let existing_animation = world
        .read_storage::<AnimationSet<u64, SpriteRender>>()
        .get(entity)
        .and_then(|s| s.get(&id))
        .cloned()
        .unwrap();
    let mut sets = world.write_storage();
    let control_set = get_animation_set::<u64, SpriteRender>(&mut sets, entity).unwrap();
    control_set.add_animation(
        id,
        &existing_animation,
        EndControl::Loop(None),
        rate,
        AnimationCommand::Start,
    );
}

fn stop_animation(world: &mut World, entity: Entity, id: u64) {
    let mut sets = world.write_storage();
    let control_set = get_animation_set::<u64, SpriteRender>(&mut sets, entity).unwrap();
    control_set.abort(id);
}

struct Loading {
    progress_counter: ProgressCounter,
    player_entity: Option<Entity>,
}

impl Loading {
    fn new() -> Self {
        Loading {
            progress_counter: ProgressCounter::new(),
            player_entity: None,
        }
    }
}

impl SimpleState for Loading {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        let world = data.world;

        self.player_entity = Some(init_player(world, &mut self.progress_counter));
        init_camera(world, self.player_entity.as_ref().cloned().unwrap());
    }

    fn update(&mut self, _data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
        if self.progress_counter.is_complete() {
            Trans::Switch(Box::new(Example {
                player_entity: self.player_entity.unwrap(),
                current_animation: 1,
            }))
        } else {
            Trans::None
        }
    }
}

struct Example {
    player_entity: Entity,
    current_animation: u64,
}

impl SimpleState for Example {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        let world = data.world;
        start_animation(world, self.player_entity, 1, 1., None);
    }

    fn handle_event(
        &mut self,
        data: StateData<'_, GameData<'_, '_>>,
        event: StateEvent,
    ) -> SimpleTrans {
        let StateData { world, .. } = data;
        if let StateEvent::Window(event) = &event {
            if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
                return Trans::Quit;
            }
            match get_key(&event) {
                Some((VirtualKeyCode::T, ElementState::Pressed)) => {
                    stop_animation(world, self.player_entity, self.current_animation);
                    start_animation(world, self.player_entity, 1, 1., None);
                    self.current_animation = 1;
                }

                Some((VirtualKeyCode::R, ElementState::Pressed)) => {
                    stop_animation(world, self.player_entity, self.current_animation);
                    start_animation(world, self.player_entity, 2, 8., None);
                    self.current_animation = 2;
                }

                _ => {}
            };
        }
        Trans::None
    }
}

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let root = application_root_dir()?.join("examples/samus/resources");
    let config = DisplayConfig::load(root.join("display_config.ron"));
    let pipe = Pipeline::build().with_stage(
        Stage::with_backbuffer()
            .clear_target([0.1, 0.1, 0.1, 1.0], 1.0)
            .with_pass(DrawFlat2D::new().with_transparency(
                ColorMask::all(),
                ALPHA,
                Some(DepthMode::LessEqualWrite), // Tells the pipeline to respect sprite z-depth
            )),
    );

    let game_data = GameDataBuilder::default()
        .with(
            PrefabLoaderSystem::<AnimatedSpritePrefab<Transform>>::default(),
            "",
            &[],
        )
        .with_bundle(TransformBundle::new())?
        .with_bundle(AnimationBundle::<u64, SpriteRender>::new(
            "animation_control_system",
            "sampler_interpolation_system",
        ))?
        .with_bundle(
            RenderBundle::new(pipe, Some(config))
                .with_sprite_sheet_processor()
                .with_sprite_visibility_sorting(&[]), // Let's us use the `Transparent` component
        )?;

    let mut game = Application::build(root, Loading::new())?.build(game_data)?;
    game.run();
    Ok(())
}
