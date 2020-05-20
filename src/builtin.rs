use std::sync::Once;
use std::ffi::CString;
use std::os::raw::c_char;

#[cfg(feature = "async")]
use std::cell::RefCell;

pub static PANIC_HOOK: Once = Once::new();

#[cfg(feature = "async")]
pub static mut RUNTIME: RefCell<Option<tokio::runtime::Runtime>> = RefCell::new(None);

#[repr(C)]
pub struct FinchString {
  pub ptr: *mut c_char,
  pub len: usize,
}

impl FinchString {
  pub fn new(string: String) -> Self {
    // TODO: Support non-c-compatible strings
    Self {
      len: string.len(),
      ptr: CString::new(string).expect("Failed to create CString").into_raw(),
    }
  }
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