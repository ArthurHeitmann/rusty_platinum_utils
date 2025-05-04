use std::{collections::HashMap, io::{Read, Seek}};

use crate::{byte_stream::ByteReader, mesh_data::{MeshData, TextureData}, wmb_mgrr::read_wmb_mgrr, wmb_na::read_wmb_na, wta_wtp::WtaWtp};


pub fn read_wmb<R1: Read + Seek, R2: Read + Seek>(
	name: &str,
	reader: &mut ByteReader<R1>,
	wta_wtp: &mut Option<WtaWtp<R2>>,
	textures: &mut HashMap<u32, TextureData>,
) -> Result<Vec<MeshData>, String> {
	let magic = reader.read_string(4)?;
	reader.seek(0)?;
	match magic.as_str() {
		"WMB3" => read_wmb_na(name, reader, wta_wtp, textures),
		"WMB4" => read_wmb_mgrr(reader, wta_wtp, textures),
		_ => Err(format!("Unknown WMB version: {}", magic)),
	}
}