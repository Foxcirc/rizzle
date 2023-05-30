   Compiling rizzle v0.2.0 (/home/Jesus/Desktop/Projects/rizzle)
warning: field `license_token` is never read
   --> src/lib.rs:271:5
    |
269 | pub struct User {
    |            ---- field in this struct
270 |     api_token: String,
271 |     license_token: String,
    |     ^^^^^^^^^^^^^
    |
    = note: `User` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis
    = note: `#[warn(dead_code)]` on by default

warning: `rizzle` (lib) generated 1 warning
   Compiling dizzle v0.1.0 (/home/Jesus/Desktop/Projects/rizzle/dizzle)
warning: variable does not need to be mutable
  --> dizzle/src/main.rs:20:9
   |
20 |     let mut session = rizzle::Session::new(credentials)?;
   |         ----^^^^^^^
   |         |
   |         help: remove this `mut`
   |
   = note: `#[warn(unused_mut)]` on by default

