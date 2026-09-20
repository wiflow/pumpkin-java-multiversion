use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{Protocol, Registry, Step};

pub struct Protocol1_20_5To1_20_3;

impl Protocol for Protocol1_20_5To1_20_3 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_20_5,
            to: JavaMinecraftVersion::V_1_20_3,
        }
    }

    fn register(&self, _reg: &mut Registry) {}
}
