//! Peer link tokens in the Keychain across a home move or a rename.
//!
//! Each home keeps its tokens under its own service, `<product> peer link
//! <hash of its secrets dir>`, so a test or dev home never sees the real
//! daemon's links. Both parts change with the rename: the product name, and
//! the path once `~/.thehermes` moves to `~/.flotilla`. Without this, every
//! linked computer's token would be left under the old service, and the
//! links would stop working after the move.
//!
//! On open, tokens found under an earlier service (any product name, this
//! dir or a home this one moved from) and missing under the current one are
//! copied over. The old items stay where they were, so a rollback to the
//! previous release still finds them; removing a link removes it under every
//! service, so a revoked link never comes back from there.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The product names services were named with, current first.
const PRODUCTS: [&str; 2] = ["Flotilla", "The Hermes"];

/// Generic passwords by service and account, as the login Keychain keeps
/// them; tests use a map.
pub(super) trait PasswordStore {
    /// account -> secret for every item under `service`.
    fn list(&self, service: &str) -> anyhow::Result<HashMap<String, String>>;
    fn put(&self, service: &str, account: &str, secret: &str) -> anyhow::Result<()>;
    /// Removing an absent item is not an error.
    fn delete(&self, service: &str, account: &str) -> anyhow::Result<()>;
}

fn digest(secrets_dir: &Path) -> String {
    let digest = Sha256::digest(secrets_dir.to_string_lossy().as_bytes());
    hex::encode(digest)[..12].to_string()
}

/// The service `product` names the tokens of `secrets_dir` under.
pub(super) fn service(product: &str, secrets_dir: &Path) -> String {
    format!("{product} peer link {}", digest(secrets_dir))
}

/// This home's service now.
pub(super) fn current_service(secrets_dir: &Path) -> String {
    service(PRODUCTS[0], secrets_dir)
}

/// Every earlier service the tokens of `secrets_dir` may be under: each
/// product name for this dir and for the secrets dirs of `former_homes`,
/// except the current service.
pub(super) fn legacy_services(secrets_dir: &Path, former_homes: &[PathBuf]) -> Vec<String> {
    let current = current_service(secrets_dir);
    let dirs = std::iter::once(secrets_dir.to_path_buf())
        .chain(former_homes.iter().map(|home| home.join("secrets")));
    let mut services = Vec::new();
    for dir in dirs {
        for product in PRODUCTS {
            let name = service(product, &dir);
            if name != current && !services.contains(&name) {
                services.push(name);
            }
        }
    }
    services
}

/// The homes the home holding `secrets_dir` may have been before: the
/// sibling pre-rename homes (`~/.thehermes`, `~/.gravity`), and the `from`
/// of every home-move log in it, finished or set aside (`migrate-home*.json`),
/// which covers a home moved by hand with `migrate-home --from`.
pub(super) fn former_homes(secrets_dir: &Path) -> Vec<PathBuf> {
    let Some(home) = secrets_dir.parent() else {
        return Vec::new();
    };
    let mut homes: Vec<PathBuf> = home
        .parent()
        .map(|user| {
            crate::brand::LEGACY_HOME_DIR_NAMES
                .iter()
                .map(|name| user.join(name))
                .collect()
        })
        .unwrap_or_default();
    let logs = std::fs::read_dir(home)
        .map(|entries| entries.flatten().map(|e| e.path()).collect::<Vec<_>>())
        .unwrap_or_default();
    for log in logs {
        let name = log.file_name().map(|n| n.to_string_lossy().into_owned());
        if !name.is_some_and(|n| n.starts_with("migrate-home") && n.ends_with(".json")) {
            continue;
        }
        let from = std::fs::read_to_string(&log)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| value["from"].as_str().map(PathBuf::from));
        if let Some(from) = from {
            if !homes.contains(&from) && from != home {
                homes.push(from);
            }
        }
    }
    homes
}

/// Copies every token under one of `legacy` that `current` lacks into
/// `current`, newest service first; returns how many were copied.
pub(super) fn carry_over(
    store: &impl PasswordStore,
    current: &str,
    legacy: &[String],
) -> anyhow::Result<usize> {
    let mut have = store.list(current)?;
    let mut copied = 0;
    for service in legacy {
        for (account, secret) in store.list(service)? {
            if have.contains_key(&account) {
                continue;
            }
            store.put(current, &account, &secret)?;
            have.insert(account, secret);
            copied += 1;
        }
    }
    Ok(copied)
}

/// Removes `account` under `current` and every one of `legacy`.
pub(super) fn remove_everywhere(
    store: &impl PasswordStore,
    current: &str,
    legacy: &[String],
    account: &str,
) -> anyhow::Result<()> {
    store.delete(current, account)?;
    for service in legacy {
        store.delete(service, account)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    /// The login Keychain as a map of (service, account) -> secret.
    #[derive(Default)]
    struct Fake(RefCell<BTreeMap<(String, String), String>>);

    impl PasswordStore for Fake {
        fn list(&self, service: &str) -> anyhow::Result<HashMap<String, String>> {
            Ok(self
                .0
                .borrow()
                .iter()
                .filter(|((s, _), _)| s == service)
                .map(|((_, a), secret)| (a.clone(), secret.clone()))
                .collect())
        }
        fn put(&self, service: &str, account: &str, secret: &str) -> anyhow::Result<()> {
            self.0
                .borrow_mut()
                .insert((service.into(), account.into()), secret.into());
            Ok(())
        }
        fn delete(&self, service: &str, account: &str) -> anyhow::Result<()> {
            self.0
                .borrow_mut()
                .remove(&(service.to_string(), account.to_string()));
            Ok(())
        }
    }

    /// The Hermes kept `~/.thehermes`'s links under "The Hermes peer link
    /// <hash of ~/.thehermes/secrets>"; after the move to `~/.flotilla` they
    /// are found and copied under the new service, and the old items stay
    /// for a rollback.
    #[test]
    fn links_survive_the_move_and_the_rename() {
        let user = Path::new("/Users/me");
        let (old, new) = (user.join(".thehermes"), user.join(".flotilla"));
        let store = Fake::default();
        let before = service("The Hermes", &old.join("secrets"));
        assert!(before.starts_with("The Hermes peer link "));
        store.put(&before, "peer-a", "tok-a").unwrap();
        store.put(&before, "peer-b", "tok-b").unwrap();

        let secrets = new.join("secrets");
        let current = current_service(&secrets);
        assert!(current.starts_with("Flotilla peer link "));
        let legacy = legacy_services(&secrets, std::slice::from_ref(&old));
        assert!(legacy.contains(&before), "{legacy:?}");
        assert!(!legacy.contains(&current));

        assert_eq!(carry_over(&store, &current, &legacy).unwrap(), 2);
        let now = store.list(&current).unwrap();
        assert_eq!(now.get("peer-a").map(String::as_str), Some("tok-a"));
        assert_eq!(now.get("peer-b").map(String::as_str), Some("tok-b"));
        assert_eq!(store.list(&before).unwrap().len(), 2, "kept for a rollback");

        // A second open copies nothing, and a token re-issued under the new
        // service is never overwritten by the old one.
        store.put(&current, "peer-a", "tok-a2").unwrap();
        assert_eq!(carry_over(&store, &current, &legacy).unwrap(), 0);
        assert_eq!(
            store
                .list(&current)
                .unwrap()
                .get("peer-a")
                .map(String::as_str),
            Some("tok-a2")
        );

        // An unlinked peer is gone under every name, so a rollback can't
        // bring it back.
        remove_everywhere(&store, &current, &legacy, "peer-b").unwrap();
        assert!(!store.list(&current).unwrap().contains_key("peer-b"));
        assert!(!store.list(&before).unwrap().contains_key("peer-b"));
    }

    /// A home that did not move still has the rename's product name to
    /// carry over from.
    #[test]
    fn a_home_that_stayed_put_is_renamed_too() {
        let secrets = Path::new("/srv/fl/secrets");
        let legacy = legacy_services(secrets, &[]);
        assert_eq!(legacy, [service("The Hermes", secrets)]);
    }

    #[test]
    fn former_homes_come_from_the_siblings_and_the_move_logs() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("chosen");
        std::fs::create_dir_all(home.join("secrets")).unwrap();
        let log = serde_json::json!({ "from": "/elsewhere/old", "to": home });
        std::fs::write(home.join("migrate-home.json"), log.to_string()).unwrap();
        let hop1 = serde_json::json!({ "from": "/elsewhere/older", "to": "/elsewhere/old" });
        std::fs::write(
            home.join("migrate-home.older-to-old.json"),
            hop1.to_string(),
        )
        .unwrap();
        let homes = former_homes(&home.join("secrets"));
        for want in [
            root.path().join(".thehermes"),
            root.path().join(".gravity"),
            PathBuf::from("/elsewhere/old"),
            PathBuf::from("/elsewhere/older"),
        ] {
            assert!(homes.contains(&want), "{want:?} in {homes:?}");
        }
    }
}
