use std::{collections::{HashMap, HashSet}, io::{stdout, Cursor, Read, Seek}, ops};
use std::io::Write;

use image::{codecs::dds::DdsDecoder, EncodableLayout, ImageDecoder, ImageReader};
use three_d::{Matrix4, SquareMatrix, Vector2, Vector3, Vector4};

use crate::{byte_stream::ByteReader, mesh_data::TextureData, wta_wtp::WtaWtp};
use crate::mesh_data::MeshData;

pub fn read_wmb_mgrr<R1: Read + Seek, R2: Read + Seek>(
	reader: &mut ByteReader<R1>,
	wta_wtp: &mut Option<WtaWtp<R2>>,
	textures: &mut HashMap<u32, TextureData>,
) -> Result<Vec<MeshData>, String> {
	let wmb = Wmb::read(reader)?;

	let mut meshes: Vec<MeshData> = Vec::new();
	let mut i = 0;

	let mut batch_infos: Vec<Option<BatchData>> = Vec::with_capacity(wmb.batches.len());
	for batch_data in wmb.batch_data_group.into_iter() {
		let i = batch_data.batch_index as usize;
		for _ in batch_infos.len()..=i {
			batch_infos.push(None);
		}
		if batch_infos[i].is_some() {
			println!("Batch data already exists for index: {}", i);
		}
		batch_infos[i] = Some(batch_data);
	}

	for mesh in wmb.meshes.iter() {
		let batch_indices = mesh.batches
			.iter()
			.flatten()
			.map(|i| *i as usize)
			.collect::<HashSet<_>>();
		for batch_i in batch_indices.into_iter() {
			i += 1;
			print!("\r{} (textures: {}, current: {})                 ", i, textures.len(), mesh.name);
			stdout().flush();
			let batch = wmb
				.batches
				.get(batch_i)
				.ok_or(format!("Batch index out of bounds: {}", batch_i))?;
			let batch_data = batch_infos.get(batch_i)
				.and_then(|b| b.as_ref())
				.ok_or(format!("Batch data index out of bounds: {}", batch_i))?;
			let material = wmb
				.materials
				.get(batch_data.material_index as usize)
				.ok_or(format!("Material index out of bounds: {}", batch_data.material_index))?;
			let vertex_group = wmb
				.vertex_groups
				.get(batch.vertex_group_index as usize)
				.ok_or(format!("Vertex group index out of bounds: {}", batch.vertex_group_index))?;
			let indexes = vertex_group.indexes
				.chunks(3)
				.map(|i| [i[2], i[1], i[0]])
				.flatten()
				// .iter()
				.skip(batch.index_start as usize)
				.take(batch.num_indexes as usize)
				// .map(|i| *i as u32)
				.collect::<Vec<u32>>();
			let vertex_start = batch.vertex_start as usize;
			let num_vertexes = batch.num_vertices as usize;
			let normals = Some(
				vertex_group.vertexes.iter()
					.skip(vertex_start)
					.take(num_vertexes)
					.map(|v| {
						let normal = &v.normal;
						Vector3::new(normal.x, normal.y, normal.z)
					})
					.collect()
			);

			let albedo_texture_id = material.get_albedo_texture_id(&wmb.textures);
			let normal_texture_id = material.get_normal_texture_id(&wmb.textures);
			if let Some(wta_wtp) = wta_wtp.as_mut() {
				try_add_texture(textures, wta_wtp, albedo_texture_id, None);
				try_add_texture(textures, wta_wtp, normal_texture_id, None);
			}
			
			let mesh_data = MeshData {
				name: format!("{}/{}", mesh.name, batch_i),
				vertices: vertex_group.vertexes.iter()
					.skip(vertex_start)
					.take(num_vertexes)
					.map(|v| Vector3::new(v.position.x, v.position.y, v.position.z))
					.collect(),
				indexes,
				tangents: Some(vertex_group.vertexes.iter()
					.skip(vertex_start)
					.take(num_vertexes)
					.map(|v| Vector4::from(v.tangent))
					.collect()),
				normals,
				uv: vertex_group.vertexes.iter()
					.skip(vertex_start)
					.take(num_vertexes)
					.map(|v| Vector2::new(v.uv[0], v.uv[1]))
					.collect(),	
				albedo_texture_id,
				normal_texture_id,
				mask_texture_id: None,
				transform: Matrix4::identity(),
				uses_transparency: material.alpha_is_transparency(wta_wtp, &wmb.textures),
				should_be_visible: !mesh.name.ends_with("_DEC"),
			};
			meshes.push(mesh_data);
		}
	}
	println!();
	
	Ok(meshes)
}

struct Point {
	x: f32,
	y: f32,
	z: f32,
}

impl Point {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(Point {
			x: reader.read_f32()?,
			y: reader.read_f32()?,
			z: reader.read_f32()?,
		})
	}
}

impl ops::Add<f32> for Point {
	type Output = Point;

	fn add(self, rhs: f32) -> Point {
		Point {
			x: self.x + rhs,
			y: self.y + rhs,
			z: self.z + rhs,
		}
	}
}

impl ops::Div<f32> for Point {
	type Output = Point;

	fn div(self, rhs: f32) -> Point {
		Point {
			x: self.x / rhs,
			y: self.y / rhs,
			z: self.z / rhs,
		}
	}
}

struct Wmb {
	vertex_groups: Vec<VertexGroup>,
	batches: Vec<Batch>,
	batch_data_group: Vec<BatchData>,
	bones: Vec<Bone>,
	bone_index_translate_table: BoneIndexTranslateTable,
	bone_sets: Vec<BoneSet>,
	materials: Vec<Material>,
	textures: Vec<Texture>,
	meshes: Vec<Mesh>,
}

impl Wmb {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let header = Header::read(reader)?;
		
		let mut vertex_groups = Vec::with_capacity(header.num_vertex_groups as usize);
		reader.seek(header.offset_vertex_groups as u64)?;
		for _ in 0..header.num_vertex_groups {
			vertex_groups.push(VertexGroup::read(reader, header.vertex_format)?);
		}

		let mut batches = Vec::with_capacity(header.num_batches as usize);
		if header.offset_batches != 0 {
			reader.seek(header.offset_batches as u64)?;
			for _ in 0..header.num_batches {
				batches.push(Batch::read(reader)?);
			}
		}

		let mut batch_data_group = None;
		if header.offset_batch_description != 0 {
			reader.seek(header.offset_batch_description as u64)?;
			batch_data_group = Some(BatchDataGroup::read(reader)?);
		}

		let mut bones = Vec::with_capacity(header.num_bones as usize);
		if header.offset_bones != 0 {
			reader.seek(header.offset_bones as u64)?;
			for _ in 0..header.num_bones {
				bones.push(Bone::read(reader)?);
			}
		}

		let bone_index_translate_table = if header.offset_bone_index_translate_table != 0 {
			reader.seek(header.offset_bone_index_translate_table as u64)?;
			BoneIndexTranslateTable::read(reader)?
		} else {
			BoneIndexTranslateTable {
				first_level: [0; 16],
				second_level: Vec::new(),
				third_level: Vec::new(),
			}
		};

		let mut bone_sets = Vec::with_capacity(header.num_bone_sets as usize);
		if header.offset_bone_sets != 0 {
			reader.seek(header.offset_bone_sets as u64)?;
			for _ in 0..header.num_bone_sets {
				bone_sets.push(BoneSet::read(reader)?);
			}
		}

		let mut materials = Vec::with_capacity(header.num_materials as usize);
		if header.offset_materials != 0 {
			reader.seek(header.offset_materials as u64)?;
			for _ in 0..header.num_materials {
				materials.push(Material::read(reader)?);
			}
		}

		let mut textures = Vec::with_capacity(header.num_textures as usize);
		if header.offset_textures != 0 {
			reader.seek(header.offset_textures as u64)?;
			for _ in 0..header.num_textures {
				textures.push(Texture::read(reader)?);
			}
		}

		let mut meshes = Vec::with_capacity(header.num_meshes as usize);
		if header.offset_meshes != 0 {
			reader.seek(header.offset_meshes as u64)?;
			for _ in 0..header.num_meshes {
				meshes.push(Mesh::read(reader)?);
			}
		}

		Ok(Wmb {
			vertex_groups,
			batches,
			batch_data_group: batch_data_group.map(|b| b.batch_data).unwrap_or(Vec::new()),
			bones,
			bone_index_translate_table,
			bone_sets,
			materials,
			textures,
			meshes,
		})
	}
}

struct Header {
	id: String,
	u_a: u32,
	vertex_format: u32,
	u_b: u16,
	u_c: i16,
	pos1: Point,
	pos2: Point,
	offset_vertex_groups: u32,
	num_vertex_groups: u32,
	offset_batches: u32,
	num_batches: u32,
	offset_batch_description: u32,
	offset_bones: u32,
	num_bones: u32,
	offset_bone_index_translate_table: u32,
	size_bone_index_translate_table: u32,
	offset_bone_sets: u32,
	num_bone_sets: u32,
	offset_materials: u32,
	num_materials: u32,
	offset_textures: u32,
	num_textures: u32,
	offset_meshes: u32,
	num_meshes: u32,
}

impl Header {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(Header {
			id: reader.read_string(4)?,
			u_a: reader.read_u32()?,
			vertex_format: reader.read_u32()?,
			u_b: reader.read_u16()?,
			u_c: reader.read_i16()?,
			pos1: Point::read(reader)?,
			pos2: Point::read(reader)?,
			offset_vertex_groups: reader.read_u32()?,
			num_vertex_groups: reader.read_u32()?,
			offset_batches: reader.read_u32()?,
			num_batches: reader.read_u32()?,
			offset_batch_description: reader.read_u32()?,
			offset_bones: reader.read_u32()?,
			num_bones: reader.read_u32()?,
			offset_bone_index_translate_table: reader.read_u32()?,
			size_bone_index_translate_table: reader.read_u32()?,
			offset_bone_sets: reader.read_u32()?,
			num_bone_sets: reader.read_u32()?,
			offset_materials: reader.read_u32()?,
			num_materials: reader.read_u32()?,
			offset_textures: reader.read_u32()?,
			num_textures: reader.read_u32()?,
			offset_meshes: reader.read_u32()?,
			num_meshes: reader.read_u32()?,
		})
	}
}

struct BoundingBox {
	x: f32,
	y: f32,
	z: f32,
	u: f32,
	v: f32,
	w: f32,
}

impl BoundingBox {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(BoundingBox {
			x: reader.read_f32()?,
			y: reader.read_f32()?,
			z: reader.read_f32()?,
			u: reader.read_f32()?,
			v: reader.read_f32()?,
			w: reader.read_f32()?,
		})
	}
}

struct Bone {
	unknown_number: i16,
	unknown_number2: i16,
	parent_index: i16,
	u_b: i16,
	relative_position: Point,
	position: Point,
}

impl Bone {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(Bone {
			unknown_number: reader.read_i16()?,
			unknown_number2: reader.read_i16()?,
			parent_index: reader.read_i16()?,
			u_b: reader.read_i16()?,
			relative_position: Point::read(reader)?,
			position: Point::read(reader)?,
		})
	}
}

struct BoneIndexTranslateTable {
	first_level: [i16; 16],
	second_level: Vec<i16>,
	third_level: Vec<i16>,
}

impl BoneIndexTranslateTable {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let mut first_level = [0; 16];
		for i in 0..16 {
			first_level[i] = reader.read_i16()?;
		}

		let mut j = 0;
		for i in 0..16 {
			if first_level[i] != -1 {
				j += 1;
			}
		}

		let mut second_level = Vec::new();
		for _ in 0..j*16 {
			second_level.push(reader.read_i16()?);
		}

		let mut k = 0;
		for i in 0..j*16 {
			if second_level[i] != -1 {
				k += 1;
			}
		}

		let mut third_level = Vec::new();
		for _ in 0..k*16 {
			third_level.push(reader.read_i16()?);
		}

		Ok(BoneIndexTranslateTable {
			first_level,
			second_level,
			third_level,
		})
	}
}

struct Vertex {
	position: Point,
	uv: [f32; 2],
	normal: Point,
	tangent: [f32; 4],
	color: Option<[u8; 4]>,
	uv2: Option<[f32; 2]>,
	bone_indices: Option<[u8; 4]>,
	bone_weights: Option<[f32; 4]>,
}

impl Vertex {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>, vertex_flags: u32) -> Result<Self, String> {
		let position = Point::read(reader)?;
		let uv = [
			reader.read_f16()?.to_f32(),
			reader.read_f16()?.to_f32()
		];
		let normal = reader.read_u32()? as i64;
		let mut normal_x = normal & ((1 << 11) - 1);
        let mut normal_y = (normal >> 11) & ((1 << 11) - 1);
        let mut normal_z = normal >> 22;
        if normal_x & (1 << 10) != 0 {
            normal_x &= !(1 << 10);
            normal_x -= 1 << 10;
		}
        if normal_y & (1 << 10) != 0 {
            normal_y &= !(1 << 10);
            normal_y -= 1 << 10;
		}
        if normal_z & (1 << 9) != 0 {
            normal_z &= !(1 << 9);
            normal_z -= 1 << 9;
		}
        let normal_x = normal_x as f32 / ((1<<10)-1) as f32;
        let normal_y = normal_y as f32 / ((1<<10)-1) as f32;
        let normal_z = normal_z as f32 / ((1<<9)-1) as f32;
		let normal = Point {
			x: normal_x,
			y: normal_y,
			z: normal_z,
		};
		let tangent = [
			(reader.read_u8()? as f32 - 127.0) / 127.0,
			(reader.read_u8()? as f32 - 127.0) / 127.0,
			(reader.read_u8()? as f32 - 127.0) / 127.0,
			(reader.read_u8()? as f32 - 127.0) / 127.0,
		];

		let mut color = None;
		let mut uv2 = None;
		let mut bone_indices = None;
		let mut bone_weights = None;

		if (vertex_flags & 0x137) == 0x137 {
			bone_indices = Some([
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
			]);
			bone_weights = Some([
				reader.read_u8()? as f32 / 255.0,
				reader.read_u8()? as f32 / 255.0,
				reader.read_u8()? as f32 / 255.0,
				reader.read_u8()? as f32 / 255.0,
			]);
		} else if vertex_flags == 0x10307 {
			color = Some([
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
			]);
			uv2 = Some([
				reader.read_f16()?.to_f32(),
				reader.read_f16()?.to_f32(),
			]);
		} else if vertex_flags == 0x10107 {
			color = Some([
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
			]);
		}

		Ok(Vertex {
			position,
			uv,
			normal,
			tangent,
			color,
			uv2,
			bone_indices,
			bone_weights,
		})
	}
}

struct VertexGroupHeader {
	vertex_offset: u32,
	vertex_ex_data_offset: u32,
	unknown1_offset: u32,
	unknown2_offset: u32,
	num_vertexes: u32,
	index_buffer_offset: u32,
	num_indexes: u32,
}

impl VertexGroupHeader {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(VertexGroupHeader {
			vertex_offset: reader.read_u32()?,
			vertex_ex_data_offset: reader.read_u32()?,
			unknown1_offset: reader.read_u32()?,
			unknown2_offset: reader.read_u32()?,
			num_vertexes: reader.read_u32()?,
			index_buffer_offset: reader.read_u32()?,
			num_indexes: reader.read_u32()?,
		})
	}
}

struct VertexExData {
	color: Option<[u8; 4]>,
	uv2: Option<[f32; 2]>,
}

impl VertexExData {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>, vertex_flags: u32) -> Result<Self, String> {
		let mut color = None;
		let mut uv2 = None;

		if (vertex_flags & 0x337) == 0x337 {
			color = Some([
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
			]);
			uv2 = Some([
				reader.read_f16()?.to_f32(),
				reader.read_f16()?.to_f32(),
			]);
		} else if vertex_flags == 0x10137 {
			color = Some([
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
			]);
		}

		Ok(VertexExData {
			color,
			uv2,
		})
	}
}

struct VertexGroup {
	vertexes: Vec<Vertex>,
	vertex_ex_data: Vec<VertexExData>,
	indexes: Vec<u32>,
}

impl VertexGroup {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>, vertex_flags: u32) -> Result<Self, String> {
		let header = VertexGroupHeader::read(reader)?;
		let mut vertexes = Vec::with_capacity(header.num_vertexes as usize);
		let mut vertex_ex_data = Vec::with_capacity(header.num_vertexes as usize);
		let mut indexes = Vec::with_capacity(header.num_indexes as usize);
		let pos = reader.position()?;

		reader.seek(header.vertex_offset as u64)?;
		for _ in 0..header.num_vertexes {
			vertexes.push(Vertex::read(reader, vertex_flags)?);
		}

		if header.vertex_ex_data_offset != 0 {
			reader.seek(header.vertex_ex_data_offset as u64)?;
			for _ in 0..header.num_vertexes {
				vertex_ex_data.push(VertexExData::read(reader, vertex_flags)?);
			}
		}

		reader.seek(header.index_buffer_offset as u64)?;
		for _ in 0..header.num_indexes {
			indexes.push(reader.read_u16()? as u32);
		}

		reader.seek(pos)?;

		Ok(VertexGroup {
			vertexes,
			vertex_ex_data,
			indexes,
		})
	}
}

struct Batch {
	vertex_group_index: u32,
	vertex_start: i32,
	index_start: i32,
	num_vertices: u32,
	num_indexes: u32,
}

impl Batch {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(Batch {
			vertex_group_index: reader.read_u32()?,
			vertex_start: reader.read_i32()?,
			index_start: reader.read_i32()?,
			num_vertices: reader.read_u32()?,
			num_indexes: reader.read_u32()?,
		})
	}
}

struct BatchData {
	batch_index: u32,
	mesh_index: u32,
	material_index: u16,
	bone_sets_index: u16,
	u_a: u32,
}

impl BatchData {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(BatchData {
			batch_index: reader.read_u32()?,
			mesh_index: reader.read_u32()?,
			material_index: reader.read_u16()?,
			bone_sets_index: reader.read_u16()?,
			u_a: reader.read_u32()?,
		})
	}
}

struct BatchDataGroup {
	batch_data: Vec<BatchData>,
}

impl BatchDataGroup {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let mut batch_data = Vec::with_capacity(4);
		for _ in 0..4 {
			let offset = reader.read_u32()?;
			let num = reader.read_u32()?;
			if offset != 0 {
				let pos = reader.position()?;
				reader.seek(offset as u64)?;
				for _ in 0..num {
					batch_data.push(BatchData::read(reader)?);
				}
				reader.seek(pos)?;
			}
		}

		Ok(BatchDataGroup {
			batch_data,
		})
	}
}


struct BoneSet {
	bone_indexes: Vec<u8>,
}

impl BoneSet {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let offset_bone_set = reader.read_u32()?;
		let num_bone_indexes = reader.read_u32()?;
		let pos = reader.position()?;

		reader.seek(offset_bone_set as u64)?;
		let mut bone_indexes = Vec::with_capacity(num_bone_indexes as usize);
		for _ in 0..num_bone_indexes {
			bone_indexes.push(reader.read_u8()?);
		}

		reader.seek(pos)?;

		Ok(BoneSet {
			bone_indexes,
		})
	}
}

struct TextureIndices {
	flags_indices: [(u32, u32); 4],
}

impl TextureIndices {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(TextureIndices {flags_indices: [
			(reader.read_u32()?, reader.read_u32()?),
			(reader.read_u32()?, reader.read_u32()?),
			(reader.read_u32()?, reader.read_u32()?),
			(reader.read_u32()?, reader.read_u32()?),
		]})
	}

	fn get_albedo_index(&self) -> Option<u32> {
		for (flag, index) in self.flags_indices {
			if flag == 0 || flag == 1 {
				return Some(index);
			}
		}
		None
	}

	fn get_normal_index(&self) -> Option<u32> {
		for (flag, index) in self.flags_indices {
			if flag == 2 {
				return Some(index);
			}
		}
		None
	}

	fn get_albedo_indices(&self) -> impl Iterator<Item = u32> + use<'_> {
		self.flags_indices.iter()
			.filter(|(flag, _)| *flag == 0 || *flag == 1)
			.map(|(_, index)| *index)
	}
}

struct Material {
	shader_name: String,
	texture_indices: TextureIndices,
	parameters: Vec<f32>,
}

impl Material {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let offset_shader_name = reader.read_u32()?;
		let offset_textures = reader.read_u32()?;
		let _u_a = reader.read_u32()?;
		let offset_parameters = reader.read_u32()?;
		let _num_textures = reader.read_u16()?;
		let _u_c = reader.read_u16()?;
		let _u_d = reader.read_u16()?;
		let num_parameters = reader.read_u16()?;
		let pos = reader.position()?;

		reader.seek(offset_shader_name as u64)?;
		let shader_name = reader.read_string(16)?;

		reader.seek(offset_textures as u64)?;
		let texture_indices = TextureIndices::read(reader)?;

		reader.seek(offset_parameters as u64)?;
		let mut parameters = Vec::with_capacity(num_parameters as usize);
		for _ in 0..num_parameters {
			parameters.push(reader.read_f32()?);
		}

		reader.seek(pos)?;

		Ok(Material {
			shader_name,
			texture_indices,
			parameters,
		})
	}

	fn get_albedo_texture_id(&self, textures: &Vec<Texture>) -> Option<u32> {
		self.texture_indices.get_albedo_index()
			.and_then(|index| textures.get(index as usize))
			.map(|texture| texture.id)	
	}

	fn get_normal_texture_id(&self, textures: &Vec<Texture>) -> Option<u32> {
		self.texture_indices.get_normal_index()
			.and_then(|index| textures.get(index as usize))
			.map(|texture| texture.id)
	}

	fn alpha_is_transparency<F: Read + Seek>(&self, wta_wtp: &mut Option<WtaWtp<F>>, textures: &Vec<Texture>) -> bool {
		if let Some(wta_wtp) = wta_wtp {
			if self.shader_name.len() >= 5 {
				const ORGANIC_PREFIXES: [&str; 3] = ["eye", "har", "skn"];
				let is_organic = ORGANIC_PREFIXES.iter().any(|prefix| self.shader_name.starts_with(prefix));
				let albedo_tex_count = self.texture_indices.get_albedo_indices()
					.map(|index| textures.get(index as usize))
					.flatten()
					.filter(|texture| wta_wtp.has_id(texture.id))
					.count();
				if albedo_tex_count == 1 || (is_organic && albedo_tex_count >= 1) {
					if &self.shader_name[3..5] != "00" || is_organic {
						if &self.shader_name[4..5] != "0" || &self.shader_name[0..3] == "har" {
							return  true;
						}
					}
				}
			}
		}
		false
	}
}


struct Texture {
	flags: u32,
	id: u32,
}

impl Texture {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(Texture {
			flags: reader.read_u32()?,
			id: reader.read_u32()?,
		})
	}
}

struct Mesh {
	name: String,
	bounding_box: BoundingBox,
	batches: Vec<Vec<u16>>,
}

impl Mesh {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let offset_name = reader.read_u32()?;
		let bounding_box = BoundingBox::read(reader)?;
		let batch_info = [
			(reader.read_u32()?, reader.read_u32()?),
			(reader.read_u32()?, reader.read_u32()?),
			(reader.read_u32()?, reader.read_u32()?),
			(reader.read_u32()?, reader.read_u32()?),
		];
		let offset_materials = reader.read_u32()?;
		let num_materials = reader.read_u32()?;
		let pos = reader.position()?;

		reader.seek(offset_name as u64)?;
		let name = reader.read_string_zero_term()?;

		let mut batches = Vec::new();
		for i in 0..4 {
			if batch_info[i].0 != 0 {
				reader.seek(batch_info[i].0 as u64)?;
				let mut batch = Vec::with_capacity(batch_info[i].1 as usize);
				for _ in 0..batch_info[i].1 {
					batch.push(reader.read_u16()?);
				}
				batches.push(batch);
			}
		}

		reader.seek(offset_materials as u64)?;
		let mut materials = Vec::with_capacity(num_materials as usize);
		for _ in 0..num_materials {
			materials.push(reader.read_u16()?);
		}

		reader.seek(pos)?;

		Ok(Mesh {
			name,
			bounding_box,
			batches,
		})
	}
}


fn decompress_dds(bytes: Vec<u8>, swizzle: Option<&dyn Fn(&mut[u8]) -> ()>) -> Result<TextureData, String> {
	const DDS_MAGIC: [u8; 4] = [0x44, 0x44, 0x53, 0x20];
	const PNG_MAGIC: [u8; 4] = [0x89, 0x50, 0x4E, 0x47];
	if bytes.len() > 4 && bytes[0..4] == DDS_MAGIC {
		let dds_decoder = DdsDecoder::new(bytes.as_slice()).map_err(|e| e.to_string())?;
		let (width, height) = dds_decoder.dimensions();
		let buffer_size = dds_decoder.total_bytes() as usize;
		let mut buffer = vec![0; buffer_size];
		dds_decoder.read_image(&mut buffer).map_err(|e| e.to_string())?;
		if let Some(swizzle) = swizzle {
			for i in (0..buffer.len()).step_by(4) {
				swizzle(&mut buffer[i..i + 4]);
			}
		}
		Ok(TextureData { bytes: buffer, width, height })
	} else if bytes.len() > 4 && bytes[0..4] == PNG_MAGIC {
		let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().unwrap();
		let image = reader.decode().map_err(|e| e.to_string())?;
		let image = image.to_rgba8();
		let width = image.width();
		let height = image.height();
		let buffer = image.as_bytes();
		let buffer = buffer.to_vec();
		Ok(TextureData { bytes: buffer, width, height })
	} else {
		Err("Unknown texture format".to_string())
	}
}

fn try_add_texture<F: Read + Seek>(
	textures: &mut HashMap<u32, TextureData>,
	wta_wtp: &mut WtaWtp<F>,
	texture_id: Option<u32>,
	swizzle: Option<&dyn Fn(&mut[u8]) -> ()>,
) -> () {
	texture_id
		.and_then(|texture_id| wta_wtp.get_texture(texture_id))
		.and_then(|texture| decompress_dds(texture, swizzle).ok())
		.map(|texture_data| textures.insert(texture_id.unwrap(), texture_data));
}
