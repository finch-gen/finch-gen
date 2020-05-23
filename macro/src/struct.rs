use rand::prelude::*;
use proc_macro2::{TokenStream};
use serde::{Serialize, Deserialize};
use quote::{quote, format_ident};
use syn::parse_quote;

fn doc_filter<'r>(x: &'r &syn::Attribute) -> bool {
  x.path.segments.last().unwrap().ident.to_string() == "doc"
}

fn doc_to_string(x: &syn::Attribute) -> String {
  let mut string = x.tokens.to_string();
  string.truncate(string.len() - 1);
  string = string.replacen("=", "", 1);
  string = string.replacen('"', "", 1);
  string.trim().to_string()
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Item {
  Struct(Struct),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PrimitiveType {
  Int32,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Type {
  Primitive(PrimitiveType)
}

impl Type {
  fn new(ty: syn::Type) -> Self {
    match ty {
      syn::Type::Path(path) => {
        Self::Primitive(PrimitiveType::Int32)
      },

      _ => todo!("Implement other types"),
    }
  }

  fn to_repr_c(&self) -> syn::Type {
    match self {
      Self::Primitive(PrimitiveType::Int32) => parse_quote!(i32),
    }
  }

  fn wrap_arg(&self, body: TokenStream) -> TokenStream {
    match self {
      Self::Primitive(_) => body,
    }
  }

  fn wrap_ret(&self, body: TokenStream) -> TokenStream {
    match self {
      Self::Primitive(_) => body,
    }
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Struct {
  pub name: String,
  pub drop_id: String,
  pub fields: StructFields,
  pub comments: Vec<String>,
}

impl Struct {
  pub(crate) fn new(strukt: syn::ItemStruct) -> (Self, TokenStream) {
    let ident = strukt.ident;

    let drop_id = format!("_{:x}", random::<u64>());
    let drop_ident = format_ident!("{}", drop_id);

    let (fields, tokens) = StructFields::new(strukt.fields);

    (
      Self {
        name: ident.to_string(),
        drop_id,
        fields,
        comments: strukt.attrs.iter().filter(doc_filter).map(doc_to_string).collect(),
      },
      quote!(
        impl #ident {
          #[no_mangle]
          pub unsafe extern fn #drop_ident(ptr: *mut Self) {
            ::std::mem::drop(::std::boxed::Box::from_raw(ptr))
          }

          #(#tokens)
          *
        }
      )
    )
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum StructFields {
  Named(Vec<NamedField>),
  Unnamed(Vec<UnnamedField>),
  Unit,
}

impl StructFields {
  fn new(fields: syn::Fields) -> (Self, Vec<TokenStream>) {
    match fields {
      syn::Fields::Named(fields) => {
        let mut fields_vec = vec![];
        let mut tokens = vec![];
        for field in fields.named {
          if let syn::Visibility::Public(_) = field.vis {
            let (f, token) = NamedField::new(field);
            fields_vec.push(f);
            tokens.push(token);
          }
        }
        (Self::Named(fields_vec), tokens)
      },

      syn::Fields::Unnamed(fields) => {
        println!("{}", fields.unnamed.len());

        let mut fields_vec = vec![];
        let mut tokens = vec![];
        for (idx, field) in fields.unnamed.into_iter().enumerate() {
          let (f, token) = UnnamedField::new(idx, field);
          fields_vec.push(f);
          tokens.push(token);
        }
        (Self::Unnamed(fields_vec), tokens)
      },

      syn::Fields::Unit => (Self::Unit, vec![]),
    }
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NamedField {
  pub name: String,
  pub ty: Type,
  pub getter_id: String,
  pub setter_id: String,
  pub comments: Vec<String>,
}

impl NamedField {
  fn new(field: syn::Field) -> (Self, TokenStream) {
    let ty = Type::new(field.ty);

    let field_name = field.ident.unwrap();
    let repr_type = ty.to_repr_c();

    let getter_body = ty.wrap_ret(quote!(self.#field_name));
    let setter_body = ty.wrap_arg(quote!(value));

    let getter_id = format!("_{:x}", random::<u64>());
    let setter_id = format!("_{:x}", random::<u64>());

    let getter_ident = format_ident!("{}", getter_id);
    let setter_ident = format_ident!("{}", setter_id);

    (
      Self {
        name: field_name.to_string(),
        ty,
        getter_id,
        setter_id,
        comments: field.attrs.iter().filter(doc_filter).map(doc_to_string).collect(),
      },
      quote!(
        #[no_mangle]
        pub unsafe extern fn #getter_ident(&self) -> #repr_type {
          #getter_body
        }
  
        #[no_mangle]
        pub unsafe extern fn #setter_ident(&mut self, value: #repr_type) {
          self.#field_name = #setter_body
        }
      )
    )
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UnnamedField {
  pub idx: usize,
  pub ty: Type,
  pub getter_id: String,
  pub setter_id: String,
  pub comments: Vec<String>,
}

impl UnnamedField {
  fn new(idx: usize, field: syn::Field) -> (Self, TokenStream) {
    let ty = Type::new(field.ty);

    let field_name = syn::Index::from(idx);
    let repr_type = ty.to_repr_c();

    let getter_body = ty.wrap_ret(quote!(self.#field_name));
    let setter_body = ty.wrap_arg(quote!(value));

    let getter_id = format!("_{:x}", random::<u64>());
    let setter_id = format!("_{:x}", random::<u64>());

    let getter_ident = format_ident!("{}", getter_id);
    let setter_ident = format_ident!("{}", setter_id);

    (
      Self {
        idx,
        ty,
        getter_id,
        setter_id,
        comments: field.attrs.iter().filter(doc_filter).map(doc_to_string).collect(),
      },
      quote!(
        #[no_mangle]
        pub unsafe extern fn #getter_ident(&self) -> #repr_type {
          #getter_body
        }
  
        #[no_mangle]
        pub unsafe extern fn #setter_ident(&mut self, value: #repr_type) {
          self.#field_name = #setter_body
        }
      )
    )
  }
}
