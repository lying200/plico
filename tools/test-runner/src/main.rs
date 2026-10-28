//! Dependency-free launcher. Build tools keep their environment; test processes
//! receive isolated homes/configs before any Rust threads or Git children start.
use std::{
    env, fs, io,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const NEXTEST_VERSION: &str = "0.9.150";
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn canonical(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = fs::canonicalize(path)?;
    // Git for Windows does not interpret verbatim Win32 config paths as Rust
    // does. Keep normal absolute drive/UNC paths when crossing that boundary.
    #[cfg(windows)]
    if let Some(text) = path.to_str() {
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return Ok(PathBuf::from(format!(r"\\{rest}")));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return Ok(PathBuf::from(rest));
        }
    }
    Ok(path)
}

fn logged(command: &mut Command, run: &Path, suite: &str) -> Result<std::process::ExitStatus> {
    let stdout_file = fs::File::create(run.join(format!("{suite}-stdout.log")))?;
    let stderr_file = fs::File::create(run.join(format!("{suite}-stderr.log")))?;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    fn tee(mut input: impl Read, mut file: fs::File, mut screen: impl Write) -> io::Result<()> {
        let mut buffer = [0; 8192];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                return Ok(());
            }
            file.write_all(&buffer[..count])?;
            screen.write_all(&buffer[..count])?;
        }
    }
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let stdout = std::thread::spawn(move || tee(out, stdout_file, io::stdout()));
    let stderr = std::thread::spawn(move || tee(err, stderr_file, io::stderr()));
    let status = child.wait()?;
    stdout.join().map_err(|_| "stdout logger panicked")??;
    stderr.join().map_err(|_| "stderr logger panicked")??;
    Ok(status)
}

fn quoted(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

fn unique(parent: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(parent)?;
    for sequence in 0..100 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = parent.join(format!("{:x}-{nanos:x}-{sequence:x}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "cannot allocate test directory",
    ))
}

fn tool(name: &str, override_key: &str) -> Result<PathBuf> {
    if let Some(path) = env::var_os(override_key) {
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(format!(
                "{override_key} is not an executable file: {}",
                path.display()
            )
            .into());
        }
        // Do not resolve symlinks: Unix rustup proxies select the tool by argv[0].
        return Ok(std::path::absolute(path)?);
    }
    for dir in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let path = dir.join(format!("{name}{}", env::consts::EXE_SUFFIX));
        if path.is_file() {
            return Ok(std::path::absolute(path)?);
        }
    }
    Err(
        format!("Missing {name}: install it on PATH or set {override_key} to its executable")
            .into(),
    )
}

fn version(tool: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new(tool).args(args).output()?;
    if !out.status.success() {
        return Err(format!(
            "{}: {}",
            tool.display(),
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

fn isolate(command: &mut Command, home: &Path, git: &Path) -> Result<()> {
    for (key, _) in env::vars_os() {
        let upper = key.to_string_lossy().to_ascii_uppercase();
        if upper.starts_with("GIT_")
            || upper.starts_with("GITUI_")
            || matches!(
                upper.as_str(),
                "EDITOR"
                    | "VISUAL"
                    | "SSH_ASKPASS"
                    | "SSH_AUTH_SOCK"
                    | "BASH_ENV"
                    | "ENV"
                    | "CDPATH"
                    | "HTTP_PROXY"
                    | "HTTPS_PROXY"
                    | "ALL_PROXY"
                    | "NO_PROXY"
            )
        {
            command.env_remove(key);
        }
    }
    for (key, folder) in [
        ("HOME", "home"),
        ("USERPROFILE", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_DATA_HOME", "data"),
        ("APPDATA", "roaming"),
        ("LOCALAPPDATA", "local"),
        ("TMP", "tmp"),
        ("TEMP", "tmp"),
        ("TMPDIR", "tmp"),
    ] {
        let path = home.join(folder);
        fs::create_dir_all(&path)?;
        command.env(key, path);
    }
    let config = home.join("gitconfig");
    fs::write(
        &config,
        "[core]\n autocrlf = false\n safecrlf = false\n[init]\n defaultBranch = main\n[user]\n name = Plico Test\n email = test@example.invalid\n[commit]\n gpgsign = false\n[tag]\n gpgsign = false\n[gc]\n auto = 0\n[maintenance]\n auto = false\n[credential]\n helper =\n",
    )?;
    let mut paths = vec![git.parent().ok_or("Git path has no parent")?.to_path_buf()];
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    command
        .env("PATH", env::join_paths(paths)?)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("TZ", "UTC");
    Ok(())
}

fn child(args: &[String]) -> Result<i32> {
    if args.len() < 3 {
        return Err("internal child: expected run directory, Git and binary".into());
    }
    // Repositories must not live below the checkout: init tests intentionally
    // inspect ancestors themselves, independently of Git's discovery ceiling.
    let parent = env::var_os("PLICO_TEST_TEMP_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("plico-tests"));
    fs::create_dir_all(&parent)?;
    let parent = canonical(parent)?;
    if parent
        .ancestors()
        .any(|p| fs::symlink_metadata(p.join(".git")).is_ok())
    {
        return Err("Test temp root is inside a Git checkout; set PLICO_TEST_TEMP_ROOT to an external directory".into());
    }
    let home = unique(&parent)?;
    let records = Path::new(&args[0]).join("cases");
    fs::create_dir_all(&records)?;
    let record = records
        .join(home.file_name().unwrap())
        .with_extension("txt");
    fs::write(&record, home.to_string_lossy().as_bytes())?;
    let mut command = Command::new(&args[2]);
    command.args(&args[3..]);
    isolate(&mut command, &home, Path::new(&args[1]))?;
    let status = command.status()?;
    if status.success() {
        if let Err(error) = fs::remove_dir_all(&home) {
            eprintln!("test cleanup failed: {}: {error}", home.display());
            return Ok(1);
        }
        fs::remove_file(record)?;
    } else {
        eprintln!("test failed ({status}); isolated files: {}", home.display());
    }
    Ok(status.code().unwrap_or(1))
}

fn main_run() -> Result<i32> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--child") {
        return child(&args[1..]);
    }
    let suite = args.first().map(String::as_str).unwrap_or("doctor");
    if !matches!(suite, "doctor" | "core" | "desktop" | "navigation" | "all") {
        return Err("usage: plico-test-runner [doctor|core|desktop|navigation|all] [nextest filter arguments]".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    // rustup must resolve this checkout's rust-toolchain.toml, even when the
    // caller starts the launcher from a different working directory.
    env::set_current_dir(root)?;
    let git = tool("git", "PLICO_TEST_GIT")?;
    let nextest = tool("cargo-nextest", "PLICO_TEST_NEXTEST")?;
    let cargo = tool("cargo", "PLICO_TEST_CARGO")?;
    let rustc = tool("rustc", "PLICO_TEST_RUSTC")?;
    let rust = version(&rustc, &["-vV"])?;
    let host = rust
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("rustc host unavailable")?;
    if env::var("CARGO_BUILD_TARGET").is_ok_and(|target| target != host) {
        return Err("Native tests only: unset the cross-compilation CARGO_BUILD_TARGET".into());
    }
    let nv = version(&nextest, &["nextest", "--version"])?;
    if nv.split_whitespace().nth(1) != Some(NEXTEST_VERSION) {
        return Err(format!("Require cargo-nextest {NEXTEST_VERSION}, found {nv}").into());
    }
    let metadata = format!(
        "git: {}\n{}\nnextest: {}\n{nv}\ncargo: {}\n{}\n{rust}\n",
        git.display(),
        version(&git, &["--version"])?,
        nextest.display(),
        cargo.display(),
        version(&cargo, &["--version"])?
    );
    println!("{metadata}");
    if suite == "doctor" {
        return Ok(0);
    }
    let run = unique(&root.join("artifacts/test-runs"))?;
    fs::write(run.join("environment.txt"), metadata)?;
    println!("Results: {}", run.display());
    let runner = [
        env::current_exe()?.to_string_lossy().into_owned(),
        "--child".into(),
        run.to_string_lossy().into_owned(),
        git.to_string_lossy().into_owned(),
    ];
    let runner_config = format!(
        "target.{host}.runner = [{}]",
        runner
            .iter()
            .map(|v| quoted(v))
            .collect::<Vec<_>>()
            .join(",")
    );
    let suites = if suite == "all" {
        vec!["core", "desktop", "navigation"]
    } else {
        vec![suite]
    };
    let mut failed = false;
    for suite in suites {
        let config = run.join(format!("{suite}.toml"));
        fs::write(
            &config,
            format!(
                "store.dir = {}\n[profile.default]\ntest-threads = {}\nfail-fast = false\nretries = 0\nslow-timeout = {{ period = '60s', terminate-after = 3 }}\n[profile.default.junit]\npath = '{suite}.xml'\n",
                quoted(&run.to_string_lossy()),
                if suite == "core" { 2 } else { 1 }
            ),
        )?;
        let mut command = Command::new(&nextest);
        // Runner defaults must not silently inherit a user's retry/filter policy.
        for (key, _) in env::vars_os() {
            if key
                .to_string_lossy()
                .to_ascii_uppercase()
                .starts_with("NEXTEST_")
            {
                command.env_remove(key);
            }
        }
        command
            .current_dir(root)
            .args([
                "nextest",
                "run",
                "--locked",
                "--user-config-file",
                "none",
                "--config-file",
            ])
            .arg(&config)
            .args(["--config", &runner_config])
            .env("CARGO", &cargo)
            .env("RUSTC", &rustc);
        // A caller's target runner must not bypass isolation.
        command.env_remove(format!(
            "CARGO_TARGET_{}_RUNNER",
            host.to_uppercase().replace('-', "_")
        ));
        if suite == "core" {
            command.arg("--no-default-features");
        } else {
            command.args(["--features", "desktop-tests", "--bin", "plico"]);
            if suite == "navigation" {
                command.args([
                    "--config",
                    "profile.test.debug-assertions=false",
                    "-E",
                    "test(diff_tab::editable_navigation_tests)",
                ]);
            }
        }
        command.args(&args[1..]);
        let status = logged(&mut command, &run, suite)?;
        fs::write(run.join(format!("{suite}-status.txt")), status.to_string())?;
        failed |= !status.success();
    }
    Ok(i32::from(failed))
}

fn main() -> ExitCode {
    match main_run() {
        Ok(0) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("plico-test-runner: {error}");
            ExitCode::FAILURE
        }
    }
}
