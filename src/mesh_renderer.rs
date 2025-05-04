use std::{collections::HashMap, rc::Rc, sync::Arc, time::{Duration, Instant}};

// use rand::Rng;
use winit::{event_loop::EventLoop, window::WindowBuilder};
use crate::mesh_data::SceneData;
use three_d::*;
use three_d::window::WindowedContext;



pub fn new_context() -> Result<WindowedContext, String> {
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new()
		.with_visible(false)
		.build(&event_loop)
		.map_err(|e| e.to_string())?;
	let context = WindowedContext::from_winit_window(
		&window,
		SurfaceSettings{
			vsync: false,
			..Default::default()
		},
	)
		.map_err(|e| e.to_string())?;
	Ok(context)
}

type Mat = DeferredPhysicalMaterial;

struct ModelInfo {
	model: Gm<Mesh, Mat>,
	transform: Matrix4<f32>,
}

pub struct RenderState {
	context: Rc<WindowedContext>,
	camera: Camera,
	models: HashMap<u32, ModelInfo>,
	pub model_states: String,
	ambient_light: AmbientLight,
	directional_light: DirectionalLight,
	pitch: f32,
	yaw: f32,
}

impl RenderState {
	pub fn new(context: Rc<WindowedContext>, width: u32, height: u32, scene_data: SceneData) -> Result<Self, String> {
		// let t1 = Instant::now();
		// let mut tex_times: Vec<Duration> = Vec::new();
		// let mut mat_times: Vec<Duration> = Vec::new();
		// let mut gm_times: Vec<Duration> = Vec::new();
		let mut models = HashMap::new();
		let mut model_states = String::new();
		let mut bounding_box = AxisAlignedBoundingBox::EMPTY;
		let mut textures: HashMap<u32, Arc<Texture2D>> = HashMap::new();
		for (i, mesh_data) in scene_data.meshes.into_iter().enumerate() {
			bounding_box.expand(mesh_data.vertices.as_slice());
			let cpu_mesh = CpuMesh {
				positions: Positions::F32(mesh_data.vertices),
				indices: Indices::U32(mesh_data.indexes),
				normals: mesh_data.normals,
				tangents: mesh_data.tangents,
				uvs: Some(mesh_data.uv.iter().map(|v| Vec2::new(v.x, v.y)).collect()),
				..Default::default()
			};
		
			// let sub_t1 = Instant::now();
			let albedo_texture = lookup_texture(&context, mesh_data.albedo_texture_id, &mut textures, &scene_data.textures);
			let normal_texture = lookup_texture(&context, mesh_data.normal_texture_id, &mut textures, &scene_data.textures);
			// tex_times.push(sub_t1.elapsed());
			// let sub_t1 = Instant::now();
			let material = Mat {
				// albedo: _random_color(),
				albedo_texture,
				normal_texture,
				alpha_cutout: if mesh_data.uses_transparency { Some(0.5) } else { None },
				..Default::default()
			};
			// mat_times.push(sub_t1.elapsed());
		
			// let sub_t1 = Instant::now();
			let mut model = Gm::new(Mesh::new(&context, &cpu_mesh), material);
			if mesh_data.should_be_visible {
				model.set_transformation(Matrix4::from(mesh_data.transform));
			}
			else {
				model.set_transformation(Matrix4::from_scale(0.0));
			}
			let model_info = ModelInfo {
				model,
				transform: Matrix4::from(mesh_data.transform),
			};
			models.insert(i as u32, model_info);
			// gm_times.push(sub_t1.elapsed());

			model_states.push_str(&format!("{},{},{}\n", i, mesh_data.name, mesh_data.should_be_visible));
		}
		model_states.push('\0');
		if bounding_box.is_empty() {
			bounding_box = AxisAlignedBoundingBox::new_with_positions(&[vec3(-1.0, -1.0, -1.0), vec3(1.0, 1.0, 1.0)]);
		}
		// let t2 = Instant::now();
		// let tex_time = tex_times.iter().sum::<Duration>();
		// println!("create models: {:?} (tex: {:?}, mat: {:?}, gm: {:?})", t2.duration_since(t1), tex_time, mat_times.iter().sum::<Duration>(), gm_times.iter().sum::<Duration>());

		let ambient_light = AmbientLight::new(&context, 0.3, Srgba::WHITE);
		let directional_light = DirectionalLight::new(&context, 3.0, Srgba::WHITE, vec3(-1.0, -1.0, -1.0));
		
		let center = bounding_box.center();
		let radius = bounding_box.distance_max(center);
		let fov = (45.0_f32).to_radians();
		let distance = radius / (fov / 2.0).tan();
		let distance = distance.max(0.1);
		let camera = Camera::new_perspective(
			Viewport::new_at_origo(width, height),
			center + Vector3::unit_z() * distance,
			center,
			vec3(0.0, 1.0, 0.0),
			radians(fov),
			distance / 4000.0,
			distance * 4.0,
		);	
		
		let mut render_state = Self {
			context,
			camera,
			models,
			model_states,
			ambient_light,
			directional_light,
			pitch: 0.0,
			yaw: 0.0,
		};
		render_state.add_camera_rotation((-20.0_f32).to_radians(), (30.0_f32).to_radians());

		Ok(render_state)
	}

	pub fn render(&mut self, width: u32, height: u32, bg_r: f32, bg_g: f32, bg_b: f32, bg_a: f32) -> Vec<u8> {
		let mut texture = Texture2D::new_empty::<[u8; 4]>(
			&self.context,
			width,
			height,
			Interpolation::Linear,
			Interpolation::Linear,
			None,
			Wrapping::ClampToEdge,
			Wrapping::ClampToEdge,
		);

		let mut depth_texture = DepthTexture2D::new::<f32>(
			&self.context,
			width,
			height,
			Wrapping::ClampToEdge,
			Wrapping::ClampToEdge,
		);

		let render_target = RenderTarget::new(
			texture.as_color_target(None),
			depth_texture.as_depth_target(),
		);
		self.camera.set_viewport(Viewport::new_at_origo(width, height));
		render_target.clear(ClearState::color_and_depth(bg_r, bg_g, bg_b, bg_a, 1.0));
		let models = self.models.values().map(|m| &m.model).collect::<Vec<_>>();
		self.directional_light.generate_shadow_map(2048, &models);
		render_target.render(&self.camera, &models, &self.lights());
		render_target.read_color::<[u8; 4]>().into_flattened()
		// render_target.read_depth().into_iter().map(|f| [(f * 100.0) as u8, (f * 100.0) as u8, (f * 100.0) as u8, 255]).flatten().collect()
	}

	pub fn add_camera_rotation(&mut self, x: f32, y: f32) {
		self.pitch += x;
		self.yaw += y;
		let r_x = Matrix3::from_angle_x(Rad(self.pitch));
		let r_y = Matrix3::from_angle_y(Rad(self.yaw));
		let position = self.camera.position();
		let target = self.camera.target();
		let offset = position - target;
		let distance = offset.magnitude();
		let new_offset = r_y * r_x * Vector3::unit_z() * distance;
		let up = r_y * r_x * Vector3::unit_y();
		let new_position = target + new_offset;
		self.camera.set_view(new_position, target, up);
	}

	pub fn add_camera_offset(&mut self, x: f32, y: f32) {
		let cam_right = self.camera.right_direction();
		let cam_up = self.camera.up_orthogonal();
		let offset = cam_right * x + cam_up * y;
		let distance = (self.camera.position() - self.camera.target()).magnitude();
		let offset = offset * distance;
		self.camera.translate(offset);
	}

	pub fn zoom_camera_by(&mut self, distance: f32) {
		let cur_distance = (self.camera.position() - self.camera.target()).magnitude();
		let distance_factor = if cur_distance < 5.0 {
			(cur_distance / 5.0).max(0.01)
		} else {
			cur_distance / 5.0
		};
		self.camera.zoom(distance * distance_factor, 0.1, 10000.0);
	}

	pub fn auto_set_target(&mut self) {
		let mid_pos = renderer::ray_intersect(
			&self.context,
			self.camera.position(),
			self.camera.view_direction(),
			999999.0,
			self.models.values().map(|m| &m.model).collect::<Vec<_>>(),
		);
		if let Some(mid_pos) = mid_pos {
			let cam_pos = self.camera.position();
			let cam_up = self.camera.up();
			self.camera.set_view(
				cam_pos,
				mid_pos.position,
				cam_up,
			);
		}
	}

	pub fn set_model_visibility(&mut self, id: u32, visible: bool) {
		if let Some(model) = self.models.get_mut(&id) {
			if visible {
				model.model.set_transformation(model.transform);
			} else {
				model.model.set_transformation(Matrix4::from_scale(0.0));
			}
		}
	}

	fn lights(&self) -> [&dyn Light; 2] {
		[&self.ambient_light, &self.directional_light]
	}
}

// fn _random_color() -> Srgba {
//     let mut rng = rand::rng();
//     Srgba::new(
//         rng.random_range(0..255),
//         rng.random_range(0..255),
//         rng.random_range(0..255),
//         255,
//     )
// }

fn tex_data_to_tex2d(context: &Context, tex_data: crate::mesh_data::TextureData) -> Result<Texture2D, String> {
	let pixel_buffer = tex_data.bytes
		.chunks(4)
		.map(|chunk| [chunk[0], chunk[1], chunk[2], chunk[3]])
		.collect::<Vec<_>>();
	let mut cpu_tex = CpuTexture {
		name: "dds".to_string(),
		data: TextureData::RgbaU8(pixel_buffer),
		width: tex_data.width,
		height: tex_data.height,
		min_filter: Interpolation::Linear,
		mag_filter: Interpolation::Linear,
		mipmap: Some(Mipmap{
			filter: Interpolation::Linear,
			max_levels: 4,
			max_ratio: 4,
		}),
		wrap_s: Wrapping::Repeat,
		wrap_t: Wrapping::Repeat,
	};
	cpu_tex.data.to_linear_srgb();
	Ok(Texture2D::new(context, &cpu_tex))
}

fn lookup_texture(
	context: &Context,
	texture_id: Option<u32>,
	texture_handles: &mut HashMap<u32, Arc<Texture2D>>,
	textures: &HashMap<u32, crate::mesh_data::TextureData>,
) -> Option<Texture2DRef> {
	let tex = if let Some(id) = texture_id {
		texture_handles.get(&id).cloned()
	} else {
		None
	};
	let tex = tex.or_else(|| {
		texture_id.and_then(|id| {
			let tex_data = textures.get(&id)?;
			let tex_handle = tex_data_to_tex2d(context, tex_data.clone()).ok()?;
			texture_handles.insert(id, Arc::new(tex_handle));
			Some(texture_handles[&id].clone())
		})
	});
	tex.map(|t| Texture2DRef{
		texture: t.clone(),
		transformation: Matrix3::identity(),
	})
}
