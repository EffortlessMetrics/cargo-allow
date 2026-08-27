// Keep the executable and library surfaces on the same implementation while
// the command modules remain crate-private. The public dispatcher is the
// intended integration-test seam; the binary continues to own process exit
// behavior in `main`.
include!("main.rs");
