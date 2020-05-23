use quote::{quote, format_ident};

pub fn make_builtin(crate_name: String) -> proc_macro2::TokenStream {
  let string_new_fn_name = format_ident!("___finch_bindgen___{}___builtin___FinchString___new", crate_name);
  let string_drop_fn_name = format_ident!("___finch_bindgen___{}___builtin___FinchString___drop", crate_name);
  let cstring_drop_fn_name = format_ident!("___finch_bindgen___{}___builtin___FinchCString___drop", crate_name);

  quote!(
    #[no_mangle]
    pub unsafe extern fn #string_new_fn_name(data: *const u8, len: usize) -> ::finch_gen::builtin::FinchString {
      ::finch_gen::builtin::FinchString::new(data, len)
    }
    
    #[no_mangle]
    pub unsafe extern fn #string_drop_fn_name(value: ::finch_gen::builtin::FinchString) {
      drop(value);
    }

    #[no_mangle]
    pub unsafe extern fn #cstring_drop_fn_name(value: ::finch_gen::builtin::FinchCString) {
      drop(value);
    }
  )
}