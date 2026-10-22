extern crate states;
struct Probe;
impl Probe {
    fn rejected(volume: states::Volume<states::Unmounted>) {
        let _ = volume.unmount(true);
    }
}
fn main() {}
