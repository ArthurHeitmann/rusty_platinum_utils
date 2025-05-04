#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
extern crate console_error_panic_hook;
use std::{alloc::{alloc, dealloc, Layout}, ffi::{c_char, c_void, CStr}, mem, ptr, rc::Rc};

use three_d::WindowedContext;

use crate::{mesh_data::SceneData, mesh_renderer::{new_context, RenderState}, wmb_scr::{read_wmb_scr, read_wmb_scr_from_bytes}};


#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_load_wmb_from_path(wmb_path: *const c_char) -> *mut SceneData {
	let wmb_path = unsafe { CStr::from_ptr(wmb_path) }.to_string_lossy().into_owned();
	match read_wmb_scr(wmb_path) {
		Ok(scene_data) => Box::into_raw(Box::new(scene_data)),
		Err(e) => {
			eprintln!("{}", e);
    		std::ptr::null_mut()
		},
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_load_wmb_from_bytes(
	name: *const c_char,
	wmb: *const u8, wmb_size: usize,
	wta_wtb: *const u8, wta_wtb_size: usize,
	wtp: *const u8, wtp_size: usize,
) -> *mut SceneData {
	let name = unsafe { CStr::from_ptr(name) }.to_str().unwrap_or("");
	let wmb = unsafe { std::slice::from_raw_parts(wmb, wmb_size) };
	let wta_wtb = if wta_wtb.is_null() || wta_wtb_size == 0 {
		None
	} else {
		Some(unsafe { std::slice::from_raw_parts(wta_wtb, wta_wtb_size) } )
	};
	let wtp = if wtp.is_null() || wtp_size == 0 {
		None
	} else {
		Some(unsafe { std::slice::from_raw_parts(wtp, wtp_size) } )
	};
	match read_wmb_scr_from_bytes(name, wmb, wta_wtb, wtp) {
		Ok(scene_data) => Box::into_raw(Box::new(scene_data)),
		Err(e) => {
			eprintln!("{}", e);
    		std::ptr::null_mut()
		},
	}
}


#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_new_context() -> *mut Rc<WindowedContext> {
	console_error_panic_hook::set_once();

	match new_context() {
		Ok(context) => Box::into_raw(Box::new(Rc::new(context))),
		Err(e) => {
			eprintln!("{}", e);
			std::ptr::null_mut()
		}
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_new_renderer(
	context: *mut Rc<WindowedContext>,
	width: u32,
	height: u32,
	scene_data: *mut SceneData,
) -> *mut RenderState {
	let context = unsafe { &*context };
	let scene_data = unsafe { Box::from_raw(scene_data) };
	let scene_data = *scene_data;
	let state = RenderState::new(context.clone(), width, height, scene_data);
	match state {
		Ok(state) => Box::into_raw(Box::new(state)),
		Err(_) => std::ptr::null_mut(),
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_drop_renderer(state: *mut RenderState) {
	unsafe {
		drop(Box::from_raw(state));
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_render(
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_add_camera_rotation(state: *mut RenderState, x: f32, y: f32) {
	unsafe {
		(*state).add_camera_rotation(x, y);
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_add_camera_offset(state: *mut RenderState, x: f32, y: f32) {
	unsafe {
		(*state).add_camera_offset(x, y);
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_zoom_camera_by(state: *mut RenderState, distance: f32) {
	unsafe {
		(*state).zoom_camera_by(distance);
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn rpu_auto_set_target(state: *mut RenderState) {
	unsafe {
		(*state).auto_set_target();
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C"  fn rpu_set_model_visibility(state: *mut RenderState, model_id: u32, visibility: bool) {
	unsafe {
		(*state).set_model_visibility(model_id, visibility);
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C"  fn rpu_get_model_states(state: *mut RenderState) -> *const char {
	unsafe {
		(*state).model_states.as_ptr() as *const char
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn malloc(size: usize) -> *mut c_void {
    if size == 0 {
        // Consistent with some malloc implementations: return null for zero size.
        return ptr::null_mut();
    }

    // We need to store the size of the allocation alongside the data.
    // We'll store `size: usize` just before the block returned to the user.
    let header_size = mem::size_of::<usize>();
    // Use alignment matching usize for the header, which is often sufficient.
    // More robust: Use max(mem::align_of::<usize>(), required_data_align)
    let align = mem::align_of::<usize>();

    // Calculate total size needed, checking for overflow.
    let total_size = match header_size.checked_add(size) {
        Some(s) => s,
        None => return ptr::null_mut(), // Size overflow
    };

    // Create the layout for the combined header + data block.
    let layout = match Layout::from_size_align(total_size, align) {
        Ok(l) => l,
        Err(_) => return ptr::null_mut(), // Could not create layout (e.g., size too big)
    };

    unsafe {
        // Allocate the memory.
        let block_ptr = alloc(layout);
        if block_ptr.is_null() {
            // Allocation failed.
            return ptr::null_mut();
        }

        // Write the original requested size into the header part of the block.
        // Treat the block_ptr as *mut usize to write the size.
        *(block_ptr as *mut usize) = size;

        // Calculate the pointer to the data part (just after the header).
        // Pointer arithmetic works on *mut u8.
        let user_ptr = block_ptr.add(header_size);

        // Return the pointer to the data part as *mut c_void.
        user_ptr as *mut c_void
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[no_mangle]
pub extern "C" fn free(ptr: *mut c_void) {
    if ptr.is_null() {
        // Freeing a null pointer is a no-op, same as in C.
        return;
    }

    let header_size = mem::size_of::<usize>();
    // Must use the same alignment as `malloc` did.
    let align = mem::align_of::<usize>();

    // Cast to *mut u8 for pointer arithmetic.
    let user_ptr_u8 = ptr as *mut u8;

    unsafe {
        // Calculate the pointer to the *start* of the allocated block (the header).
        // `offset` is slightly safer than `sub` for pointer arithmetic.
        let block_ptr = user_ptr_u8.offset(-(header_size as isize));

        // Read the original requested size from the header.
        let original_size = *(block_ptr as *mut usize);

        // Calculate the total size of the block (header + data).
        let total_size = header_size + original_size; // Assume no overflow if malloc succeeded

        // Reconstruct the layout used for allocation.
        // Use unchecked version assuming malloc created a valid layout if ptr is not null.
        let layout = Layout::from_size_align_unchecked(total_size, align);

        // Deallocate the entire block starting from `block_ptr`.
        dealloc(block_ptr, layout);
    }
}
