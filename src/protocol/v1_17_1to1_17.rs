use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{Protocol, Registry, Step};

pub struct Protocol1_17_1To1_17;

impl Protocol for Protocol1_17_1To1_17 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_17_1,
            to: JavaMinecraftVersion::V_1_17,
        }
    }

    fn register(&self, _reg: &mut Registry) {}
}
