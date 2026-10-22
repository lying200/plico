fn main() {
    let result = std::env::args()
        .nth(1)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))
        .and_then(|prompt| plico::askpass::helper(&prompt));
    match result {
        Ok(Some(answer)) => println!("{answer}"),
        // Match Rebased GitAskPassApp: cancellation produces no stdout answer.
        Ok(None) => eprintln!("Authentication request was cancelled"),
        Err(_) => {
            eprintln!("Could not communicate with GitReview authentication session");
            std::process::exit(1);
        }
    }
}
