//! Read-only diagnostic harness: `cargo run --no-default-features --example refresh_profile -- <repository>`.
//! Includes Git subprocess time; does not benchmark UI/GPU rendering or measure memory.
use plico::{commit_changes::Snapshot, git::{Cancel, Repository}};
fn main() {
    let repo = Repository::new(std::env::args_os().nth(1).expect("repository path"));
    for n in 0..3 {
        let now = std::time::Instant::now();
        match Snapshot::load(&repo, &Cancel::default()) {
            Ok(snapshot) => println!("refresh {n}: {:?}, files={}", now.elapsed(), snapshot.files.len()),
            Err(error) => { eprintln!("refresh {n}: {:?}, error={error}", now.elapsed()); std::process::exit(1); }
        }
    }
}
