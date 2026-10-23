use conglobate::step::{PreparationStorage, StoragePreparation, StoragePreparationFailure};
fn cannot_return_with_local_parent<'a, R: PreparationStorage>(prepared: StoragePreparation<'a, R>) -> Box<StoragePreparationFailure<'a, R>> {
    let parent = std::path::PathBuf::from("/temporary-domain-parent");
    prepared.launch(std::process::Command::new("/trusted-helper"), std::time::Instant::now(), &parent).unwrap_err()
}
