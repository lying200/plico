use super::*;

fn account() -> Account {
    Account {
        provider: Provider::GitLab,
        server: "https://storage-fixture.invalid".into(),
        login: "storage-fixture".into(),
    }
}
fn key(account: &Account) -> String {
    format!("hosting:token:{}", account.id())
}
fn stored(store: &Store, account: &Account) -> Option<String> {
    store.get(&key(account)).unwrap()
}
fn token(value: &str) -> Credentials {
    Credentials::token(value.into())
}
fn read(store: &Store, account: &Account) -> Result<Credentials, String> {
    credentials_from(store, account, &Cancel::default(), |_, _, _| {
        panic!("An unexpired token must not refresh")
    })
}
#[test]
fn saving_can_retry_either_failed_write_without_duplicate_accounts() {
    let account = account();
    for failing_key in [key(&account), "hosting:accounts".into()] {
        let store = Store::default();
        store.fail_next_mutation(&failing_key, false);
        assert!(save_to(&store, &account, &token("first"), &Cancel::default()).is_err());
        assert!(load(&store).unwrap().is_empty());
        save_to(&store, &account, &token("retry"), &Cancel::default()).unwrap();
        assert_eq!(load(&store).unwrap(), vec![account.clone()]);
        assert_eq!(read(&store, &account).unwrap().access, "retry");
        save_to(&store, &account, &token("updated"), &Cancel::default()).unwrap();
        assert_eq!(load(&store).unwrap(), vec![account.clone()]);
        assert_eq!(read(&store, &account).unwrap().access, "updated");
    }
}
#[test]
fn removal_can_retry_index_or_token_failure_without_affecting_other_accounts() {
    let account = account();
    let other = Account {
        login: "other".into(),
        ..account.clone()
    };
    for (failing_key, remove) in [("hosting:accounts".into(), false), (key(&account), true)] {
        let store = Store::default();
        save_to(&store, &account, &token("first"), &Cancel::default()).unwrap();
        save_to(&store, &other, &token("other"), &Cancel::default()).unwrap();
        store.fail_next_mutation(&failing_key, remove);
        assert!(remove_from(&store, &account).is_err());
        assert_eq!(read(&store, &other).unwrap().access, "other");
        remove_from(&store, &account).unwrap();
        assert_eq!(load(&store).unwrap(), vec![other.clone()]);
        assert!(stored(&store, &account).is_none());
        assert!(read(&store, &account).is_err());
        remove_from(&store, &account).unwrap();
    }
}
#[test]
fn completed_logout_or_relogin_wins_over_inflight_refresh() {
    for relogin in [false, true] {
        let store = Store::default();
        let account = account();
        let mut expired = token("expired");
        expired.expires_at = Some(0);
        save_to(&store, &account, &expired, &Cancel::default()).unwrap();
        let result = credentials_from(&store, &account, &Cancel::default(), |a, c, cancel| {
            assert_eq!(c.access, "expired");
            if relogin {
                save_to(&store, a, &token("new-login"), cancel).unwrap();
            } else {
                remove_from(&store, a).unwrap();
            }
            Ok(token("late-refresh"))
        });
        assert_eq!(
            result.err().as_deref(),
            Some("Account changed during authorization")
        );
        if relogin {
            assert_eq!(read(&store, &account).unwrap().access, "new-login");
        } else {
            assert!(load(&store).unwrap().is_empty());
            assert!(stored(&store, &account).is_none());
        }
    }
}
#[test]
fn failed_refresh_persistence_preserves_previous_credentials() {
    let store = Store::default();
    let account = account();
    let mut expired = token("expired");
    expired.expires_at = Some(0);
    save_to(&store, &account, &expired, &Cancel::default()).unwrap();
    let before = stored(&store, &account);
    store.fail_next_mutation(&key(&account), false);
    let result = credentials_from(&store, &account, &Cancel::default(), |_, _, _| {
        Ok(token("rotated"))
    });
    assert!(result.is_err());
    assert_eq!(stored(&store, &account), before);
    assert_eq!(load(&store).unwrap(), vec![account]);
}

#[test]
fn cancelled_save_or_refresh_does_not_mutate_persistence() {
    let store = Store::default();
    let account = account();
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(save_to(&store, &account, &token("cancelled"), &cancelled).is_err());
    assert!(load(&store).unwrap().is_empty());
    assert!(stored(&store, &account).is_none());
    let mut expired = token("expired");
    expired.expires_at = Some(0);
    save_to(&store, &account, &expired, &Cancel::default()).unwrap();
    let before = stored(&store, &account);
    let cancel = Cancel::default();
    let result = credentials_from(&store, &account, &cancel, |_, _, c| {
        c.cancel();
        Ok(token("late-refresh"))
    });
    assert_eq!(result.err().as_deref(), Some("Cancelled"));
    assert_eq!(stored(&store, &account), before);
}
#[test]
fn removed_index_rejects_late_refresh_after_delete_failure() {
    let store = Store::default();
    let account = account();
    let mut expired = token("expired");
    expired.expires_at = Some(0);
    save_to(&store, &account, &expired, &Cancel::default()).unwrap();
    let before = stored(&store, &account);
    let result = credentials_from(&store, &account, &Cancel::default(), |a, _, _| {
        store.fail_next_mutation(&key(a), true);
        assert!(remove_from(&store, a).is_err());
        assert!(load(&store).unwrap().is_empty());
        Ok(token("late-refresh"))
    });
    assert_eq!(
        result.err().as_deref(),
        Some("Account changed during authorization")
    );
    assert_eq!(stored(&store, &account), before);
    remove_from(&store, &account).unwrap();
    assert!(stored(&store, &account).is_none());
}

#[test]
fn orphaned_tokens_cannot_supply_credentials_or_start_refresh() {
    for expires_at in [None, Some(0)] {
        let store = Store::default();
        let account = account();
        let mut credentials = token("orphan");
        credentials.expires_at = expires_at;
        save_to(&store, &account, &credentials, &Cancel::default()).unwrap();
        store.fail_next_mutation(&key(&account), true);
        assert!(remove_from(&store, &account).is_err());
        assert!(load(&store).unwrap().is_empty());
        let result = credentials_from(&store, &account, &Cancel::default(), |_, _, _| {
            panic!("Removed account must not initiate an OAuth refresh")
        });
        assert_eq!(
            result.err().as_deref(),
            Some("Account needs to log in again")
        );
        remove_from(&store, &account).unwrap();
        assert!(stored(&store, &account).is_none());
    }
}
