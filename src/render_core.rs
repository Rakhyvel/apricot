//! This file contains the core rendering functionality that is shared between 2D and 3D rendering.

use std::fmt::Debug;

use nalgebra_glm::{Mat4, Vec3, Vec4};

use crate::{
    camera::ViewProj,
    error::Error,
    mesh::{MeshData, MeshShape},
    opengl::{self, create_program, DrawOptions, Primitive, RawMesh},
    registry::{Handle, Registry},
};

use super::{
    font::{FontId, FontManager},
    opengl::{Program, Texture, Uniform},
    render2d::Queue2d,
    resources::Resources,
    view::View,
};

const DEFAULT_FONT: &[u8] = include_bytes!("fonts/Not Jam Mono Clean 16.ttf");

pub struct RenderContext {
    pub(crate) resources: Resources,
    pub(crate) view: View,
    pub(crate) queue_2d: Queue2d,
    pub builtins: Builtins,
}

pub struct Builtins {
    pub program_2d: ProgramId,
    pub program_3d: ProgramId,
    pub program_3d_solid: ProgramId,
    pub program_shadow: ProgramId,
    pub program_line: ProgramId,
    pub white_texture: TextureId,
    pub font: FontId,
    pub quad: MeshId,
    pub cube: MeshId,
    pub plane: MeshId,
}

/// Refers to a mesh added to the renderer
pub type MeshId = Handle<Mesh>;

/// Refers to a texture added to the renderer
pub type TextureId = Handle<Texture>;

/// Refers to a shader program added to the renderer
pub type ProgramId = Handle<Program>;

/// How a `LinePath` looks
#[derive(Clone, Copy, Debug)]
pub struct LineStyle {
    /// Thickness in pixels. Most drivers only support 1
    pub width: f32,
    pub color: Vec4,
    /// Whether the line fades out behind the seam, like a trail
    pub fade: bool,
    /// Where along the path the fade starts, from 0 (the first point) to 1 (the last)
    pub seam: f32,
    /// Whether 3D geometry in front of the line hides it
    pub depth_test: bool,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            color: nalgebra_glm::vec4(0.7, 0.9, 0.9, 0.9),
            fade: true,
            seam: 0.0,
            depth_test: true,
        }
    }
}

/// A 3D line through a list of points
pub struct LinePath {
    mesh: RawMesh,
    pub style: LineStyle,
}

/// A mesh uploaded to the GPU, with its shape kept on the CPU for picking and culling
pub struct Mesh {
    raw: RawMesh,
    shape: MeshShape,
}

impl RenderContext {
    pub fn new() -> Result<Self, Error> {
        fn builtin(vert: &'static str, frag: &'static str) -> Program {
            create_program(vert, frag).expect("apricot built-in shader failed to compile")
        }

        let mut programs = Registry::new();
        let mut textures = Registry::new();
        let mut meshes = Registry::new();

        let builtins = Builtins {
            program_2d: programs.add(
                builtin(
                    include_str!("shaders/2d.vert"),
                    include_str!("shaders/2d.frag"),
                ),
                Some("2d"),
            ),
            program_3d: programs.add(
                builtin(
                    include_str!("shaders/3d.vert"),
                    include_str!("shaders/3d.frag"),
                ),
                Some("3d"),
            ),
            program_3d_solid: programs.add(
                builtin(
                    include_str!("shaders/3d.vert"),
                    include_str!("shaders/solid-color.frag"),
                ),
                Some("3d-solid"),
            ),
            program_shadow: programs.add(
                builtin(
                    include_str!("shaders/shadow.vert"),
                    include_str!("shaders/shadow.frag"),
                ),
                Some("shadow"),
            ),
            program_line: programs.add(
                builtin(
                    include_str!("shaders/line.vert"),
                    include_str!("shaders/line.frag"),
                ),
                Some("line"),
            ),
            white_texture: {
                let white = Texture::new();
                white.set_pixels(1, 1, &[255; 4]);
                textures.add(white, Some("white"))
            },
            font: FontId::placeholder(0),
            quad: meshes.add(Mesh::from_data(MeshData::quad()), Some("quad")),
            cube: meshes.add(Mesh::from_data(MeshData::cube()), Some("cube")),
            plane: meshes.add(Mesh::from_data(MeshData::plane()), Some("plane")),
        };

        let mut renderer = Self {
            resources: Resources::new(meshes, textures, programs, FontManager::new()?),
            view: View::default(),
            queue_2d: Queue2d::default(),
            builtins,
        };

        renderer.builtins.font = renderer.add_font_from_bytes(
            DEFAULT_FONT,
            "default",
            16,
            sdl2::ttf::FontStyle::NORMAL,
        )?;
        renderer.set_font(renderer.builtins.font);
        Ok(renderer)
    }

    /// The uniform called `name` in `program`. Setting it works any time, whatever's being drawn
    pub fn uniform(&self, program: ProgramId, name: &str) -> Uniform {
        self.get_program_from_id(program)
            .expect("invalid ProgramId")
            .uniform(name)
    }

    fn bind(&self, program: ProgramId) {
        self.get_program_from_id(program)
            .expect("invalid ProgramId")
            .bind();
    }

    pub fn clear(&self, color: Vec4) {
        opengl::clear(color);
    }

    /// Draw `mesh` with `program`, which gets the model, view, and projection matrices
    pub fn draw(
        &self,
        program: ProgramId,
        mesh: &Mesh,
        model_matrix: nalgebra_glm::Mat4,
        view_proj: &ViewProj,
    ) {
        self.uniform(program, "u_model_matrix")
            .set_mat4(&model_matrix);
        self.uniform(program, "u_view_matrix")
            .set_mat4(&view_proj.view);
        self.uniform(program, "u_proj_matrix")
            .set_mat4(&view_proj.proj);

        self.bind(program);
        mesh.raw.draw();
    }

    /// Draw `mesh_id` with the built-in 3D shader from the current camera, in a solid `color`
    pub fn draw_mesh(&self, mesh_id: MeshId, model: Mat4, color: Vec4) {
        self.draw_mesh_textured(mesh_id, self.builtins.white_texture, model, color);
    }

    /// Draw `mesh_id` with a texture, tinted by `tint`. A white tint leaves the texture as is
    pub fn draw_mesh_textured(
        &self,
        mesh_id: MeshId,
        texture_id: TextureId,
        model: Mat4,
        tint: Vec4,
    ) {
        self.draw_mesh_textured_with(mesh_id, texture_id, model, tint, &DrawOptions::default());
    }

    /// Draw `mesh_id` with a texture and custom GL `options`
    pub(crate) fn draw_mesh_textured_with(
        &self,
        mesh_id: MeshId,
        texture_id: TextureId,
        model: Mat4,
        tint: Vec4,
        options: &DrawOptions,
    ) {
        let before = options.apply();
        let program = self.builtins.program_3d;
        self.uniform(program, "u_color").set_vec4(&tint);
        self.uniform(program, "shadow_map").set_i32(1);

        match self.light() {
            Some(light) => {
                self.uniform(program, "u_sun_dir")
                    .set_vec3(&light.light_dir);
                self.uniform(program, "light_mvp")
                    .set_mat4(&light.light_proj_view);
                self.uniform(program, "u_shadows")
                    .set_i32(light.shadows as i32);
                self.uniform(program, "u_shadow_texel")
                    .set_f32(light.texel_size);
                if light.shadows {
                    opengl::bind_texture_id(1, light.depth_map);
                }
            }
            None => {
                self.uniform(program, "u_sun_dir").set_vec3(&Vec3::zeros());
                self.uniform(program, "u_shadows").set_i32(0);
            }
        }

        let texture = self
            .get_texture_from_id(texture_id)
            .expect("invalid TextureId");
        texture.activate(0);
        self.uniform(program, "texture0").set_i32(0);

        let mesh = self.get_mesh_from_id(mesh_id).expect("invalid MeshId");
        self.draw(program, &mesh, model, &self.camera_view_proj());
        before.restore();
    }

    /// Draw a line path from the current camera, moved by `transform`
    pub fn draw_line_path(&self, path: &LinePath, transform: Mat4) {
        let style = &path.style;
        let program = self.builtins.program_line;

        let view_proj = self.camera_view_proj();
        self.uniform(program, "u_model_matrix").set_mat4(&transform);
        self.uniform(program, "u_view_matrix")
            .set_mat4(&view_proj.view);
        self.uniform(program, "u_proj_matrix")
            .set_mat4(&view_proj.proj);
        self.uniform(program, "u_color").set_vec4(&style.color);
        self.uniform(program, "u_seam").set_f32(style.seam);
        self.uniform(program, "u_fade").set_i32(style.fade as i32);
        self.uniform(program, "u_num_vertices")
            .set_i32(path.mesh.vertex_count() as i32);

        let options = DrawOptions {
            depth_test: style.depth_test,
            depth_write: false, // lines shouldn't hide what's drawn after them
            line_width: style.width,
            ..Default::default()
        };
        self.draw_raw(&path.mesh, program, &options);
    }

    /// Draw a `RawMesh` with `program` and `options`
    pub fn draw_raw(&self, mesh: &RawMesh, program: ProgramId, options: &DrawOptions) {
        let before = options.apply();
        self.bind(program);
        mesh.draw();
        before.restore();
    }
}

impl LinePath {
    /// A line through `points`, in order
    pub fn new(points: &[Vec3], style: LineStyle) -> Self {
        Self {
            mesh: RawMesh::new(&flatten(points), &[3], Primitive::LineStrip),
            style,
        }
    }

    /// Replace the points the line goes through. Cheaper than making a new `LinePath`
    pub fn set_points(&mut self, points: &[Vec3]) {
        self.mesh.set_vertices(&flatten(points));
    }
}

impl Mesh {
    /// Upload `data` to the GPU. Keeps its shape on the CPU, but the normals and UVs only live on
    /// the GPU
    pub fn from_data(data: MeshData) -> Self {
        // The layout has to match the slots the shaders read
        let raw = RawMesh::with_indices(
            &data.interleaved(),
            &data.indices,
            &MeshData::LAYOUT,
            Primitive::Triangles,
        );
        Mesh {
            raw,
            shape: MeshShape::from(data),
        }
    }

    /// Upload a mesh straight from the bytes of a .obj file
    pub fn from_obj(obj_file_data: &[u8]) -> Result<Self, Error> {
        Ok(Self::from_data(MeshData::from_obj(obj_file_data)?))
    }

    /// The mesh's positions and triangles, for picking and culling
    pub fn shape(&self) -> &MeshShape {
        &self.shape
    }
}

fn flatten(points: &[Vec3]) -> Vec<f32> {
    points.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}
