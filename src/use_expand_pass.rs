use std::fs::File;
use std::io::Read;
use std::error::Error;
use std::path::PathBuf;
use std::collections::HashMap;

use crate::type_gather_pass;

#[derive(Debug)]
pub enum Item {
  Mod(ModulePath),
  Struct,
  Enum,
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ModulePath {
  pub path: PathBuf,
  pub mods: Vec<String>,
}

impl ToString for ModulePath {
  fn to_string(&self) -> String {
    if self.mods.len() > 0 {
      format!("{}:{}", self.path.display(), self.mods.join("/"))
    } else {
      format!("{}", self.path.display())
    }
  }
}

impl std::fmt::Debug for ModulePath {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.to_string())
  }
}

impl ModulePath {
  fn base(mut self) -> Self {
    self.path.pop();
    self.mods = vec![];
    self
  }

  fn join(mut self, join: impl Into<PathBuf>) -> Self {
    self.path.push(join.into());
    self
  }

  fn join_mod(mut self, join: String) -> Self {
    self.mods.push(join);
    self
  }
}

impl From<PathBuf> for ModulePath {
  fn from(path: PathBuf) -> Self {
    Self {
      path,
      mods: vec![],
    }
  }
}

#[derive(Debug)]
pub struct Context {
  pub items: HashMap<String, Item>,
  pub parent: Option<ModulePath>,
}

#[derive(Debug)]
pub struct State {
  pub contexts: HashMap<ModulePath, Context>,
}

impl State {
  fn new() -> Self {
    Self {
      contexts: HashMap::new(),
    }
  }

  fn parse_file(state: &mut State, path: &ModulePath, parent: Option<ModulePath>) -> Result<Context, Box<dyn Error>> {
    let mut file = File::open(&path.path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    let ast = syn::parse_file(&content)?;

    Self::parse(state, path, parent, &ast.items)
  }

  fn parse(state: &mut State, path: &ModulePath, parent: Option<ModulePath>, items: &Vec<syn::Item>) -> Result<Context, Box<dyn Error>> {
    let mut ctx = Context {
      items: HashMap::new(),
      parent,
    };

    for item in items {
      match item {
        syn::Item::Use(use_stmt) => {
          println!("{:#?}", use_stmt);
        }

        _ => {},
      }
    }

    Ok(ctx)
  }
}

pub fn parse(pass_1: type_gather_pass::State) -> Result<(), Box<dyn Error>> {
  let mut state = State::new();

  let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

  let path = ModulePath::from(manifest_dir.join("src/lib.rs"));
  let ctx = State::parse_file(&mut state, &path, None)?;
  state.contexts.insert(path, ctx);

  println!("{:#?}", state);

  Ok(())
}