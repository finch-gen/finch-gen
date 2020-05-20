use proc_macro::TokenStream;

#[rustversion::not(nightly)]
use quote::{quote, quote_spanned};

#[derive(Eq, PartialEq)]
pub enum DiagnosticLevel {
  Error,
}

#[cfg(nightly)]
impl Into<proc_macro::Level> for DiagnosticLevel {
  fn into(self) -> proc_macro::Level {
    match self {
      DiagnosticLevel::Error => proc_macro::Level::Error,
    }
  }
}

#[cfg(nightly)]
pub struct Diagnostic {
  diag: proc_macro::Diagnostic,
}

#[cfg(nightly)]
impl Diagnostic {
  pub fn spanned(span: proc_macro2::Span, level: DiagnosticLevel, message: &str) -> Self {
    Self {
      diag: proc_macro::Diagnostic::spanned(vec![span.unwrap()], level.into(), message),
    }
  }

  pub fn span_help<T: Into<String>>(mut self, span: proc_macro2::Span, message: T) -> Self {
    self.diag = self.diag.span_help(vec![span.unwrap()], message);
    self
  }

  pub fn help<T: Into<String>>(mut self, message: T) -> Self {
    self.diag = self.diag.help(message);
    self
  }

  pub fn note<T: Into<String>>(mut self, message: T) -> Self {
    self.diag = self.diag.note(message);
    self
  }

  pub fn emit(self, tokens: TokenStream) -> TokenStream {
    self.diag.emit();
    tokens
  }
}

#[cfg(not(nightly))]
pub struct Diagnostic {
  tokens: proc_macro2::TokenStream,
}

#[cfg(not(nightly))]
#[allow(unused_mut)]
impl Diagnostic {
  pub fn spanned(span: proc_macro2::Span, level: DiagnosticLevel, message: &str) -> Self {
    if level == DiagnosticLevel::Error {
      Self {
        tokens: quote_spanned! { span => compile_error!(#message); },
      }
    } else {
      Self {
        tokens: proc_macro2::TokenStream::new(),
      }
    }
  }

  pub fn span_help<T: Into<String>>(mut self, _span: proc_macro2::Span, _message: T) -> Self {
    self
  }

  pub fn help<T: Into<String>>(mut self, _message: T) -> Self {
    self
  }

  pub fn note<T: Into<String>>(mut self, _message: T) -> Self {
    self
  }

  pub fn emit(self, item: TokenStream) -> TokenStream {
    let item = proc_macro2::TokenStream::from(item);
    let tokens = self.tokens;
    TokenStream::from(quote!(
      #item
      #tokens
    ))
  }
}