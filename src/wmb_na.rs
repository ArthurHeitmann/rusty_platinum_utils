use std::{collections::{HashMap, HashSet}, fs::File, io::{stdout, BufReader, Read, Seek}, ops, slice};
use std::io::{self, Write};

use image::{self, codecs::dds::DdsDecoder, ImageBuffer, ImageDecoder, Rgba};
use three_d::{Matrix4, SquareMatrix, Vector2, Vector3, Vector4};

use crate::{byte_stream::ByteReader, mesh_data::{SceneData, TextureData}, wta_wtp::WtaWtp};
use crate::mesh_data::MeshData;

pub fn read_wmb_na<R: Read + Seek>(
	name: &str,
	reader: &mut ByteReader<R>,
	wta_wtp: &mut Option<WtaWtp<BufReader<File>>>,
	textures: &mut HashMap<u32, TextureData>,
) -> Result<Vec<MeshData>, String> {
	let wmb = Wmb::read(reader)?;
	let is_player_pl = name.contains("pl000") || name.contains("pl010") || name.contains("pl020");

	let mut meshes: Vec<MeshData> = Vec::new();
	let mut i = 0;

	for (lod_i, lod) in wmb.lods.iter().enumerate() {
		let lod_name = &lod.name;
		let batch_start_index = lod.batch_start as usize;
		for (batch_i, batch_info) in lod.batch_infos.iter().enumerate() {
			i += 1;
			let mesh = wmb
				.meshes
				.get(batch_info.mesh_index as usize)
				.ok_or(format!("Mesh index out of bounds: {}", batch_info.mesh_index))?;
			print!("\r{} / {} (textures: {}, current: {}_{})                 ", i, lod.batch_infos.len(), textures.len(), lod_name, mesh.name);
			stdout().flush();
			let material = wmb
				.materials
				.get(batch_info.material_index as usize)
				.ok_or(format!("Material index out of bounds: {}", batch_info.material_index))?;
			let vertex_group = wmb
				.vertex_groups
				.get(batch_info.vertex_group_index as usize)
				.ok_or(format!("Vertex group index out of bounds: {}", batch_info.vertex_group_index))?;
			let batch = wmb
				.batches
				.get(batch_start_index + batch_i)
				.ok_or(format!("Batch index out of bounds: {}", batch_start_index + batch_i))?;
			let indexes = vertex_group.indexes
				.chunks(3)
				.map(|i| [i[2], i[1], i[0]])
				.flatten()
				// .iter()
				.skip(batch.index_start as usize)
				.take(batch.num_indexes as usize)
				// .map(|i| *i as u32)
				.collect::<Vec<u32>>();
			let vertex_start = *indexes.iter().min().unwrap() as usize;
			let vertex_end = *indexes.iter().max().unwrap() as usize;
			let num_vertexes = vertex_end - vertex_start + 1;
			let first_vertex = vertex_group.vertexes.get(vertex_start).ok_or("Failed to get first_vertex").unwrap();
			let first_vertex_ex = vertex_group.vertex_ex_data.get(vertex_start).ok_or("Failed to get first_vertex_ex").unwrap();
			let normals = if first_vertex.normal.is_some() {
				Some(vertex_group.vertexes.iter()
					.skip(vertex_start)
					.take(num_vertexes)
					.map(|v| {
						let normal = v.normal.as_ref().unwrap();
						Vector3::new(normal.x, normal.y, normal.z)
					})
					.collect())
			} else if first_vertex_ex.normal.is_some() {
				Some(vertex_group.vertex_ex_data.iter()
					.skip(vertex_start)
					.take(num_vertexes)
					.map(|v| {
						let normal = v.normal.as_ref().unwrap();
						Vector3::new(normal.x, normal.y, normal.z)
					})
					.collect())
			} else {
				None
			};

			let albedo_texture_id = material.get_albedo_texture_id();
			let normal_texture_id = material.get_normal_texture_id();
			let mask_texture_id = material.get_mask_map_texture_id();
			if let Some(wta_wtp) = wta_wtp.as_mut() {
				try_add_texture(textures, wta_wtp, albedo_texture_id, None);
				try_add_texture(textures, wta_wtp, normal_texture_id, None);
				// try_add_texture(&mut textures, &mut wta_wtp, mask_texture_id, Some(&mask_map_swizzle));
			}

			let mut should_be_visible = lod_i == 0;
			should_be_visible &= !is_player_pl || !mesh.name.contains("Armor") && !mesh.name.contains("serious") && !mesh.name.contains("Broken") && !mesh.name.contains("DLC");
			
			let mesh_data = MeshData {
				name: format!("{}/{}/{}", lod_name, mesh.name, batch_i),
				vertices: vertex_group.vertexes.iter()
					.skip(vertex_start)
					.take(num_vertexes)
					.map(|v| Vector3::new(v.position.x, v.position.y, v.position.z))
					.collect(),
				indexes: indexes
					.iter()
					.map(|i| i - vertex_start as u32)
					.collect(),
				tangents: Some(vertex_group.vertexes.iter()
					.skip(vertex_start)
					.take(num_vertexes)
					.map(|v| Vector4::new(v.tangent.x, v.tangent.y, v.tangent.z, -v.tangent_sign))
					.collect()),
				normals,
				uv: vertex_group.vertexes.iter()
					.skip(vertex_start)
					.take(num_vertexes)
					.map(|v| Vector2::new(v.uv[0], v.uv[1]))
					.collect(),	
				albedo_texture_id,
				normal_texture_id,
				mask_texture_id,
				transform: Matrix4::identity(),
				uses_transparency: true,
				should_be_visible,
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
	header: Header,
	bones: Vec<Bone>,
	bone_index_translate_table: BoneIndexTranslateTable,
	vertex_groups: Vec<VertexGroup>,
	batches: Vec<Batch>,
	lods: Vec<Lod>,
	col_tree_nodes: Vec<ColTreeNode>,
	bone_map: Vec<i32>,
	bone_sets: Vec<BoneSet>,
	materials: Vec<Material>,
	meshes: Vec<Mesh>,
	mesh_material: Vec<MeshMaterial>,
}

impl Wmb {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let header = Header::read(reader)?;

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
		
		let mut vertex_groups = Vec::with_capacity(header.num_vertex_groups as usize);
		reader.seek(header.offset_vertex_groups as u64)?;
		for _ in 0..header.num_vertex_groups {
			vertex_groups.push(VertexGroup::read(reader, header.flags as u32)?);
		}

		let mut batches = Vec::with_capacity(header.num_batches as usize);
		if header.offset_batches != 0 {
			reader.seek(header.offset_batches as u64)?;
			for _ in 0..header.num_batches {
				batches.push(Batch::read(reader)?);
			}
		}

		let mut lods = Vec::with_capacity(header.num_lods as usize);
		if header.offset_lods != 0 {
			reader.seek(header.offset_lods as u64)?;
			for _ in 0..header.num_lods {
				lods.push(Lod::read(reader)?);
			}
		}

		let mut col_tree_nodes = Vec::with_capacity(header.num_col_tree_nodes as usize);
		if header.offset_col_tree_nodes != 0 {
			reader.seek(header.offset_col_tree_nodes as u64)?;
			for _ in 0..header.num_col_tree_nodes {
				col_tree_nodes.push(ColTreeNode::read(reader)?);
			}
		}

		let mut bone_map = Vec::with_capacity(header.bone_map_size as usize);
		if header.offset_bone_map != 0 {
			reader.seek(header.offset_bone_map as u64)?;
			for _ in 0..header.bone_map_size {
				bone_map.push(reader.read_i32()?);
			}
		}

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

		let mut meshes = Vec::with_capacity(header.num_meshes as usize);
		if header.offset_meshes != 0 {
			reader.seek(header.offset_meshes as u64)?;
			for _ in 0..header.num_meshes {
				meshes.push(Mesh::read(reader)?);
			}
		}

		let mut mesh_material = Vec::with_capacity(header.num_mesh_material as usize);
		if header.offset_mesh_material != 0 {
			reader.seek(header.offset_mesh_material as u64)?;
			for _ in 0..header.num_mesh_material {
				mesh_material.push(MeshMaterial::read(reader)?);
			}
		}


		Ok(Wmb {
			header,
			bones,
			bone_index_translate_table,
			vertex_groups,
			batches,
			lods,
			col_tree_nodes,
			bone_map,
			bone_sets,
			materials,
			meshes,
			mesh_material,
		})
	}
}

struct Header {
	id: String,
	version: u32,
	unknown_a: i32,
	flags: i16,
	reference_bone: i16,
	bounding_box: BoundingBox,
	offset_bones: u32,
	num_bones: u32,
	offset_bone_index_translate_table: u32,
	bone_translate_table_size: u32,
	offset_vertex_groups: u32,
	num_vertex_groups: u32,
	offset_batches: u32,
	num_batches: u32,
	offset_lods: u32,
	num_lods: u32,
	offset_col_tree_nodes: u32,
	num_col_tree_nodes: u32,
	offset_bone_map: u32,
	bone_map_size: u32,
	offset_bone_sets: u32,
	num_bone_sets: u32,
	offset_materials: u32,
	num_materials: u32,
	offset_meshes: u32,
	num_meshes: u32,
	offset_mesh_material: u32,
	num_mesh_material: u32,
	offset_unknown0: u32,
	num_unknown0: u32,
}

impl Header {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(Header {
			id: reader.read_string(4)?,
			version: reader.read_u32()?,
			unknown_a: reader.read_i32()?,
			flags: reader.read_i16()?,
			reference_bone: reader.read_i16()?,
			bounding_box: BoundingBox::read(reader)?,
			offset_bones: reader.read_u32()?,
			num_bones: reader.read_u32()?,
			offset_bone_index_translate_table: reader.read_u32()?,
			bone_translate_table_size: reader.read_u32()?,
			offset_vertex_groups: reader.read_u32()?,
			num_vertex_groups: reader.read_u32()?,
			offset_batches: reader.read_u32()?,
			num_batches: reader.read_u32()?,
			offset_lods: reader.read_u32()?,
			num_lods: reader.read_u32()?,
			offset_col_tree_nodes: reader.read_u32()?,
			num_col_tree_nodes: reader.read_u32()?,
			offset_bone_map: reader.read_u32()?,
			bone_map_size: reader.read_u32()?,
			offset_bone_sets: reader.read_u32()?,
			num_bone_sets: reader.read_u32()?,
			offset_materials: reader.read_u32()?,
			num_materials: reader.read_u32()?,
			offset_meshes: reader.read_u32()?,
			num_meshes: reader.read_u32()?,
			offset_mesh_material: reader.read_u32()?,
			num_mesh_material: reader.read_u32()?,
			offset_unknown0: reader.read_u32()?,
			num_unknown0: reader.read_u32()?,
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
	id: i16,
	parent_index: i16,
	local_position: Point,
	local_rotation: Point,
	local_scale: Point,
	position: Point,
	rotation: Point,
	scale: Point,
	t_position: Point,
}

impl Bone {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(Bone {
			id: reader.read_i16()?,
			parent_index: reader.read_i16()?,
			local_position: Point::read(reader)?,
			local_rotation: Point::read(reader)?,
			local_scale: Point::read(reader)?,
			position: Point::read(reader)?,
			rotation: Point::read(reader)?,
			scale: Point::read(reader)?,
			t_position: Point::read(reader)?,
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
		for i in 0..j*16 {
			second_level.push(reader.read_i16()?);
		}

		let mut k = 0;
		for i in 0..j*16 {
			if second_level[i] != -1 {
				k += 1;
			}
		}

		let mut third_level = Vec::new();
		for i in 0..k*16 {
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
	tangent: Point,
	tangent_sign: f32,
	tangent_length: f32,
	uv: [f32; 2],
	normal: Option<Point>,
	uv2: Option<[f32; 2]>,
	bone_indices: Option<[u8; 4]>,
	bone_weights: Option<[f32; 4]>,
	color: Option<[u8; 4]>,
}

const UV2_FLAGS: &[u32] = &[1, 4, 5, 12, 14];
const BONE_FLAGS: &[u32] = &[7, 10, 11];
const COLOR_FLAGS: &[u32] = &[4, 5, 12, 14];

impl Vertex {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>, vertex_flags: u32) -> Result<Self, String> {
		let position = Point::read(reader)?;
		let tangent = [reader.read_u8()? as f32, reader.read_u8()? as f32, reader.read_u8()? as f32];
		let tangent_sign = reader.read_u8()?;
		let uv = [
			reader.read_f16()?.to_f32(),
			reader.read_f16()?.to_f32()
		];
		let tangent = Point {
			x: (tangent[0] - 127.0) / 127.0,
			y: (tangent[1] - 127.0) / 127.0,
			z: (tangent[2] - 127.0) / 127.0,
		};
		let tangent_sign = (tangent_sign as f32 - 127.0) / 127.0;
		let tangent_length = (tangent.x * tangent.x + tangent.y * tangent.y + tangent.z * tangent.z).sqrt();

		let normal = if vertex_flags == 0 {
			let v = Point{
				x: reader.read_f16()?.to_f32(),
				y: reader.read_f16()?.to_f32(),
				z: reader.read_f16()?.to_f32(),
			};
			reader.read_f16()?;
			Some(v)
		} else {
			None
		};
		let uv2 = if UV2_FLAGS.contains(&vertex_flags) {
			Some([
				reader.read_f16()?.to_f32(),
				reader.read_f16()?.to_f32()
			])
		} else {
			None
		};
		let bone_indices: Option<[u8; 4]>;
		let bone_weights: Option<[f32; 4]>;
		if BONE_FLAGS.contains(&vertex_flags) {
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
		} else {
			bone_indices = None;
			bone_weights = None;
		};
		let color = if COLOR_FLAGS.contains(&vertex_flags) {
			Some([
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
				reader.read_u8()?,
			])
		} else {
			None
		};

		Ok(Vertex {
			position,
			tangent,
			tangent_sign,
			tangent_length,
			uv,
			normal,
			uv2,
			bone_indices,
			bone_weights,
			color,
		})
	}
}

struct VertexGroupHeader {
	vertex_offset: u32,
	vertex_ex_data_offset: u32,
	unknown1_offset: u32,
	unknown2_offset: u32,
	vertex_size: u32,
	vertex_ex_data_size: u32,
	unknown1_size: u32,
	unknown2_size: u32,
	num_vertexes: u32,
	vertex_flags: u32,
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
			vertex_size: reader.read_u32()?,
			vertex_ex_data_size: reader.read_u32()?,
			unknown1_size: reader.read_u32()?,
			unknown2_size: reader.read_u32()?,
			num_vertexes: reader.read_u32()?,
			vertex_flags: reader.read_u32()?,
			index_buffer_offset: reader.read_u32()?,
			num_indexes: reader.read_u32()?,
		})
	}
}

struct VertexExData {
	normal: Option<Point>,
	uv2: Option<[f32; 2]>,
	uv3: Option<[f32; 2]>,
	uv4: Option<[f32; 2]>,
	uv5: Option<[f32; 2]>,
	color: Option<[u8; 4]>,
}

impl VertexExData {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>, vertex_flags: u32) -> Result<Self, String> {
		let normal: Option<Point>;
		let uv2: Option<[f32; 2]>;
		let uv3: Option<[f32; 2]>;
		let uv4: Option<[f32; 2]>;
		let uv5: Option<[f32; 2]>;
		let color: Option<[u8; 4]>;

		match vertex_flags {
			1 | 4 => {
				normal = Some(VertexExData::read_normal(reader)?);
				uv2 = None;
				uv3 = None;
				uv4 = None;
				uv5 = None;
				color = None;
			}
			5 => {
				normal = Some(VertexExData::read_normal(reader)?);
				uv3 = Some(VertexExData::read_uv(reader)?);
				uv2 = None;
				uv4 = None;
				uv5 = None;
				color = None;
			}
			7 => {
				uv2 = Some(VertexExData::read_uv(reader)?);
				normal = Some(VertexExData::read_normal(reader)?);
				uv3 = None;
				uv4 = None;
				uv5 = None;
				color = None;
			}
			10 => {
				uv2 = Some(VertexExData::read_uv(reader)?);
				color = Some(VertexExData::read_color(reader)?);
				normal = Some(VertexExData::read_normal(reader)?);
				uv3 = None;
				uv4 = None;
				uv5 = None;
			}
			11 => {
				uv2 = Some(VertexExData::read_uv(reader)?);
				color = Some(VertexExData::read_color(reader)?);
				normal = Some(VertexExData::read_normal(reader)?);
				uv3 = Some(VertexExData::read_uv(reader)?);
				uv4 = None;
				uv5 = None;
			}
			12 => {
				normal = Some(VertexExData::read_normal(reader)?);
				uv3 = Some(VertexExData::read_uv(reader)?);
				uv4 = Some(VertexExData::read_uv(reader)?);
				uv5 = Some(VertexExData::read_uv(reader)?);
				uv2 = None;
				color = None;
			}
			14 => {
				normal = Some(VertexExData::read_normal(reader)?);
				uv3 = Some(VertexExData::read_uv(reader)?);
				uv4 = Some(VertexExData::read_uv(reader)?);
				uv2 = None;
				uv5 = None;
				color = None;
			}
			_ => {
				return Err(format!("Unknown vertex flags: {}", vertex_flags));
			}
		};

		Ok(VertexExData {
			normal,
			uv2,
			uv3,
			uv4,
			uv5,
			color,
		})
	}

	fn read_normal<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Point, String> {
		let v = Point {
			x: reader.read_f16()?.to_f32(),
			y: reader.read_f16()?.to_f32(),
			z: reader.read_f16()?.to_f32(),
		};
		reader.read_f16()?;
		Ok(v)
	}

	fn read_uv<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<[f32; 2], String> {
		Ok([
			reader.read_f16()?.to_f32(),
			reader.read_f16()?.to_f32(),
		])
	}

	fn read_color<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<[u8; 4], String> {
		Ok([
			reader.read_u8()?,
			reader.read_u8()?,
			reader.read_u8()?,
			reader.read_u8()?,
		])
	}
}

struct VertexGroup {
	vertexes: Vec<Vertex>,
	vertex_ex_data: Vec<VertexExData>,
	indexes: Vec<u32>,
}

impl VertexGroup {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>, header_flags: u32) -> Result<Self, String> {
		let header = VertexGroupHeader::read(reader)?;
		let mut vertexes = Vec::with_capacity(header.num_vertexes as usize);
		let mut vertex_ex_data = Vec::with_capacity(header.num_vertexes as usize);
		let mut indexes = Vec::with_capacity(header.num_indexes as usize);
		let pos = reader.position()?;

		reader.seek(header.vertex_offset as u64)?;
		for _ in 0..header.num_vertexes {
			vertexes.push(Vertex::read(reader, header.vertex_flags)?);
		}

		reader.seek(header.vertex_ex_data_offset as u64)?;
		for _ in 0..header.num_vertexes {
			vertex_ex_data.push(VertexExData::read(reader, header.vertex_flags)?);
		}

		reader.seek(header.index_buffer_offset as u64)?;
		let read_idx_func = if header_flags & 0x8 != 0 {
			ByteReader::read_u32
		} else {
			|reader: &mut ByteReader<R>| reader.read_u16().map(|x| x as u32)
		};
		for _ in 0..header.num_indexes {
			indexes.push(read_idx_func(reader)?);
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
	bone_set_index: i32,
	vertex_start: u32,
	index_start: u32,
	num_vertexes: u32,
	num_indexes: u32,
	num_primitives: u32,
}

impl Batch {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(Batch {
			vertex_group_index: reader.read_u32()?,
			bone_set_index: reader.read_i32()?,
			vertex_start: reader.read_u32()?,
			index_start: reader.read_u32()?,
			num_vertexes: reader.read_u32()?,
			num_indexes: reader.read_u32()?,
			num_primitives: reader.read_u32()?,
		})
	}
}

struct Lod {
	name: String,
	lod_level: i32,
	batch_start: u32,
	batch_infos: Vec<BatchInfo>,
}

impl Lod {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let offset_name = reader.read_u32()?;
		let lod_level = reader.read_i32()?;
		let batch_start = reader.read_u32()?;
		let offset_batch_infos = reader.read_u32()?;
		let num_batch_infos = reader.read_u32()?;
		let pos = reader.position()?;

		reader.seek(offset_name as u64)?;
		let name = reader.read_string_zero_term()?;

		reader.seek(offset_batch_infos as u64)?;
		let mut batch_infos = Vec::with_capacity(num_batch_infos as usize);
		for _ in 0..num_batch_infos {
			batch_infos.push(BatchInfo::read(reader)?);
		}

		reader.seek(pos)?;

		Ok(Lod {
			name,
			lod_level,
			batch_start,
			batch_infos,
		})
	}
}

struct BatchInfo {
	vertex_group_index: u32,
	mesh_index: u32,
	material_index: u32,
	col_tree_node_index: i32,
	mesh_mat_pair_index: u32,
	index_to_unknown1: i32,
}

impl BatchInfo {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(BatchInfo {
			vertex_group_index: reader.read_u32()?,
			mesh_index: reader.read_u32()?,
			material_index: reader.read_u32()?,
			col_tree_node_index: reader.read_i32()?,
			mesh_mat_pair_index: reader.read_u32()?,
			index_to_unknown1: reader.read_i32()?,
		})
	}
}

struct ColTreeNode {
	p1: Point,
	p2: Point,
	left: i32,
	right: i32,
}

impl ColTreeNode {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(ColTreeNode {
			p1: Point::read(reader)?,
			p2: Point::read(reader)?,
			left: reader.read_i32()?,
			right: reader.read_i32()?,
		})
	}
}

struct BoneSet {
	bone_indexes: Vec<i16>,
}

impl BoneSet {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let offset_bone_set = reader.read_u32()?;
		let num_bone_indexes = reader.read_u32()?;
		let pos = reader.position()?;

		reader.seek(offset_bone_set as u64)?;
		let mut bone_indexes = Vec::with_capacity(num_bone_indexes as usize);
		for _ in 0..num_bone_indexes {
			bone_indexes.push(reader.read_i16()?);
		}

		reader.seek(pos)?;

		Ok(BoneSet {
			bone_indexes,
		})
	}
}

struct Material {
	name: String,
	shader_name: String,
	technique_name: String,
	textures: Vec<Texture>,
	parameter_groups: Vec<ParameterGroup>,
	variables: Vec<Variable>,
}

impl Material {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let unknown0 = [
			reader.read_u16()?,
			reader.read_u16()?,
			reader.read_u16()?,
			reader.read_u16()?,
		];
		let offset_name = reader.read_u32()?;
		let offset_shader_name = reader.read_u32()?;
		let offset_technique_name = reader.read_u32()?;
		let unknown1 = reader.read_u32()?;
		let offset_textures = reader.read_u32()?;
		let num_textures = reader.read_u32()?;
		let offset_parameter_groups = reader.read_u32()?;
		let num_parameters_group = reader.read_u32()?;
		let offset_variables = reader.read_u32()?;
		let num_variables = reader.read_u32()?;
		let pos = reader.position()?;

		reader.seek(offset_name as u64)?;
		let name = reader.read_string_zero_term()?;

		reader.seek(offset_shader_name as u64)?;
		let shader_name = reader.read_string_zero_term()?;

		reader.seek(offset_technique_name as u64)?;
		let technique_name = reader.read_string_zero_term()?;

		reader.seek(offset_textures as u64)?;
		let mut textures = Vec::with_capacity(num_textures as usize);
		for _ in 0..num_textures {
			textures.push(Texture::read(reader)?);
		}

		reader.seek(offset_parameter_groups as u64)?;
		let mut parameter_groups = Vec::with_capacity(num_parameters_group as usize);
		for _ in 0..num_parameters_group {
			parameter_groups.push(ParameterGroup::read(reader)?);
		}

		reader.seek(offset_variables as u64)?;
		let mut variables = Vec::with_capacity(num_variables as usize);
		for _ in 0..num_variables {
			variables.push(Variable::read(reader)?);
		}

		reader.seek(pos)?;

		Ok(Material {
			name,
			shader_name,
			technique_name,
			textures,
			parameter_groups,
			variables,
		})
	}

	fn get_albedo_texture_id(&self) -> Option<u32> {
		for texture in &self.textures {
			if texture.name.contains("g_AlbedoMap") {
				return Some(texture.id);
			}
		}
		None
	}

	fn get_normal_texture_id(&self) -> Option<u32> {
		for texture in &self.textures {
			if texture.name.contains("g_NormalMap") {
				return Some(texture.id);
			}
		}
		None
	}

	fn get_mask_map_texture_id(&self) -> Option<u32> {
		for texture in &self.textures {
			if texture.name.contains("g_MaskMap") {
				return Some(texture.id);
			}
		}
		None
	}
}

struct Texture {
	id: u32,
	name: String,
}

impl Texture {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let offset_name = reader.read_u32()?;
		let id = reader.read_u32()?;
		let pos = reader.position()?;

		reader.seek(offset_name as u64)?;
		let name = reader.read_string_zero_term()?;

		reader.seek(pos)?;

		Ok(Texture {
			id,
			name,
		})
	}
}

struct ParameterGroup {
	index: i32,
	parameters: Vec<f32>,
}

impl ParameterGroup {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let index = reader.read_i32()?;
		let offset_parameters = reader.read_u32()?;
		let num_parameters = reader.read_u32()?;
		let pos = reader.position()?;

		reader.seek(offset_parameters as u64)?;
		let mut parameters = Vec::with_capacity(num_parameters as usize);
		for _ in 0..num_parameters {
			parameters.push(reader.read_f32()?);
		}

		reader.seek(pos)?;

		Ok(ParameterGroup {
			index,
			parameters,
		})
	}
}

struct Variable {
	name: String,
	value: f32,
}

impl Variable {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let offset_name = reader.read_u32()?;
		let value = reader.read_f32()?;
		let pos = reader.position()?;

		reader.seek(offset_name as u64)?;
		let name = reader.read_string_zero_term()?;

		reader.seek(pos)?;

		Ok(Variable {
			name,
			value,
		})
	}
}

struct Mesh {
	name: String,
	bounding_box: BoundingBox,
	materials: Vec<u16>,
	bones: Vec<u16>,
}

impl Mesh {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		let name_offset = reader.read_u32()?;
		let bounding_box = BoundingBox::read(reader)?;
		let offset_materials = reader.read_u32()?;
		let num_materials = reader.read_u32()?;
		let offset_bones = reader.read_u32()?;
		let num_bones = reader.read_u32()?;
		let pos = reader.position()?;

		reader.seek(name_offset as u64)?;
		let name = reader.read_string_zero_term()?;

		reader.seek(offset_materials as u64)?;
		let mut materials = Vec::with_capacity(num_materials as usize);
		for _ in 0..num_materials {
			materials.push(reader.read_u16()?);
		}

		reader.seek(offset_bones as u64)?;
		let mut bones = Vec::with_capacity(num_bones as usize);
		for _ in 0..num_bones {
			bones.push(reader.read_u16()?);
		}

		reader.seek(pos)?;

		Ok(Mesh {
			name,
			bounding_box,
			materials,
			bones,
		})
	}
}

struct MeshMaterial {
	mesh_id: u32,
	material_id: u32,
}

impl MeshMaterial {
	fn read<R: Read + Seek>(reader: &mut ByteReader<R>) -> Result<Self, String> {
		Ok(MeshMaterial {
			mesh_id: reader.read_u32()?,
			material_id: reader.read_u32()?,
		})
	}
}

fn decompress_dds(bytes: Vec<u8>, swizzle: Option<&dyn Fn(&mut[u8]) -> ()>) -> Result<TextureData, String> {
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
}

fn mask_map_swizzle(pixel: &mut[u8]) {
    // in
    // R: metallic
    // G: smoothness
    // B: AO
    // out
    // R: AO
    // G: roughness
    // B: metallic
    pixel.swap(0, 2);
    pixel[1] = 255 - pixel[1];
}

fn try_add_texture(
	textures: &mut HashMap<u32, TextureData>,
	wta_wtp: &mut WtaWtp<BufReader<File>>,
	texture_id: Option<u32>,
	swizzle: Option<&dyn Fn(&mut[u8]) -> ()>,
) -> () {
	texture_id
		.and_then(|texture_id| wta_wtp.get_texture(texture_id))
		.and_then(|texture| decompress_dds(texture, swizzle).ok())
		.map(|texture_data| textures.insert(texture_id.unwrap(), texture_data));
}
