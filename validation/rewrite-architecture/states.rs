#![forbid(unsafe_code)]
// Architecture-only compile probe; no kernel effects or production claims.
mod private { pub trait Sealed {} }
pub trait State: private::Sealed { type Resources; }
pub struct Unmounted;
pub struct Mounted;
pub struct Device(std::fs::File);
pub struct Mount { device: Device }
impl private::Sealed for Unmounted {}
impl private::Sealed for Mounted {}
impl State for Unmounted { type Resources = Device; }
impl State for Mounted { type Resources = Mount; }
pub struct Volume<S: State> { resources: S::Resources }
pub struct Root<'a> { _volume: &'a mut Volume<Mounted> }
pub struct Failure<S: State> { owner: Volume<S> }
impl<S: State> Failure<S> { pub fn recover(self) -> Volume<S> { self.owner } }
impl Volume<Unmounted> {
    pub fn new(file: std::fs::File) -> Self { Self { resources: Device(file) } }
    pub fn mount(self) -> Volume<Mounted> {
        Volume { resources: Mount { device: self.resources } }
    }
}
impl Volume<Mounted> {
    pub fn root(&mut self) -> Root<'_> { Root { _volume: self } }
    pub fn unmount(self, succeed: bool) -> Result<Volume<Unmounted>, Failure<Mounted>> {
        if succeed { Ok(Volume { resources: self.resources.device }) }
        else { Err(Failure { owner: self }) }
    }
}
impl Drop for Device { fn drop(&mut self) { let _ = self.0.metadata(); } }
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_transition_retains_ownership() {
        let mounted = Volume::new(std::fs::File::open("/dev/null").unwrap()).mount();
        let failed = match mounted.unmount(false) { Ok(_) => panic!("unexpected"), Err(e) => e };
        assert!(failed.recover().unmount(true).is_ok());
    }
}
