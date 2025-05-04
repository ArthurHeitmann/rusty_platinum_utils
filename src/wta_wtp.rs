use std::{collections::HashMap, fs::File, io::{BufReader, Cursor, Read, Seek, SeekFrom}, path::Path};

use crate::byte_stream::ByteReader;


pub struct WtaWtp<F: Read + Seek> {
	id_offsets: HashMap<u32, TexturePos>,
	wtp_file: BufReader<F>,
}

pub struct TexturePos {
	pub offset: u32,
	pub size: u32,
}

pub enum WtaBasenameExt {
	Maybe(String),
	Try(String),
	No,
}

impl WtaWtp<File> {
	pub fn from_wmb(wmb_path_orig: &String, basename_ext: WtaBasenameExt) -> Result<Self, String> {
		let wmb_path = Path::new(wmb_path_orig);
		
		let mut base_name = wmb_path.file_stem()
			.ok_or("Invalid wmb path")?
			.to_str()
			.ok_or("Invalid wmb path")?
			.to_string();
		if let WtaBasenameExt::Try(ext) = &basename_ext {
			base_name.push_str(ext);
		}
		let dir = wmb_path.parent().ok_or("Invalid wmb path")?;
		let dir_extension = dir.extension().ok_or("Invalid wmb path")?;
		let dat_dir;
		let dtt_dir;
		if dir_extension == "dat" {
			dat_dir = dir.to_path_buf();
			dtt_dir = dir.with_extension("dtt");
		} else if dir_extension == "dtt" {
			dat_dir = dir.with_extension("dat");
			dtt_dir = dir.to_path_buf();
		} else {
			return Err("WMB not in a DAT or DTT directory".to_string());
		}
		if !dat_dir.is_dir() || !dtt_dir.is_dir() {
			return Err("DAT or DTT directory not found".to_string());
		}
		let wta_path = dat_dir.join(format!("{base_name}.wta"));
		let wtp_path = dtt_dir.join(format!("{base_name}.wtp"));
		let wtb_path = dtt_dir.join(format!("{base_name}.wtb"));
		let wta_exists = wta_path.exists();
		let wtp_exists = wtp_path.exists();
		let wtb_exists = wtb_path.exists();
		if (!wta_exists || !wtp_exists) && !wtb_exists {
			if let WtaBasenameExt::Maybe(ext) = basename_ext {
				return Self::from_wmb(wmb_path_orig, WtaBasenameExt::Try(ext));
			}
			return Err("WTA, WTP or WTB file not found".to_string());
		}

		let id_offsets = if wta_exists {
			let wta_file = File::open(wta_path).map_err(|e| e.to_string())?;
			read_wta(BufReader::new(wta_file))?
		} else {
			let wtb_file = File::open(&wtb_path).map_err(|e| e.to_string())?;
			read_wta(BufReader::new(wtb_file))?
		};

		let wtp_file = if wta_exists {
			File::open(wtp_path).map_err(|e| e.to_string())?
		} else {
			File::open(wtb_path).map_err(|e| e.to_string())?
		};
		let wtp_reader = BufReader::new(wtp_file);

		Ok(WtaWtp { id_offsets, wtp_file: wtp_reader })
	}
}

impl<'a> WtaWtp<Cursor<&'a[u8]>> {
	pub fn from_bytes(wta_wtb: Option<&'a[u8]>, wtp: Option<&'a[u8]>) -> Result<Self, String> {
		let (id_offsets, wtp_reader) = match (wta_wtb, wtp) {
			(Some(wta), Some(wtp)) => {
				let wta_cursor = Cursor::new(wta);
				let wtp_cursor = Cursor::new(wtp);
				let id_offsets = read_wta(BufReader::new(wta_cursor))?;
				(id_offsets, BufReader::new(wtp_cursor))
			}
			(Some(wta), None) => {
				let wta_cursor = Cursor::new(&wta);
				let id_offsets = read_wta(BufReader::new(wta_cursor))?;
				let wtp_cursor = Cursor::new(wta);
				(id_offsets, BufReader::new(wtp_cursor))
			}
			_ => return Err("WTA or WTP file not found".to_string()),
		};

		Ok(WtaWtp { id_offsets, wtp_file: wtp_reader })
	}
}
	
impl<F: Read + Seek> WtaWtp<F> {
	pub fn get_texture(&mut self, id: u32) -> Option<Vec<u8>> {
		let pos = self.id_offsets.get(&id)?;
		self.wtp_file.seek(SeekFrom::Start(pos.offset as u64)).ok()?;
		let mut buffer = vec![0; pos.size as usize];
		self.wtp_file.read_exact(&mut buffer).ok()?;
		Some(buffer)
	}

	pub fn has_id(&self, id: u32) -> bool {
		self.id_offsets.contains_key(&id)
	}
}

fn read_wta<R: Read + Seek>(wta_file: BufReader<R>) -> Result<HashMap<u32, TexturePos>, String> {
	let mut wta_reader = ByteReader::new(BufReader::new(wta_file));
	wta_reader.seek(8)?;
	let tex_count = wta_reader.read_i32()? as u64;
	let offset_offsets = wta_reader.read_i32()? as u64;
	let offset_sizes = wta_reader.read_i32()? as u64;
	let _offset_flags = wta_reader.read_i32()? as u64;
	let offset_ids = wta_reader.read_i32()? as u64;
	let mut id_offsets = HashMap::new();
	for i in 0..tex_count {
		wta_reader.seek(offset_offsets + i * 4)?;
		let offset = wta_reader.read_u32()?;
	
		wta_reader.seek(offset_sizes + i * 4)?;
		let size = wta_reader.read_u32()?;
	
		wta_reader.seek(offset_ids + i * 4)?;
		let id = wta_reader.read_u32()?;
	
		id_offsets.insert(id, TexturePos { offset, size });
	}
	Ok(id_offsets)
}
