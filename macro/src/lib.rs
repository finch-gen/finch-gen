#![cfg_attr(nightly, feature(proc_macro_diagnostic))]

use std::sync::Once;
use std::sync::Mutex;
use std::iter::FromIterator;
use std::collections::HashSet;
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
  static ref CLASS_ERROR: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
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
          CLASS_ERROR.lock().unwrap().insert(name.to_string());

          return Diagnostic::spanned(data.span(), DiagnosticLevel::Error, "finch-gen[E0001] struct not public but exported with #[finch_bindgen]")
            .note("go to https://finch-gen.github.io/docs/errors/E0001 for more information")
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
        return Diagnostic::spanned(ty.span(), DiagnosticLevel::Error, &format!("finch-gen[E0005] invalid type found: expected path, got '{}'", quote!(#ty)))
          .note("go to https://finch-gen.github.io/docs/errors/E0005 for more information")
          .emit(TokenStream::new());
      }

      if CLASS_ERROR.lock().unwrap().contains(&name.to_string()) {
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
                    let pat = &arg.pat;
                    input_names.push(arg.ty.convert_arg(quote!(#pat)));
                  },
                  _ => {},
                }
              }

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
                  return Diagnostic::spanned(asyncness.span, DiagnosticLevel::Error, "finch-gen[E0002] found async function but the 'async' feature is not enabled")
                    .note("go to https://finch-gen.github.io/docs/errors/E0002 for more information")
                    .help("enable the 'async' feature for finch-gen in your Cargo.toml")
                    .emit(TokenStream::new());
                }
              } else {
                fn_body
              };

              let ret_expr;
              let body;
              if let syn::ReturnType::Type(_, ty) = &method.sig.output {
                let ret_type = ty.to_c_type();
                ret_expr = quote!(-> #ret_type);
                body = ty.convert_ret(fn_body);
              } else {
                ret_expr = proc_macro2::TokenStream::new();
                body = fn_body;
              }

              let inputs: syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma> = syn::punctuated::Punctuated::from_iter(
                inputs.into_iter().map(|x| {
                  match &x {
                    syn::FnArg::Receiver(_) => x,
                    syn::FnArg::Typed(y) => {
                      let mut z = y.clone();
                      z.ty = Box::new(z.ty.to_c_type());
                      syn::FnArg::Typed(z)
                    }
                  }
                })
              );

              let doc_comments = method.attrs.iter().filter(doc_filter);
              let panic_hook = inject_panic_hook();

              functions.push(quote!(
                #(#doc_comments)
                *

                #extra_comments
                #[no_mangle]
                pub unsafe extern fn #int_method_name(#inputs) #ret_expr {
                  #panic_hook

                  #body
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
      return Diagnostic::spanned(input.span(), DiagnosticLevel::Error, &format!("finch-gen[E0003] unexpected type for #[finch_bindgen], expected struct or impl, got '{}'", item))
        .note("go to https://finch-gen.github.io/docs/errors/E0003 for more information")
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
        ::std::process::abort();
      }));
    });
  };)
}

trait ToCType {
  fn to_c_type(&self) -> syn::Type;
  fn convert_arg(&self, body: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
  fn convert_ret(&self, body: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
}

impl ToCType for syn::Type {
  fn to_c_type(&self) -> syn::Type {
    match self.clone() {
      syn::Type::Path(path) => {
        let ident = path.path.segments.first().unwrap().ident.clone();
        let ty_name = ident.to_string();
  
        match ty_name.as_str() {
          "bool" | "char" | "u8" | "u16" | "u32" | "u64" | "usize"|
          "i8" | "i16" | "i32" | "i64" | "isize" | "f32" | "f64" |
          "c_void" | "c_char" | "c_schar" | "c_uchar" | "c_float" |
          "c_double" | "c_short" | "c_int" | "c_long" | "c_longlong" |
          "c_ushort" | "c_uint" | "c_ulong" | "c_ulonglong" |
          "uint8_t" | "uint16_t" | "uint32_t" | "uint64_t" | "uintptr_t" |
          "size_t" |" int8_t" | "int16_t" | "int32_t" | "int64_t" |
          "intptr_t" | "ssize_t" | "ptrdiff_t" => parse_quote!(#self),
  
          "Self" => parse_quote!(*mut #self),
  
          "String" => parse_quote!(::finch_gen::builtin::FinchString),
  
          "Option" => {
            if let syn::PathArguments::AngleBracketed(generics) = &path.path.segments.first().unwrap().arguments {
              if let syn::GenericArgument::Type(ty) = generics.args.first().unwrap() {
                let inner_type = ty.to_c_type();
                parse_quote!(::finch_gen::builtin::FinchOption<#inner_type>)
              } else {
                parse_quote!(())
              }
            } else {
              parse_quote!(())
            }
          },
  
          "Result" => {
            if let syn::PathArguments::AngleBracketed(generics) = &path.path.segments.first().unwrap().arguments {
              if let syn::GenericArgument::Type(ty) = generics.args.first().unwrap() {
                let inner_type = ty.to_c_type();
                parse_quote!(::finch_gen::builtin::FinchResult<#inner_type>)
              } else {
                parse_quote!(())
              }
            } else {
              parse_quote!(())
            }
          },
  
          _ => parse_quote!(()),
        }
      },
  
      _ => parse_quote!(()),
    }
  }

  fn convert_arg(&self, body: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    match self.clone() {
      syn::Type::Path(path) => {
        let ident = path.path.segments.first().unwrap().ident.clone();
        let ty_name = ident.to_string();
  
        match ty_name.as_str() {
          "bool" | "char" | "u8" | "u16" | "u32" | "u64" | "usize"|
          "i8" | "i16" | "i32" | "i64" | "isize" | "f32" | "f64" |
          "c_void" | "c_char" | "c_schar" | "c_uchar" | "c_float" |
          "c_double" | "c_short" | "c_int" | "c_long" | "c_longlong" |
          "c_ushort" | "c_uint" | "c_ulong" | "c_ulonglong" |
          "uint8_t" | "uint16_t" | "uint32_t" | "uint64_t" | "uintptr_t" |
          "size_t" |" int8_t" | "int16_t" | "int32_t" | "int64_t" |
          "intptr_t" | "ssize_t" | "ptrdiff_t" => body,
  
          "Self" => quote!(Box::into_raw(Box::new(#body))),
  
          "String" => quote!(*Box::from_raw(::std::mem::ManuallyDrop::new(#body).string)),

          "Option" => {
            if let syn::PathArguments::AngleBracketed(generics) = &path.path.segments.first().unwrap().arguments {
              if let syn::GenericArgument::Type(ty) = generics.args.first().unwrap() {
                let inner_body = ty.convert_arg(quote!(x));
                quote!({
                  if let ::finch_gen::builtin::FinchOption::Some(x) = #body {
                    Some(#inner_body)
                  } else {
                    None
                  }
                })
              } else {
                proc_macro2::TokenStream::from(
                  Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0006] expected type for generics`"))
                    .note("go to https://finch-gen.github.io/docs/errors/E0006 for more information")
                    .emit(TokenStream::new()),
                )
              }
            } else {
              proc_macro2::TokenStream::from(
                Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0007] expected generics for Option"))
                  .note("go to https://finch-gen.github.io/docs/errors/E0007 for more information")
                  .emit(TokenStream::new()),
              )
            }
          },
  
          _ => {
            proc_macro2::TokenStream::from(
              Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0004] unsupported type '{}'", quote!(#self)))
                .note("go to https://finch-gen.github.io/docs/errors/E0004 for more information")
                .emit(TokenStream::new()),
            )
          }
        }
      },
  
      _ => {
          proc_macro2::TokenStream::from(
            Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0004] unsupported type '{}'", quote!(#self)))
              .note("go to https://finch-gen.github.io/docs/errors/E0004 for more information")
              .emit(TokenStream::new()),
          )
      },
    }
  }

  fn convert_ret(&self, body: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    match self.clone() {
      syn::Type::Path(path) => {
        let ident = path.path.segments.first().unwrap().ident.clone();
        let ty_name = ident.to_string();
  
        match ty_name.as_str() {
          "bool" | "char" | "u8" | "u16" | "u32" | "u64" | "usize"|
          "i8" | "i16" | "i32" | "i64" | "isize" | "f32" | "f64" |
          "c_void" | "c_char" | "c_schar" | "c_uchar" | "c_float" |
          "c_double" | "c_short" | "c_int" | "c_long" | "c_longlong" |
          "c_ushort" | "c_uint" | "c_ulong" | "c_ulonglong" |
          "uint8_t" | "uint16_t" | "uint32_t" | "uint64_t" | "uintptr_t" |
          "size_t" |" int8_t" | "int16_t" | "int32_t" | "int64_t" |
          "intptr_t" | "ssize_t" | "ptrdiff_t" => body,
  
          "Self" => quote!(Box::into_raw(Box::new(#body))),
  
          "String" => quote!(::finch_gen::builtin::FinchString::from(#body)),
  
          "Option" => {
            if let syn::PathArguments::AngleBracketed(generics) = &path.path.segments.first().unwrap().arguments {
              if let syn::GenericArgument::Type(ty) = generics.args.first().unwrap() {
                let inner_body = ty.convert_ret(quote!(x));
                quote!({
                  if let Some(x) = #body {
                    ::finch_gen::builtin::FinchOption::Some(#inner_body)
                  } else {
                    ::finch_gen::builtin::FinchOption::None
                  }
                })
              } else {
                proc_macro2::TokenStream::from(
                  Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0006] expected type for generics`"))
                    .note("go to https://finch-gen.github.io/docs/errors/E0006 for more information")
                    .emit(TokenStream::new()),
                )
              }
            } else {
              proc_macro2::TokenStream::from(
                Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0007] expected generics for Option"))
                  .note("go to https://finch-gen.github.io/docs/errors/E0007 for more information")
                  .emit(TokenStream::new()),
              )
            }
          },
  
          "Result" => {
            if let syn::PathArguments::AngleBracketed(generics) = &path.path.segments.first().unwrap().arguments {
              if let syn::GenericArgument::Type(ty) = generics.args.first().unwrap() {
                let inner_body = ty.convert_ret(quote!(x));
                quote!({
                  let r = #body;
                  match r {
                    Ok(x) => ::finch_gen::builtin::FinchResult::Ok(#inner_body),
                    Err(x) => ::finch_gen::builtin::FinchResult::Err(::finch_gen::builtin::FinchString::from(format!("{}", x))),
                  }
                })
              } else {
                proc_macro2::TokenStream::from(
                  Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0006] expected type for generics`"))
                    .note("go to https://finch-gen.github.io/docs/errors/E0006 for more information")
                    .emit(TokenStream::new()),
                )
              }
            } else {
              proc_macro2::TokenStream::from(
                Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0007] expected generics for Result"))
                  .note("go to https://finch-gen.github.io/docs/errors/E0007 for more information")
                  .emit(TokenStream::new()),
              )
            }
          },
  
          _ => {
            proc_macro2::TokenStream::from(
              Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0004] unsupported type '{}'", quote!(#self)))
                .note("go to https://finch-gen.github.io/docs/errors/E0004 for more information")
                .emit(TokenStream::new()),
            )
          }
        }
      },
  
      _ => {
          proc_macro2::TokenStream::from(
            Diagnostic::spanned(self.span(), DiagnosticLevel::Error, &format!("finch-gen[E0004] unsupported type '{}'", quote!(#self)))
              .note("go to https://finch-gen.github.io/docs/errors/E0004 for more information")
              .emit(TokenStream::new()),
          )
      },
    }
  }
}
