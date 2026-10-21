//! Read-only profiling of the preflight currently started by Log selection.
//! Optional latency budget is diagnostic only, not a hardware-dependent CI test.
use plico::{git::{Cancel, Repository}, reword::Prepared};
use std::time::Instant;

fn main() {
    let repo = Repository::new(std::env::args_os().nth(1).expect("repository"));
    let budget = std::env::args().nth(2).map(|s| s.parse::<u128>().expect("milliseconds"));
    let token = Cancel::default();
    let head = plico::commit_operation::head(&repo, &token).unwrap().expect("HEAD");
    let mut slow = false;
    for n in 0..5 {
        let start = Instant::now();
        let result = Prepared::prepare(&repo, head.clone(), &token);
        let elapsed = start.elapsed().as_millis();
        let outcome = match result {
            Ok(_) => "eligible",
            Err(ref error) if error.contains("protected branch") => "protected",
            Err(error) => panic!("Unexpected preflight failure: {error}"),
        };
        println!("{n}: reword-prepare={elapsed}ms outcome={outcome}");
        slow |= budget.is_some_and(|b| elapsed > b);
    }
    if slow { std::process::exit(1); }
}
