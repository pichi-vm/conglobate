extern crate states;
struct Probe;
impl Probe {
    fn rejected(mut volume: states::Volume<states::Mounted>) {
        let root = volume.root();
        let _ = volume.unmount(true);
        drop(root);
    }
}
fn main() {}
