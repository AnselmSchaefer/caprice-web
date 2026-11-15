//! What the generated bindings (`bindings.rs`, made with `--runtime-path crate::rt`) need of a
//! runtime: wit-bindgen's own, cut down to this one world, so a theme's build needs no crate but
//! this one. `cabi_realloc` itself is written into the theme by `theme!`, where the linker exports
//! it as it is, with no static library to bring it in (as wit-bindgen's runtime needs).

use std::alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc as grow};
use std::ptr::NonNull;

/// The canonical ABI's allocator, as wit-bindgen has it: Caprice asks for room for the input and
/// the events through it.
///
/// # Safety
/// As `std::alloc`'s: `old` was given by this for `old_len` bytes aligned to `align`.
pub unsafe fn realloc(old: *mut u8, old_len: usize, align: usize, new_len: usize) -> *mut u8 {
    let ptr = unsafe {
        if old_len == 0 {
            if new_len == 0 {
                return align as *mut u8;
            }
            alloc(Layout::from_size_align_unchecked(new_len, align))
        } else {
            grow(old, Layout::from_size_align_unchecked(old_len, align), new_len)
        }
    };
    if ptr.is_null() {
        handle_alloc_error(unsafe { Layout::from_size_align_unchecked(new_len, align) });
    }
    ptr
}

/// Statics' constructors, run once before the first call in, as wit-bindgen does.
#[cfg(target_arch = "wasm32")]
pub fn run_ctors_once() {
    static mut RUN: bool = false;
    unsafe {
        if !RUN {
            unsafe extern "C" {
                fn __wasm_call_ctors();
            }
            __wasm_call_ctors();
            RUN = true;
        }
    }
}

/// Nothing to bring in: `theme!` writes `cabi_realloc` into the theme itself.
pub fn maybe_link_cabi_realloc() {}

/// Memory the bindings lower an import's arguments into, freed when dropped (wit-bindgen's).
pub struct Cleanup {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl Cleanup {
    pub fn new(layout: Layout) -> (*mut u8, Option<Cleanup>) {
        if layout.size() == 0 {
            return (std::ptr::null_mut(), None);
        }
        let ptr = unsafe { alloc(layout) };
        let Some(ptr) = NonNull::new(ptr) else { handle_alloc_error(layout) };
        (ptr.as_ptr(), Some(Cleanup { ptr, layout }))
    }

    pub fn forget(self) {
        std::mem::forget(self);
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        unsafe { dealloc(self.ptr.as_ptr(), self.layout) }
    }
}
