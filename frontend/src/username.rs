use gloo_storage::{LocalStorage, Storage, errors::StorageError};

const LOCALSTORAGE_KEY: &str = "username";

pub fn get() -> Option<String> {
    match LocalStorage::get::<String>(LOCALSTORAGE_KEY) {
        Ok(username) if username.is_empty() => None,
        Ok(username) => Some(username),
        Err(StorageError::KeyNotFound(_)) => None,
        Err(e) => panic!("Failed to read username from LocalStorage: {e:?}"),
    }
}

pub fn set(s: String) {
    LocalStorage::set::<String>(LOCALSTORAGE_KEY, s).unwrap();
}
