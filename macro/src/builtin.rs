use quote::{quote, format_ident};

pub fn make_builtin(crate_name: String) -> proc_macro2::TokenStream {
  let string_drop_fn_name = format_ident!("___finch_bindgen___{}___builtin___FinchString___drop", crate_name);

  quote!(
    #[no_mangle]
    pub unsafe extern fn #string_drop_fn_name(string: ::finch_gen::builtin::FinchString) {
      drop(::std::ffi::CString::from_raw(string.ptr));
    }
  )
}