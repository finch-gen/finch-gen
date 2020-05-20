#[rustversion::not(nightly)]
fn print_cfg() {}

#[rustversion::nightly]
fn print_cfg() {
  println!("cargo:rustc-cfg=nightly");
}

fn main() {
  print_cfg();
}
