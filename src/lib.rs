use std::error::Error;

pub use finch_macro::*;

pub mod builtin;

pub(crate) mod type_gather_pass;
pub(crate) mod use_expand_pass;

pub fn parse() -> Result<(), Box<dyn Error>> {
  let pass_1 = type_gather_pass::parse()?;
  use_expand_pass::parse(pass_1)?;

  Ok(())
}