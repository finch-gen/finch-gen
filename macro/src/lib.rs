#![cfg_attr(nightly, feature(proc_macro_diagnostic))]

use std::sync::Once;
use std::iter::FromIterator;
use std::sync::Mutex;
use syn::spanned::Spanned;
use proc_macro::TokenStream;
use lazy_static::lazy_static;
use syn::{parse_macro_input, parse_quote};
use quote::{quote, format_ident};

mod builtin;
mod diagnostic;
use diagnostic::{Diagnostic, DiagnosticLevel};

static INJECT: Once = Once::new();

lazy_static! {
  // static ref CLASS_ERROR: HashMap<String, bool> = {
  //   HashMap::new()
  // };
  static ref CLASS_ERROR: Mutex<Vec<String>> = Mutex::new(vec![]);
}

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
          CLASS_ERROR.lock().unwrap().push(name.to_string());

          return Diagnostic::spanned(data.span(), DiagnosticLevel::Error, "finch-gen[E001] struct not public but exported with #[finch_bindgen]")
            .note("go to https://finch-gen.github.io/docs/errors/E001 for more information")
            .span_help(data.struct_token.span, "add 'pub' here")
            .emit(item);
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
        let ty = input.self_ty;
        return Diagnostic::spanned(ty.span(), DiagnosticLevel::Error, &format!("finch-gen[E005] invalid type found: expected path, got '{}'", quote!(#ty)))
          .note("go to https://finch-gen.github.io/docs/errors/E005 for more information")
          .emit(TokenStream::new());
      }

      if CLASS_ERROR.lock().unwrap().iter().find(|x| x == &&name.to_string()).is_some() {
        return item;
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

              let fn_body = if let Some(asyncness) = method.sig.asyncness {
                if cfg!(feature = "async") {
                  quote!({
                    let mut rt = finch_gen::builtin::RUNTIME.get_mut();
                    if rt.is_none() {
                      *rt = ::std::option::Option::Some(::tokio::runtime::Runtime::new().expect("failed to create tokio runtime"));
                    }
  
                    if let ::std::option::Option::Some(rt) = rt {
                      rt.block_on(async {
                        #fn_body.await
                      })
                    } else {
                      panic!("failed to get tokio runtime")
                    }
                  })
                } else {
                  return Diagnostic::spanned(asyncness.span, DiagnosticLevel::Error, "finch-gen[E002] found async function but the 'async' feature is not enabled")
                    .note("go to https://finch-gen.github.io/docs/errors/E002 for more information")
                    .help("enable the 'async' feature for finch-gen in your Cargo.toml")
                    .emit(TokenStream::new());
                }
              } else {
                fn_body
              };

              let (ret_expr, fn_body) = convert_return_type(&ret_type, fn_body);
              let inputs: syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma> = syn::punctuated::Punctuated::from_iter(inputs.into_iter());

              let doc_comments = method.attrs.iter().filter(doc_filter);
              let panic_hook = inject_panic_hook();

              functions.push(quote!(
                #(#doc_comments)
                *

                #extra_comments
                #[no_mangle]
                pub unsafe extern fn #int_method_name(#inputs) #ret_expr {
                  #panic_hook

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
      return Diagnostic::spanned(input.span(), DiagnosticLevel::Error, &format!("finch-gen[E003] unexpected type for #[finch_bindgen], expected struct or impl, got '{}'", item))
        .note("go to https://finch-gen.github.io/docs/errors/E003 for more information")
        .emit(item);
    }
  }
}

fn inject_boilerplate() -> proc_macro2::TokenStream {
  let mut out = proc_macro2::TokenStream::new();
  INJECT.call_once(|| {
    out = builtin::make_builtin(crate_name());
  });

  out
}

fn inject_panic_hook() -> proc_macro2::TokenStream {
  quote!({
    ::finch_gen::builtin::PANIC_HOOK.call_once(|| {
      ::std::panic::set_hook(Box::new(|x| {
        ::std::eprintln!("thread '<{}>' {}", ::std::thread::current().name().unwrap_or("unnamed"), x);
        ::std::process::exit(1);
      }));
    });
  };)
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
              quote!(-> ::finch_gen::builtin::FinchString),
              quote!(::finch_gen::builtin::FinchString::new(#body))
            ),

            _ => {
              return (
                proc_macro2::TokenStream::new(),
                proc_macro2::TokenStream::from(
                  Diagnostic::spanned(ty.span(), DiagnosticLevel::Error, &format!("finch-gen[E004] unsupported type '{}'", quote!(#ty)))
                    .note("go to https://finch-gen.github.io/docs/errors/E004 for more information")
                    .emit(TokenStream::new()),
                  ),
              );
            }
          }
        },

        _ => {
          return (
            proc_macro2::TokenStream::new(),
            proc_macro2::TokenStream::from(
              Diagnostic::spanned(ty.span(), DiagnosticLevel::Error, &format!("finch-gen[E004] unsupported type '{}'", quote!(#ty)))
                .note("go to https://finch-gen.github.io/docs/errors/E004 for more information")
                .emit(TokenStream::new()),
              ),
          );
        }
      }
    }
  }
}
