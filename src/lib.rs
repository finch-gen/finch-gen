use std::os::raw::c_char;

pub use finch_macro::*;

#[repr(C)]
pub struct FinchString {
  ptr: *mut c_char,
  len: usize,
}

#[repr(C)]
pub enum FinchOption<T> {
  Some(T),
  None,
}

#[repr(C)]
pub enum FinchResult<T, E> {
  Ok(T),
  Err(E),
}
