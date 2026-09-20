use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{Protocol, Registry, Step};

pub struct Protocol1_21To1_20_5;

impl Protocol for Protocol1_21To1_20_5 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_21,
            to: JavaMinecraftVersion::V_1_20_5,
        }
    }

    fn register(&self, _reg: &mut Registry) {}
}
