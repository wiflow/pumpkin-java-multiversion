use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{Protocol, Registry, Step};

pub struct Protocol1_19_1To1_19;

impl Protocol for Protocol1_19_1To1_19 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_19_1,
            to: JavaMinecraftVersion::V_1_19,
        }
    }

    fn register(&self, _reg: &mut Registry) {}
}
