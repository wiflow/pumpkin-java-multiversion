use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{Protocol, Registry, Step};

pub struct Protocol1_17To1_16_4;

impl Protocol for Protocol1_17To1_16_4 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_17,
            to: JavaMinecraftVersion::V_1_16_4,
        }
    }

    fn register(&self, _reg: &mut Registry) {}
}
