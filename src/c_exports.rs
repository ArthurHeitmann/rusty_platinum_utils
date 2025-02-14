use std::ffi::{c_char, CStr};

use three_d::WindowedContext;

use crate::{mesh_data::SceneData, mesh_renderer::{new_context, RenderState}, wmb_scr::read_wmb_scr};


#[no_mangle]
pub extern fn rpu_load_wmb(wmb_path: *const c_char) -> *mut SceneData {
	let wmb_path = unsafe { CStr::from_ptr(wmb_path) }.to_string_lossy().into_owned();
	match read_wmb_scr(wmb_path) {
		Ok(scene_data) => Box::into_raw(Box::new(scene_data)),
		Err(e) => {
			eprintln!("{}", e);
    		std::ptr::null_mut()
		},
	}
}


#[no_mangle]
pub extern fn rpu_new_context() -> *mut WindowedContext {
	match new_context() {
		Ok(context) => Box::into_raw(Box::new(context)),
		Err(e) => {
			eprintln!("{}", e);
			std::ptr::null_mut()
		}
	}
}

#[no_mangle]
pub extern fn rpu_new_renderer(
	context: *mut WindowedContext,
	width: u32,
	height: u32,
	scene_data: *mut SceneData,
) -> *mut RenderState<'static> {
	let context = unsafe { &*context };
	let scene_data = unsafe { Box::from_raw(scene_data) };
	let scene_data = *scene_data;
	let state = RenderState::new(context, width, height, scene_data);
	match state {
		Ok(state) => Box::into_raw(Box::new(state)),
		Err(_) => std::ptr::null_mut(),
	}
}

#[no_mangle]
pub extern fn rpu_drop_renderer(state: *mut RenderState) {
	unsafe {
		drop(Box::from_raw(state));
	}
}

#[no_mangle]
pub extern fn rpu_render(
	state: *mut RenderState,
	buffer: *mut u8, buffer_size: usize,
	width: u32, height: u32,
	bg_r: f32, bg_g: f32, bg_b: f32, bg_a: f32,
) -> i32 {
	let mut pixels_buffer = unsafe {
		(*state).render(width, height, bg_r, bg_g, bg_b, bg_a)
	};
	if pixels_buffer.len() > buffer_size {
		return -1;
	}
	unsafe {
		pixels_buffer.as_mut_ptr().copy_to_nonoverlapping(buffer, pixels_buffer.len());
	}
	pixels_buffer.len() as i32
}

#[no_mangle]
pub extern fn rpu_add_camera_rotation(state: *mut RenderState, x: f32, y: f32) {
	unsafe {
		(*state).add_camera_rotation(x, y);
	}
}

#[no_mangle]
pub extern fn rpu_add_camera_offset(state: *mut RenderState, x: f32, y: f32) {
	unsafe {
		(*state).add_camera_offset(x, y);
	}
}

#[no_mangle]
pub extern fn rpu_zoom_camera_by(state: *mut RenderState, distance: f32) {
	unsafe {
		(*state).zoom_camera_by(distance);
	}
}

#[no_mangle]
pub extern fn rpu_auto_set_target(state: *mut RenderState) {
	unsafe {
		(*state).auto_set_target();
	}
}

#[no_mangle]
pub extern  fn rpu_set_model_visibility(state: *mut RenderState, model_id: u32, visibility: bool) {
	unsafe {
		(*state).set_model_visibility(model_id, visibility);
	}
}

#[no_mangle]
pub extern  fn rpu_get_model_states(state: *mut RenderState) -> *const char {
	unsafe {
		(*state).model_states.as_ptr() as *const char
	}
}
