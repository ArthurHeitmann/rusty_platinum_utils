use std::{collections::HashMap, io::{Cursor, Read, Seek}};

use three_d::Matrix4;

use crate::{byte_stream::ByteReader, mesh_data::{MeshData, TextureData}, wmb::read_wmb, wta_wtp::WtaWtp};


pub fn read_scr_mgrr<R1: Read + Seek, R2: Read + Seek>(
	reader: &mut ByteReader<R1>,
	wta_wtp: &mut Option<WtaWtp<R2>>,
	textures: &mut HashMap<u32, TextureData>,
) -> Result<Vec<MeshData>, String> {
	reader.seek(6)?;
	let num_models = reader.read_u16()?;
	let offsets_offset = reader.read_u32()?;
	reader.seek(offsets_offset as u64)?;
	let mut offsets = Vec::new();
	for _ in 0..num_models {
		offsets.push(reader.read_u32()? as u64);
	}
	let mut meshes = Vec::new();
	for (i, offset) in offsets.iter().enumerate() {
		reader.seek(*offset as u64)?;
		let wmb_offset = reader.read_u32()? as u64;
		let wmb_name = reader.read_string(64)?;
		let wmb_name = wmb_name.replace('\0', "");
		let translation = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
		let rotation = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
		let scale = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
		let translation = Matrix4::from_translation(translation.into());
		let rotation_x = Matrix4::from_angle_x(three_d::Rad(rotation[0]));
		let rotation_y = Matrix4::from_angle_y(three_d::Rad(rotation[1]));
		let rotation_z = Matrix4::from_angle_z(three_d::Rad(rotation[2]));
		let rotation = rotation_x * rotation_y * rotation_z;
		let scale = Matrix4::from_nonuniform_scale(scale[0], scale[1], scale[2]);
		let transform = translation * rotation * scale;

		reader.seek(wmb_offset)?;
		let size = if i + 1 < num_models as usize {
			offsets[i + 1] - wmb_offset
		} else {
			reader.size()? - wmb_offset
		};
		let wmb_bytes = reader.read(size as usize)?;
		let cursor = Cursor::new(wmb_bytes);
		let mut wmb_reader = ByteReader::new(cursor);
		
		let mut mesh_datas = read_wmb(&wmb_name, &mut wmb_reader, wta_wtp, textures)?;
		for mesh_data in mesh_datas.iter_mut() {
			mesh_data.name = wmb_name.clone();
			mesh_data.transform = transform;
		}

		meshes.append(&mut mesh_datas);
	}
	Ok(meshes)
}
