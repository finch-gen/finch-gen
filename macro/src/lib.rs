
use std::sync::Once;
use std::iter::FromIterator;
use syn::spanned::Spanned;
use proc_macro::TokenStream;
use syn::{parse_macro_input, parse_quote};
use quote::{quote, quote_spanned, format_ident};

static INJECT: Once = Once::new();

fn doc_filter<'r>(x: &'r &syn::Attribute) -> bool {
  x.path.segments.first().unwrap().ident.to_string() == "doc"
}

fn crate_name() -> String {
  std::env::var("CARGO_PKG_NAME").unwrap().replace("-", "_")
}

#[proc_macro_attribute]
pub fn finch_bindgen(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let cloned = item.clone();
  let input = parse_macro_input!(cloned as syn::Item);

  match input {
    syn::Item::Struct(data) => {
      let name = &data.ident;

      match data.vis {
        syn::Visibility::Public(_) => {}
        _ => {
          return TokenStream::from(quote_spanned! {
            data.span() =>
            compile_error!("struct not public but exported with #[finch_bindgen]");
          });
        }
      }

      let mut functions = Vec::new();

      match &data.fields {
        syn::Fields::Named(fields) => {
          for field in &fields.named {
            if let syn::Visibility::Public(_) = field.vis {
              let readable = true;
              let writeable = true;
              // for attr in &field.attrs {
              //   match attr.path.segments.first().unwrap().ident.to_string().as_str() {
              //     "finch_private" => {readable = false; writeable = false},
              //     "finch_readonly" => writeable = false,
              //     "finch_writeonly" => readable = false,
              //     _ => {},
              //   }
              // }

              let field_name = field.clone().ident.unwrap();
              let field_type = &field.ty;
              let doc_comments = field.attrs.iter().filter(doc_filter);

              if readable {
                let getter_name = format_ident!("___finch_bindgen___{}___class___{}___getter___{}", crate_name(), name, field_name);
                let doc_comments_getter = doc_comments.clone();
                functions.push(quote!(
                  #(#doc_comments_getter)
                  *
                  #[no_mangle]
                  pub unsafe extern fn #getter_name(&self) -> #field_type {
                    self.#field_name
                  }
                ));
              }

              if writeable {
                let setter_name = format_ident!("___finch_bindgen___{}___class___{}___setter___{}", crate_name(), name, field_name);
                functions.push(quote!(
                  #(#doc_comments)
                  *
                  #[no_mangle]
                  pub unsafe extern fn #setter_name(&mut self, value: #field_type) {
                    self.#field_name = value
                  }
                ));
              }
            }
          }
        }

        _ => {}
      }

      let doc_comments = data.attrs.iter().filter(doc_filter);

      let drop_name = format_ident!("___finch_bindgen___{}___class___{}___drop", crate_name(), name);
      let new_name = format_ident!("___finch_bindgen___{}___class___{}___type", crate_name(), name);

      let item = proc_macro2::TokenStream::from(item);

      let boilerplate = inject_boilerplate();
      let class_impl = quote!(
        #item

        #(#doc_comments)
        *
        #[allow(non_camel_case_types)]
        type #new_name = #name;

        #[allow(non_snake_case)]
        impl #new_name {
          #[no_mangle]
          pub unsafe extern fn #drop_name(ptr: *mut Self) {
            drop(Box::from_raw(ptr))
          }

          #(#functions)*
        }

        #boilerplate
      );

      TokenStream::from(class_impl)
    }

    syn::Item::Impl(input) => {
      let name;
      if let syn::Type::Path(path) = *input.self_ty.clone() {
        name = path.path.segments.first().unwrap().ident.clone();
      } else {
        return TokenStream::from(quote_spanned! {
          input.span() =>
          compile_error!("invalid type found, expected path");
        });
      }

      let mut functions = Vec::new();

      for item in &input.items {
        match item {
          syn::ImplItem::Method(method) => {
            if let syn::Visibility::Public(_) = method.vis {
              let method_name = &method.sig.ident;
              let mut inputs = Vec::from_iter(method.sig.inputs.clone());
              let mut input_names = Vec::new();

              for input in &method.sig.inputs {
                match input {
                  syn::FnArg::Typed(arg) => {
                    input_names.push(arg.pat.clone());
                  },
                  _ => {},
                }
              }

              let ret_type = &method.sig.output;
              let int_method_name;
              let fn_body;
              let mut extra_comments = quote!();

              if method.sig.inputs.len() > 0 {
                match method.sig.inputs.first().unwrap() {
                  syn::FnArg::Receiver(receiver) => {
                    if receiver.reference.is_some() {
                      int_method_name = format_ident!("___finch_bindgen___{}___class___{}___method___{}", crate_name(), name, method_name);
                      fn_body = quote!(self.#method_name(#(#input_names),*));
                    } else {
                      int_method_name = format_ident!("___finch_bindgen___{}___class___{}___method_consume___{}", crate_name(), name, method_name);
                      inputs.remove(0);
                      inputs.insert(0, parse_quote!(ptr: *mut Self));
                      fn_body = quote!(Box::from_raw(ptr).#method_name(#(#input_names),*));
                      extra_comments = quote!(
                        /// This method consumes the internal pointer.
                        /// You cannot call any methods, or get/set any values
                        /// after calling this method.
                      );
                    }
                  },

                  syn::FnArg::Typed(_) => {
                    int_method_name = format_ident!("___finch_bindgen___{}___class___{}___static___{}", crate_name(), name, method_name);
                    fn_body = quote!(Self::#method_name(#(#input_names),*));
                  }
                }
              } else {
                int_method_name = format_ident!("___finch_bindgen___{}___class___{}___static___{}", crate_name(), name, method_name);
                fn_body = quote!(Self::#method_name(#(#input_names),*));
              }
              

              let (ret_expr, fn_body) = convert_return_type(&ret_type, fn_body);
              let inputs: syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma> = syn::punctuated::Punctuated::from_iter(inputs.into_iter());

              let doc_comments = method.attrs.iter().filter(doc_filter);

              functions.push(quote!(
                #(#doc_comments)
                *

                #extra_comments
                #[no_mangle]
                pub unsafe extern fn #int_method_name(#inputs) #ret_expr {
                  #fn_body
                }
              ));
            }
          },

          _ => {},
        }
      }

      let new_name = format_ident!("___finch_bindgen___{}___class___{}___type", crate_name(), name);

      let item = proc_macro2::TokenStream::from(item);

      let boilerplate = inject_boilerplate();
      let class_impl = quote!(
        #item

        #[allow(non_snake_case)]
        impl #new_name {
          #(#functions)*
        }

        #boilerplate
      );

      proc_macro::TokenStream::from(class_impl)
    }

    _ => {
      let error = format!("unexpected type for #[finch_bindgen], expected struct or impl, got \"{}\"", item);
      return TokenStream::from(quote_spanned! {
        input.span() =>
        compile_error!(#error);
      });
    }
  }
}

fn inject_boilerplate() -> proc_macro2::TokenStream {
  let mut out = proc_macro2::TokenStream::new();
  INJECT.call_once(|| {
    let init_fn_name = format_ident!("___finch_bindgen___{}___initialize", crate_name());
    let fn_name = format_ident!("___finch_bindgen___{}___boilerplate___free___cstring", crate_name());

    out = quote!(
      #[no_mangle]
      pub unsafe extern fn #init_fn_name() {
        std::panic::set_hook(Box::new(|x| {
          println!("thread '<{}>' {}", ::std::thread::current().name().unwrap_or("unnamed"), x);
          std::process::exit(1);
        }));
      }

      #[no_mangle]
      pub unsafe extern fn #fn_name(string: *mut ::std::os::raw::c_char) {
        drop(::std::ffi::CString::from_raw(string));
      }
    );
  });

  out
}

fn convert_return_type(ret_type: &syn::ReturnType, body: proc_macro2::TokenStream) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
  match ret_type.clone() {
    syn::ReturnType::Default => (proc_macro2::TokenStream::new(), body),
    syn::ReturnType::Type(_, ty) => {
      match *ty.clone() {
        syn::Type::Path(path) => {
          let ident = path.path.segments.first().unwrap().ident.clone();
          let ty_name = ident.to_string();

          match ty_name.as_str() {
            "bool" => (quote!(#ret_type), body),
            "char" => (quote!(#ret_type), body),
            "u8" => (quote!(#ret_type), body),
            "u16" => (quote!(#ret_type), body),
            "u32" => (quote!(#ret_type), body),
            "u64" => (quote!(#ret_type), body),
            "usize" => (quote!(#ret_type), body),
            "i8" => (quote!(#ret_type), body),
            "i16" => (quote!(#ret_type), body),
            "i32" => (quote!(#ret_type), body),
            "i64" => (quote!(#ret_type), body),
            "isize" => (quote!(#ret_type), body),
            "f32" => (quote!(#ret_type), body),
            "f64" => (quote!(#ret_type), body),

            "c_void" => (quote!(#ret_type), body),
            "c_char" => (quote!(#ret_type), body),
            "c_schar" => (quote!(#ret_type), body),
            "c_uchar" => (quote!(#ret_type), body),
            "c_float" => (quote!(#ret_type), body),
            "c_double" => (quote!(#ret_type), body),
            "c_short" => (quote!(#ret_type), body),
            "c_int" => (quote!(#ret_type), body),
            "c_long" => (quote!(#ret_type), body),
            "c_longlong" => (quote!(#ret_type), body),
            "c_ushort" => (quote!(#ret_type), body),
            "c_uint" => (quote!(#ret_type), body),
            "c_ulong" => (quote!(#ret_type), body),
            "c_ulonglong" => (quote!(#ret_type), body),

            "uint8_t" => (quote!(#ret_type), body),
            "uint16_t" => (quote!(#ret_type), body),
            "uint32_t" => (quote!(#ret_type), body),
            "uint64_t" => (quote!(#ret_type), body),
            "uintptr_t" => (quote!(#ret_type), body),
            "size_t" => (quote!(#ret_type), body),
            "int8_t" => (quote!(#ret_type), body),
            "int16_t" => (quote!(#ret_type), body),
            "int32_t" => (quote!(#ret_type), body),
            "int64_t" => (quote!(#ret_type), body),
            "intptr_t" => (quote!(#ret_type), body),
            "ssize_t" => (quote!(#ret_type), body),
            "ptrdiff_t" => (quote!(#ret_type), body),

            "Self" => (
              quote!(-> *mut #ident),
              quote!(Box::into_raw(Box::new(#body)))
            ),

            "String" => (
              quote!(-> *mut std::os::raw::c_char),
              quote!(std::ffi::CString::new(#body).expect("Failed to create CString").into_raw())
            ),

            _ => {
              let error = format!("finch-gen does not support the type \"{}\"", quote!(#ty));
              return (proc_macro2::TokenStream::from(quote_spanned! {
                path.span() =>
                compile_error!(#error);
              }), proc_macro2::TokenStream::new());
            }
          }
        },

        _ => {
          let error = format!("finch-gen does not support the type \"{}\"", quote!(#ty));
          return (proc_macro2::TokenStream::new(), proc_macro2::TokenStream::from(quote_spanned! {
            ty.span() =>
            compile_error!(#error);
          }));
        }
      }
    }
  }
}
