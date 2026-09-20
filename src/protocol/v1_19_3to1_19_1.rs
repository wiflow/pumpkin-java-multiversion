use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{Protocol, Registry, Step};

pub struct Protocol1_19_3To1_19_1;

impl Protocol for Protocol1_19_3To1_19_1 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_19_3,
            to: JavaMinecraftVersion::V_1_19_1,
        }
    }

    fn register(&self, _reg: &mut Registry) {}
}
