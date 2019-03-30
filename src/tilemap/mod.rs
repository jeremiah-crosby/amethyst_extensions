use amethyst::assets::Loader;
use amethyst::core::nalgebra::{Vector2, Vector3};
use amethyst::core::{GlobalTransform, Transform};
use amethyst::ecs::{Component, DenseVecStorage};
use amethyst::prelude::*;
use amethyst::renderer::PosTex;
use amethyst::renderer::{
    FilterMethod, Mesh, PngFormat, SamplerInfo, SurfaceType, TextureBuilder, TextureData,
    TextureHandle, TextureMetadata, WrapMode,
};
use genmesh::generators::{IndexedPolygon, Plane, SharedVertex};
use genmesh::{EmitTriangles, MapVertex, Quad, Triangulate, Vertices};

use std::fs::File;
use std::path::{Path, PathBuf};
use tiled::{parse, Layer};

use log::{debug, error};

pub use self::tilemap_pass::DrawTilemap;

mod tilemap_pass;

pub fn initialise_tilemap(world: &mut World, base_dir: &str, map_name: &str) {
    let mut path_buf = PathBuf::new();
    path_buf.push(base_dir);
    path_buf.push(map_name);
    debug!("Loading tilemap {}", path_buf.to_str().unwrap());
    use amethyst::assets::Handle;
    use amethyst::renderer::{Material, MaterialDefaults};

    let map_path = path_buf.as_path();
    let map_file = match File::open(map_path) {
        Err(e) => {
            error!("Error opening .tmx file: {}", e);
            return;
        }
        Ok(f) => f,
    };
    let map = match parse(map_file) {
        Err(e) => {
            error!("Error while parsing .tmx file: {}", e);
            return;
        }
        Ok(m) => m,
    };
    let (tileset, tileset_img) = match map.tilesets.get(0) {
        Some(tileset) => match tileset.images.get(0) {
            Some(img) => (tileset, img),
            None => return,
        },
        None => return,
    };
    let tileset_width = tileset_img.width as u32 / tileset.tile_width;
    let tileset_height = tileset_img.height as u32 / tileset.tile_height;
    let image_source = &tileset_img.source;

    let tilemap_dimensions = TilemapDimensions {
        width: map.width,
        height: map.height,
    };

    let tilesheet_dimensions = TilesheetDimensions {
        width: tileset_width,
        height: tileset_height,
    };

    let layers = &map.layers;
    for layer in layers {
        let mut tilemap_layer = TilemapLayer {
            name: String::from(layer.name.as_str()),
            layer: layer.clone(),
            tiles: generate_tile_data(&layer, tileset_width, tileset_height),
        };

        let half_width: f32 = ((map.width * map.tile_width) / 2) as f32;
        let half_height: f32 = ((map.height * map.tile_height) / 2) as f32;

        let (mesh, material, tilemap_texture) = {
            let tex_storage = world.read_resource();
            let loader = world.read_resource::<Loader>();

            let tilemap_texture = loader.load_from_data(
                TextureData::F32(
                    tilemap_layer.generate_tilemap_texture(&tilesheet_dimensions),
                    build_tilemap_texture_metadata(&tilemap_dimensions, &tilesheet_dimensions),
                ),
                (),
                &tex_storage,
            );

            let mesh: Handle<Mesh> = loader.load_from_data(
                generate_tilemap_plane(map.tile_width, map.width, map.height).into(),
                (),
                &world.read_resource(),
            );

            let mat_defaults = world.read_resource::<MaterialDefaults>();

            let mut tileset_path_buf = PathBuf::new();
            tileset_path_buf.push(map_path.parent().unwrap_or(Path::new("")).as_os_str());
            tileset_path_buf.push(image_source);
            let tilemap_material = Material {
                albedo: loader.load(
                    tileset_path_buf.to_str().unwrap(),
                    PngFormat,
                    TextureMetadata::srgb(),
                    (),
                    &tex_storage,
                ),
                ..mat_defaults.0.clone()
            };

            (mesh, tilemap_material, tilemap_texture)
        };

        let mut transform = Transform::default();
        transform.set_x(half_width);
        transform.set_y(half_height);
        transform.set_z(0.0);

        world
            .create_entity()
            .with(mesh)
            .with(material)
            .with(transform)
            .with(GlobalTransform::default())
            .with(tilemap_dimensions.clone())
            .with(tilesheet_dimensions.clone())
            .with(tilemap_layer)
            .build();
    }
}

fn build_tilemap_texture_metadata(
    tilemap_dimensions: &TilemapDimensions,
    tilesheet_dimensions: &TilesheetDimensions,
) -> TextureMetadata {
    use gfx::format::ChannelType;
    let sampler_info = SamplerInfo::new(FilterMethod::Scale, WrapMode::Clamp);
    let texture_width = tilemap_dimensions.width * tilesheet_dimensions.width;
    let texture_height = tilemap_dimensions.height * tilesheet_dimensions.height;
    TextureMetadata {
        sampler: sampler_info,
        mip_levels: 1,
        dynamic: false,
        format: SurfaceType::D32,
        size: Some((texture_width as u16, texture_height as u16)),
        channel: ChannelType::Float,
    }
}

// TODO: Remove
pub fn generate_tilemap_plane(
    tilesize: u32,
    tilemap_width: u32,
    tilemap_height: u32,
) -> Vec<PosTex> {
    let plane = Plane::new();

    let half_width = (tilesize * tilemap_width) as f32 / 2.0;
    let half_height = (tilesize * tilemap_height) as f32 / 2.0;

    let vertex_data: Vec<PosTex> = plane
        .shared_vertex_iter()
        .map(|old_v| {
            let vertex_x = (half_width * old_v.pos.x).round();
            let vertex_y = (half_height * old_v.pos.y).round();

            let u_pos = (1.0 + old_v.pos.x) / 2.0;
            let v_pos = (1.0 + old_v.pos.y) / 2.0;

            let tilemap_x = (u_pos * tilemap_width as f32).round();
            let tilemap_y = (v_pos * tilemap_height as f32).round();

            PosTex {
                position: Vector3::new(vertex_x, vertex_y, 0.0),
                tex_coord: Vector2::new(tilemap_x, tilemap_height as f32 - tilemap_y),
            }
        })
        .collect();

    let indexed_vertex_data: Vec<PosTex> = plane
        .indexed_polygon_iter()
        .triangulate()
        .vertices()
        .map(|i| {
            *vertex_data.get(i as usize).unwrap_or(&PosTex {
                position: Vector3::new(0., 0., 0.),
                tex_coord: Vector2::new(0., 0.),
            })
        })
        .collect();

    indexed_vertex_data
}

pub fn generate_tile_data(
    layer: &tiled::Layer,
    tileset_width: u32,
    tileset_height: u32,
) -> Vec<[f32; 4]> {
    let mut tiles = Vec::new();
    for rows in &layer.tiles {
        for tile in rows {
            if *tile != 0 {
                // subtract 1.0 from the x coordinate because the first gid of the tileset is 1
                // this could be made cleaner
                tiles.push([
                    (*tile - 1) as f32 % tileset_width as f32,
                    (tileset_height - 1) as f32 - (((*tile - 1) / tileset_width) as f32),
                    0.0,
                    0.0,
                ]);
            } else {
                // There's no tile, so push a really big invalid row index so the shader will render transparency.
                tiles.push([100000.0, 0.0, 0.0, 0.0]);
            }
        }
    }
    tiles
}

#[derive(Clone)]
pub struct TilemapDimensions {
    pub width: u32,
    pub height: u32,
}

impl Component for TilemapDimensions {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Clone)]
pub struct TilesheetDimensions {
    pub width: u32,
    pub height: u32,
}

impl Component for TilesheetDimensions {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Clone)]
pub struct TilemapLayer {
    pub name: String,
    pub layer: Layer,
    pub tiles: Vec<[f32; 4]>,
}

impl TilemapLayer {
    pub fn generate_tilemap_texture(&self, tilesheet_dimensions: &TilesheetDimensions) -> Vec<f32> {
        let mut data = Vec::new();

        // We're mapping the texture from the set of tile indexes. so for each index, we need to generate all pixels in the tile with
        // corresponding tile index. So if we have [1, 2, 3] (tilemap size of 1 * 3 = 3), then the result will have 9 pixels, with
        // all pixels in square 1 being set to 1/tileset_count.
        for x in 0..self.layer.tiles.len() * tilesheet_dimensions.height as usize {
            let row = &self.layer.tiles[x / tilesheet_dimensions.height as usize];
            for y in 0..row.len() * tilesheet_dimensions.width as usize {
                data.push(row[y / tilesheet_dimensions.width as usize] as f32);
            }
        }

        data
    }
}

impl Component for TilemapLayer {
    type Storage = DenseVecStorage<Self>;
}
