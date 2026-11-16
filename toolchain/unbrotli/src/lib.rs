//! A brotli stream decoded a megabyte at a time: the page puts the compressed bytes in (`input`),
//! then takes out what `read` writes into the one output buffer, until it returns 0. So the whole
//! file is held only once, in the page's own buffer, never also here.

use std::io::Read;

static mut INPUT: Vec<u8> = Vec::new();
static mut OUTPUT: Vec<u8> = Vec::new();
static mut DECODER: Option<brotli_decompressor::Decompressor<&'static [u8]>> = None;

/// Room for `len` compressed bytes, and for a chunk of `chunk` decoded ones; the input's address.
#[unsafe(no_mangle)]
pub extern "C" fn input(len: usize, chunk: usize) -> *mut u8 {
    unsafe {
        let input = &mut *std::ptr::addr_of_mut!(INPUT);
        *input = vec![0; len];
        *std::ptr::addr_of_mut!(OUTPUT) = vec![0; chunk];
        *std::ptr::addr_of_mut!(DECODER) = None;
        input.as_mut_ptr()
    }
}

/// The output buffer's address.
#[unsafe(no_mangle)]
pub extern "C" fn output() -> *mut u8 {
    unsafe { (*std::ptr::addr_of_mut!(OUTPUT)).as_mut_ptr() }
}

/// Decodes the next chunk into the output buffer: how many bytes, 0 at the end, -1 on an error.
#[unsafe(no_mangle)]
pub extern "C" fn read() -> isize {
    unsafe {
        let decoder = &mut *std::ptr::addr_of_mut!(DECODER);
        if decoder.is_none() {
            let input: &'static [u8] = &*std::ptr::addr_of!(INPUT);
            *decoder = Some(brotli_decompressor::Decompressor::new(input, 1 << 16));
        }
        let out = &mut *std::ptr::addr_of_mut!(OUTPUT);
        let d = decoder.as_mut().unwrap();
        let mut n = 0;
        while n < out.len() {
            match d.read(&mut out[n..]) {
                Ok(0) => break,
                Ok(k) => n += k,
                Err(_) => return -1,
            }
        }
        n as isize
    }
}
