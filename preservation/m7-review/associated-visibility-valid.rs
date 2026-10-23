#![forbid(unsafe_code)]
pub trait State { type Resources; }
pub struct Mounted;
mod details {
    pub struct Resources;
    impl super::State for super::Mounted { type Resources = Resources; }
}
pub type Projected = <Mounted as State>::Resources;
