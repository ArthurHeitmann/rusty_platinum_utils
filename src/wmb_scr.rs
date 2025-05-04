use std::{collections::HashMap, fs::File, io::{BufReader, Cursor}, time::Instant};

use crate::{byte_stream::ByteReader, mesh_data::{SceneData, TextureData}, scr_mgrr::read_scr_mgrr, wmb_mgrr::read_wmb_mgrr, wmb_na::read_wmb_na, wta_wtp::{WtaBasenameExt, WtaWtp}};


pub fn read_wmb_scr(path: String) -> Result<SceneData, String> {
	// let t1 = Instant::now();
	let file = File::open(&path).map_err(|e| e.to_string())?;
	let mut reader = ByteReader::new(BufReader::new(file));
	let magic = reader.read_string(4)?;
	reader.seek(0)?;
	let mut wta_wtp = WtaWtp::from_wmb(&path, WtaBasenameExt::Maybe("scr".to_string())).ok();
	let mut textures: HashMap<u32, TextureData> = HashMap::new();
	let meshes = match magic.as_str() {
		"WMB3" => read_wmb_na(&path, &mut reader, &mut wta_wtp, &mut textures),
		"WMB4" => read_wmb_mgrr(&mut reader, &mut wta_wtp, &mut textures),
		"SCR\0" => read_scr_mgrr(&mut reader, &mut wta_wtp, &mut textures),
		_ => Err(format!("Unknown WMB version: {}", magic)),
	}?;

	// println!("WMB read time: {:?}", t1.elapsed());
	Ok(SceneData {
		meshes: meshes,
		textures,
	})
}

pub fn read_wmb_scr_from_bytes(name: &str, wmb: &[u8], wta_wtb: Option<&[u8]>, wtp: Option<&[u8]>) -> Result<SceneData, String> {
	// let t1 = Instant::now();
	let mut reader = ByteReader::new(Cursor::new(wmb));
	let magic = reader.read_string(4)?;
	reader.seek(0)?;
	let mut wta_wtp = WtaWtp::from_bytes(wta_wtb, wtp).ok();
	let mut textures: HashMap<u32, TextureData> = HashMap::new();
	let meshes = match magic.as_str() {
		"WMB3" => read_wmb_na(name, &mut reader, &mut wta_wtp, &mut textures),
		"WMB4" => read_wmb_mgrr(&mut reader, &mut wta_wtp, &mut textures),
		"SCR\0" => read_scr_mgrr(&mut reader, &mut wta_wtp, &mut textures),
		_ => Err(format!("Unknown WMB version: {}", magic)),
	}?;

	// println!("WMB read time: {:?}", t1.elapsed());
	Ok(SceneData {
		meshes: meshes,
		textures,
	})
}
