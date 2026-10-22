fn main() {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    let sequence = first.as_deref() == Some(std::ffi::OsStr::new("--sequence"));
    let path = if sequence { args.next() } else { first };
    let result = match (path, args.next()) {
        (Some(path), None) => plico::editor::helper(std::path::Path::new(&path), sequence),
        _ => Err(std::io::ErrorKind::InvalidInput.into()),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(_) => {
            eprintln!("Git editor request failed");
            std::process::exit(1);
        }
    }
}
