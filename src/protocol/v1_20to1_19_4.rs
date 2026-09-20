use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{Protocol, Registry, Step};

pub struct Protocol1_20To1_19_4;

impl Protocol for Protocol1_20To1_19_4 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_20,
            to: JavaMinecraftVersion::V_1_19_4,
        }
    }

    fn register(&self, _reg: &mut Registry) {}
}
