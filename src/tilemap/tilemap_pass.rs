use std::marker::PhantomData;

use glsl_layout::*;

use amethyst::assets::AssetStorage;
use amethyst::core::nalgebra::Matrix4;
use amethyst::core::transform::GlobalTransform;

use amethyst::ecs::ReadStorage;

use amethyst::core::specs::prelude::{Join, Read, ReadExpect};

use amethyst::renderer::error::Result;
use amethyst::renderer::{
    ActiveCamera, Camera, Encoder, Factory, Material, MaterialDefaults, Mesh, MeshHandle, Position,
    Query, TexCoord, Texture,
};

use amethyst::renderer::pipe::pass::{Pass, PassData};
use amethyst::renderer::pipe::{Effect, NewEffect};

use gfx::{preset::blend::ALPHA, pso::buffer::ElemStride};
use gfx_core::state::ColorMask;

use super::{TilemapDimensions, TilemapLayer, TilesheetDimensions};

const TILEMAP_VERT_SRC: &[u8] = include_bytes!("../../resources/shaders/tilemap_v.glsl");
const TILEMAP_FRAG_SRC: &[u8] = include_bytes!("../../resources/shaders/tilemap_f.glsl");

#[derive(Clone, Copy, Debug, Uniform)]
struct VertexArgs {
    proj: mat4,
    view: mat4,
    model: mat4,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Uniform)]
struct FragmentArgs {
    u_world_size: vec4,
    u_tilesheet_size: vec4,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TileMapBuffer {
    u_data: [[f32; 4]; 4096],
}

/// Draw mesh without lighting
/// `V` is `VertexFormat`
#[derive(Derivative, Clone, Debug, PartialEq)]
#[derivative(Default(bound = "V: Query<(Position, TexCoord)>, Self: Pass"))]
pub struct DrawTilemap<V> {
    _pd: PhantomData<V>,
    layer_name: String,
}

impl<V> DrawTilemap<V>
where
    V: Query<(Position, TexCoord)>,
    Self: Pass,
{
    /// Create instance of `DrawTilemap` pass
    pub fn new(layer_name: &str) -> Self {
        let mut ret: DrawTilemap<V> = Default::default();
        ret.layer_name = String::from(layer_name);
        ret
    }
}

impl<'a, V> PassData<'a> for DrawTilemap<V>
where
    V: Query<(Position, TexCoord)>,
{
    type Data = (
        Option<Read<'a, ActiveCamera>>,
        ReadStorage<'a, Camera>,
        Read<'a, AssetStorage<Mesh>>,
        Read<'a, AssetStorage<Texture>>,
        ReadExpect<'a, MaterialDefaults>,
        ReadStorage<'a, MeshHandle>,
        ReadStorage<'a, Material>,
        ReadStorage<'a, GlobalTransform>,
        ReadStorage<'a, TilemapDimensions>,
        ReadStorage<'a, TilesheetDimensions>,
        ReadStorage<'a, TilemapLayer>,
    );
}

impl<V> Pass for DrawTilemap<V>
where
    V: Query<(Position, TexCoord)>,
{
    fn compile(&mut self, effect: NewEffect) -> Result<Effect> {
        use std::mem;
        effect
            .simple(TILEMAP_VERT_SRC, TILEMAP_FRAG_SRC)
            .with_raw_constant_buffer("VertexArgs", mem::size_of::<VertexArgs>(), 1)
            .with_raw_vertex_buffer(V::QUERIED_ATTRIBUTES, V::size() as ElemStride, 0)
            .with_raw_constant_buffer("TileMapBuffer", mem::size_of::<TileMapBuffer>(), 1)
            .with_raw_constant_buffer("FragmentArgs", mem::size_of::<FragmentArgs>(), 1)
            .with_texture("TilesheetTexture")
            .with_blended_output("Color", ColorMask::all(), ALPHA, None)
            .build()
    }

    fn apply<'a, 'b: 'a>(
        &'a mut self,
        encoder: &mut Encoder,
        effect: &mut Effect,
        _factory: Factory,
        (
            active,
            camera,
            mesh_storage,
            tex_storage,
            material_defaults,
            mesh,
            material,
            global,
            tilemap_dimensions,
            tilesheet_dimensions,
            tile_layer,
        ): (
            Option<Read<'a, ActiveCamera>>,
            ReadStorage<'a, Camera>,
            Read<'a, AssetStorage<Mesh>>,
            Read<'a, AssetStorage<Texture>>,
            ReadExpect<'a, MaterialDefaults>,
            ReadStorage<'b, MeshHandle>,
            ReadStorage<'b, Material>,
            ReadStorage<'b, GlobalTransform>,
            ReadStorage<'b, TilemapDimensions>,
            ReadStorage<'b, TilesheetDimensions>,
            ReadStorage<'b, TilemapLayer>,
        ),
    ) {
        let camera: Option<(&Camera, &GlobalTransform)> = active
            .and_then(|a| {
                let cam = camera.get(a.entity.unwrap());
                let transform = global.get(a.entity.unwrap());
                cam.into_iter().zip(transform.into_iter()).next()
            })
            .or_else(|| (&camera, &global).join().next());

        let mesh_storage = &mesh_storage;
        let tex_storage = &tex_storage;
        let material_defaults = &material_defaults;

        for (mesh, material, global, tilemap_dimensions, tilesheet_dimensions, tile_layer) in (
            &mesh,
            &material,
            &global,
            &tilemap_dimensions,
            &tilesheet_dimensions,
            &tile_layer,
        )
            .join()
        {
            if tile_layer.name != self.layer_name {
                continue;
            }

            let mesh = match mesh_storage.get(mesh) {
                Some(mesh) => mesh,
                None => continue,
            };
            let vbuf = match mesh.buffer(V::QUERIED_ATTRIBUTES) {
                Some(vbuf) => vbuf.clone(),
                None => return,
            };

            let vertex_args = camera
                .as_ref()
                .map(|&(ref cam, ref transform)| {
                    let proj: [[f32; 4]; 4] = cam.proj.into();
                    let view: [[f32; 4]; 4] = transform
                        .0
                        .try_inverse()
                        .unwrap_or_else(|| Matrix4::repeat(1.))
                        .into();
                    let model: [[f32; 4]; 4] = global.0.into();

                    VertexArgs {
                        proj: proj.into(),
                        view: view.into(),
                        model: model.into(),
                    }
                })
                .unwrap_or_else(|| {
                    let proj: [[f32; 4]; 4] = Matrix4::repeat(1.).into();
                    let view: [[f32; 4]; 4] = Matrix4::repeat(1.).into();
                    let model: [[f32; 4]; 4] = global.0.into();
                    VertexArgs {
                        proj: proj.into(),
                        view: view.into(),
                        model: model.into(),
                    }
                });

            let option_tilesheet_texture = tex_storage
                .get(&material.albedo)
                .or_else(|| tex_storage.get(&material_defaults.0.albedo));

            if let Some(tilesheet_texture) = option_tilesheet_texture {
                //debug!("Updating VertexArgs");
                effect.update_constant_buffer("VertexArgs", &vertex_args.std140(), encoder);
                effect.data.textures.push(tilesheet_texture.view().clone());
                effect
                    .data
                    .samplers
                    .push(tilesheet_texture.sampler().clone());
            }

            let fragment_args = FragmentArgs {
                u_world_size: [
                    tilemap_dimensions.width as f32,
                    tilemap_dimensions.height as f32,
                    0.0,
                    0.0,
                ]
                .into(),
                u_tilesheet_size: [
                    tilesheet_dimensions.width as f32,
                    tilesheet_dimensions.height as f32,
                    0.0,
                    0.0,
                ]
                .into(),
            };
            //debug!("Updating TileMapBuffer");
            effect.update_buffer("TileMapBuffer", &tile_layer.tiles[..], encoder);

            //debug!("Updating FragmentArgs");
            effect.update_constant_buffer("FragmentArgs", &fragment_args.std140(), encoder);

            effect.data.vertex_bufs.push(vbuf);

            effect.draw(mesh.slice(), encoder);
            effect.clear();
        }
    }
}
