use std::collections::HashMap;

use three_d::{Matrix4, Vector2, Vector3, Vector4};


#[derive(Default)]
pub struct SceneData {
	pub meshes: Vec<MeshData>,
	pub textures: HashMap<u32, TextureData>,
}

pub struct MeshData {
	pub name: String,
	pub vertices: Vec<Vector3<f32>>,
	pub indexes: Vec<u32>,
	pub tangents: Option<Vec<Vector4<f32>>>,
	pub normals: Option<Vec<Vector3<f32>>>,
	pub uv: Vec<Vector2<f32>>,
	pub albedo_texture_id: Option<u32>,
	pub normal_texture_id: Option<u32>,
	pub mask_texture_id: Option<u32>,
	pub transform: Matrix4<f32>,
	pub uses_transparency: bool,
	pub should_be_visible: bool,
}

#[derive(Clone)]
pub struct TextureData {
	pub bytes: Vec<u8>,
	pub width: u32,
	pub height: u32,
}
