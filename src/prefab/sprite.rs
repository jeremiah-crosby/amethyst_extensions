use amethyst::animation::{
    Animation, AnimationSet, InterpolationFunction, Sampler, SpriteRenderChannel,
    SpriteRenderPrimitive,
};
use amethyst::assets::{
    AssetStorage, Handle, Loader, PrefabData, PrefabError, ProgressCounter, Source,
};
use amethyst::core::specs::prelude::Read;
use amethyst::core::Transform;
use amethyst::ecs::{Entity, ReadExpect, WriteStorage};
use amethyst::renderer::{
    PngFormat, Sprite, SpriteRender, SpriteSheet, SpriteSheetFormat, SpriteSheetHandle, Texture,
    TextureMetadata,
};

/// Structure acting as scaffolding for serde when loading a spritesheet file.
/// Positions originate in the top-left corner (bitmap image convention).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpritePosition {
    /// Horizontal position of the sprite in the sprite sheet
    pub x: u32,
    /// Vertical position of the sprite in the sprite sheet
    pub y: u32,
    /// Width of the sprite
    pub width: u32,
    /// Height of the sprite
    pub height: u32,
    /// Number of pixels to shift the sprite to the left and down relative to the entity holding it
    pub offsets: Option<[f32; 2]>,
}

/// Structure acting as scaffolding for serde when loading a spritesheet file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SerializedSpriteSheet {
    /// Width of the sprite sheet
    pub spritesheet_width: u32,
    /// Height of the sprite sheet
    pub spritesheet_height: u32,
    /// Description of the sprites
    pub sprites: Vec<SpritePosition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpriteAnimationData {
    pub id: u64,
    pub frame_count: u16,
    pub indices: Vec<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimatedSpritePrefab {
    pub id: u64,
    pub spritesheet_png_path: String,
    pub sprite_positions: SerializedSpriteSheet,
    pub animations: Vec<SpriteAnimationData>,

    #[serde(skip, default = "default_spritesheet_handle")]
    spritesheet_handle: Option<SpriteSheetHandle>,

    #[serde(skip, default = "default_animation_handle")]
    animation_handles: Vec<(u64, Handle<Animation<SpriteRender>>)>,
}

fn default_spritesheet_handle() -> Option<SpriteSheetHandle> {
    None
}

fn default_animation_handle() -> Vec<(u64, Handle<Animation<SpriteRender>>)> {
    Vec::new()
}

impl AnimatedSpritePrefab {
    fn load_sprite_sheet(
        &mut self,
        loader: &Loader,
        sprite_sheet_store: &AssetStorage<SpriteSheet>,
        texture_store: &AssetStorage<Texture>,
    ) {
        let texture_handle = {
            loader.load(
                self.spritesheet_png_path.as_str(),
                PngFormat,
                TextureMetadata::srgb_scale(),
                (),
                texture_store,
            )
        };

        let mut sprites: Vec<Sprite> = Vec::with_capacity(self.sprite_positions.sprites.len());
        for sp in &self.sprite_positions.sprites {
            let sprite = Sprite::from_pixel_values(
                self.sprite_positions.spritesheet_width as u32,
                self.sprite_positions.spritesheet_height as u32,
                sp.width as u32,
                sp.height as u32,
                sp.x as u32,
                sp.y as u32,
                sp.offsets.unwrap_or([0.0; 2]),
            );
            sprites.push(sprite);
        }
        let sheet = SpriteSheet {
            texture: texture_handle,
            sprites,
        };
        self.spritesheet_handle = Some(loader.load_from_data(sheet, (), sprite_sheet_store));
    }

    fn load_animations(
        &mut self,
        loader: &Loader,
        sampler_store: &AssetStorage<Sampler<SpriteRenderPrimitive>>,
        animation_store: &AssetStorage<Animation<SpriteRender>>,
    ) {
        for animation_data in &self.animations {
            // Sampler
            let sampler = Sampler {
                input: (0..animation_data.frame_count + 1).map(f32::from).collect(),
                output: animation_data
                    .indices
                    .iter()
                    .map(|i| SpriteRenderPrimitive::SpriteIndex(i.clone()))
                    .collect(),
                function: InterpolationFunction::Step,
            };
            let sampler_handle = loader.load_from_data(sampler.clone(), (), sampler_store);

            // Animation
            let animation =
                Animation::new_single(0, SpriteRenderChannel::SpriteIndex, sampler_handle);
            let animation_handle = loader.load_from_data(animation, (), animation_store);
            self.animation_handles
                .push((animation_data.id, animation_handle));
        }
    }
}

impl<'a> PrefabData<'a> for AnimatedSpritePrefab {
    type SystemData = (
        ReadExpect<'a, Loader>,
        Read<'a, AssetStorage<SpriteSheet>>,
        Read<'a, AssetStorage<Texture>>,
        WriteStorage<'a, SpriteRender>,
        WriteStorage<'a, Transform>,
        Read<'a, AssetStorage<Sampler<SpriteRenderPrimitive>>>,
        Read<'a, AssetStorage<Animation<SpriteRender>>>,
        WriteStorage<'a, AnimationSet<u64, SpriteRender>>,
    );

    type Result = ();

    fn add_to_entity(
        &self,
        entity: Entity,
        (
            ref loader,
            ref sprite_sheet_store,
            ref texture_store,
            ref mut sprite_render_store,
            ref mut transform_store,
            ref sampler_store,
            ref animation_store,
            ref mut animation_set_store,
        ): &mut Self::SystemData,
        entities: &[Entity],
    ) -> Result<(), PrefabError> {
        let mut transform = Transform::default();
        transform.set_x(0.0);
        transform.set_y(0.0);
        transform_store.insert(entity, transform);

        let sprite = SpriteRender {
            sprite_sheet: self.spritesheet_handle.as_ref().cloned().unwrap(),
            sprite_number: 1,
        };
        sprite_render_store.insert(entity, sprite);

        for animation_handle in &self.animation_handles {
            let animation_set = animation_set_store
                .entry(entity)
                .unwrap()
                .or_insert_with(AnimationSet::default);
            animation_set.insert(animation_handle.0.clone(), animation_handle.1.clone());
        }
        Ok(())
    }

    fn load_sub_assets(
        &mut self,
        _progress: &mut ProgressCounter,
        (
            ref loader,
            ref sprite_sheet_store,
            ref texture_store,
            ref sprite_render_store,
            ref transform_store,
            ref sampler_store,
            ref animation_store,
            ref mut animation_set_store,
        ): &mut Self::SystemData,
    ) -> Result<bool, PrefabError> {
        self.load_sprite_sheet(loader, sprite_sheet_store, texture_store);
        self.load_animations(loader, sampler_store, animation_store);
        Ok(true)
    }
}