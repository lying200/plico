use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn isolated_child_probe() {
    let Some(marker) = env::var_os("PLICO_PROBE_MARKER") else {
        return;
    };
    for key in [
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_CONFIG_COUNT",
        "EDITOR",
        "SSH_AUTH_SOCK",
        "HTTP_PROXY",
    ] {
        assert!(env::var_os(key).is_none(), "inherited {key}");
    }
    let home = PathBuf::from(env::var_os("HOME").unwrap());
    assert!(home.is_dir());
    assert!(
        home.ancestors().all(|p| !p.join(".git").exists()),
        "test home must not be nested in a checkout"
    );
    assert_ne!(home, PathBuf::from("poison-home"));
    let git = |args: &[&str]| {
        let result = Command::new("git").args(args).output().unwrap();
        assert!(result.status.success(), "{result:?}");
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    };
    assert_eq!(git(&["config", "--global", "core.autocrlf"]), "false");
    // A test's deliberate configuration must still override the baseline.
    assert_eq!(
        git(&["-c", "core.autocrlf=true", "config", "core.autocrlf"]),
        "true"
    );
    git(&["config", "--global", "user.name", "Changed inside test"]);
    fs::write(marker, home.to_string_lossy().as_bytes()).unwrap();
    assert!(
        env::var_os("PLICO_PROBE_FAIL").is_none(),
        "intentional failure"
    );
}

#[test]
fn polluted_concurrent_children_have_independent_homes_and_cleanup() {
    let root = env::temp_dir().join(format!(
        "plico runner 中文 {}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let test_temp = root.join("case homes 中文");
    let git = env::var_os("PLICO_TEST_GIT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::split_paths(&env::var_os("PATH").unwrap())
                .map(|dir| dir.join(format!("git{}", env::consts::EXE_SUFFIX)))
                .find(|path| path.is_file())
                .expect("Git required")
        });
    let mut children = Vec::new();
    for index in 0..2 {
        let marker = root.join(format!("marker-{index}"));
        children.push(
            Command::new(env!("CARGO_BIN_EXE_plico-test-runner"))
                .arg("--child")
                .arg(&root)
                .arg(&git)
                .arg(env::current_exe().unwrap())
                .args(["--exact", "isolated_child_probe", "--nocapture"])
                .env("PLICO_PROBE_MARKER", marker)
                .env("PLICO_TEST_TEMP_ROOT", &test_temp)
                .env("GIT_DIR", "invalid-repository")
                .env("GIT_INDEX_FILE", "invalid-index")
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "core.autocrlf")
                .env("GIT_CONFIG_VALUE_0", "true")
                .env("EDITOR", "invalid-editor")
                .env("SSH_AUTH_SOCK", "invalid-socket")
                .env("HTTP_PROXY", "http://127.0.0.1:1")
                .env("HOME", "poison-home")
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let first = fs::read_to_string(root.join("marker-0")).unwrap();
    let second = fs::read_to_string(root.join("marker-1")).unwrap();
    assert_ne!(first, second);
    assert!(!PathBuf::from(first).exists());
    assert!(!PathBuf::from(second).exists());
    let marker = root.join("failed-marker");
    let failed = Command::new(env!("CARGO_BIN_EXE_plico-test-runner"))
        .arg("--child")
        .arg(&root)
        .arg(&git)
        .arg(env::current_exe().unwrap())
        .args(["--exact", "isolated_child_probe"])
        .env("PLICO_PROBE_MARKER", &marker)
        .env("PLICO_TEST_TEMP_ROOT", &test_temp)
        .env("PLICO_PROBE_FAIL", "1")
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let retained = PathBuf::from(fs::read_to_string(marker).unwrap());
    assert!(retained.is_dir(), "failed test evidence must be retained");
    assert!(String::from_utf8_lossy(&failed.stderr).contains("isolated files:"));
    // The launcher retains this owned failure directory outside the report root.
    fs::remove_dir_all(retained.parent().unwrap()).unwrap();
    let nested = root.join("nested");
    fs::create_dir_all(nested.join(".git")).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_plico-test-runner"))
        .arg("--child")
        .arg(&root)
        .arg(&git)
        .arg(env::current_exe().unwrap())
        .env("PLICO_TEST_TEMP_ROOT", nested.join("tmp"))
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("inside a Git checkout"));
    fs::remove_dir_all(root).unwrap();
}
