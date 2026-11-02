//! Which credentials of the Vault a project's abilities may use:
//! `projects/<slug>/access.yaml`, committed (ids only, never values).
//!
//! A grant is the user's yes for one ability and one credential. It is also
//! what brings a credential into a project: a credential granted to nothing
//! here is not used here. The Vault never sees this file; the executor reads
//! it before it hands an ability anything.

use crate::store::Store;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Access {
    /// Ability id → the credentials it may use, in the order granted.
    #[serde(default)]
    pub grants: BTreeMap<String, Vec<String>>,
}

fn file(store: &Store, slug: &str) -> PathBuf {
    store.home().project_dir(slug).join("access.yaml")
}

impl Access {
    pub fn load(store: &Store, slug: &str) -> Result<Self> {
        Ok(store.read_yaml_opt(&file(store, slug))?.unwrap_or_default())
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        store.write_yaml(&file(store, slug), self)
    }

    pub fn granted(&self, ability: &str) -> &[String] {
        self.grants.get(ability).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn allows(&self, ability: &str, credential: &str) -> bool {
        self.granted(ability).iter().any(|c| c == credential)
    }

    /// The abilities a credential is granted to here.
    pub fn abilities_of(&self, credential: &str) -> Vec<String> {
        self.grants.iter().filter(|(_, cs)| cs.iter().any(|c| c == credential)).map(|(a, _)| a.clone()).collect()
    }

    /// Every credential used here.
    pub fn credentials(&self) -> Vec<String> {
        let mut out: Vec<String> = self.grants.values().flatten().cloned().collect();
        out.sort();
        out.dedup();
        out
    }

    /// Returns whether anything changed.
    pub fn grant(&mut self, ability: &str, credential: &str) -> bool {
        let list = self.grants.entry(ability.to_string()).or_default();
        if list.iter().any(|c| c == credential) {
            return false;
        }
        list.push(credential.to_string());
        true
    }

    pub fn revoke(&mut self, ability: &str, credential: &str) -> bool {
        let Some(list) = self.grants.get_mut(ability) else { return false };
        let before = list.len();
        list.retain(|c| c != credential);
        let changed = list.len() != before;
        if list.is_empty() {
            self.grants.remove(ability);
        }
        changed
    }

    /// A credential gone from the Vault: gone from every grant. Returns the abilities it was granted to.
    pub fn forget(&mut self, credential: &str) -> Vec<String> {
        let abilities = self.abilities_of(credential);
        for a in &abilities {
            self.revoke(a, credential);
        }
        abilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grants_revokes_and_forgets() {
        let mut a = Access::default();
        assert!(a.grant("x-post", "cred_1"));
        assert!(!a.grant("x-post", "cred_1"));
        assert!(a.grant("x-read", "cred_1"));
        assert!(a.grant("x-read", "cred_2"));
        assert!(a.allows("x-post", "cred_1") && !a.allows("x-post", "cred_2"));
        assert_eq!(a.abilities_of("cred_1"), vec!["x-post", "x-read"]);
        assert_eq!(a.credentials(), vec!["cred_1", "cred_2"]);
        assert_eq!(a.forget("cred_1"), vec!["x-post", "x-read"]);
        assert!(!a.grants.contains_key("x-post"));
        assert!(a.revoke("x-read", "cred_2"));
        assert!(a.grants.is_empty());
    }
}
