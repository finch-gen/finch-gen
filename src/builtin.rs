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
  pub ptr: *const c_char,
  pub len: usize,
  pub string: *mut String,
}

impl FinchString {
  pub unsafe fn new(data: *const u8, len: usize) -> Self {
    let ptr = ::std::alloc::alloc(::std::alloc::Layout::from_size_align(len, 1).expect("failed to create ::std::alloc::Layout"));
    ::std::ptr::copy_nonoverlapping(data, ptr, len);
    let string = String::from_raw_parts(ptr, len, len);
    Self::from(string)
  }
}

impl From<String> for FinchString {
  fn from(string: String) -> Self {
    let string = Box::new(string);
    Self {
      len: string.as_bytes().len(),
      ptr: string.as_ptr() as *const c_char,
      string: Box::into_raw(string),
    }
  }
}

impl Drop for FinchString {
  fn drop(&mut self) {
    drop(unsafe { Box::from_raw(self.string) });
  }
}

#[repr(C)]
pub struct FinchCString {
  pub ptr: *mut c_char,
  pub len: usize,
}

impl From<String> for FinchCString {
  fn from(string: String) -> Self {
    let string = CString::new(string).expect("Failed to create CString");
    Self {
      len: string.as_bytes().len(),
      ptr: string.into_raw(),
    }
  }
}

impl Drop for FinchCString {
  fn drop(&mut self) {
    drop(unsafe { CString::from_raw(self.ptr) });
  }
}

#[repr(C)]
pub enum FinchOption<T> {
  Some(T),
  None,
}

#[repr(C)]
pub enum FinchResult<T> {
  Ok(T),
  Err(FinchString),
}
