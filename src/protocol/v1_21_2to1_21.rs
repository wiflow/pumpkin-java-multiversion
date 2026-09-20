use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{Protocol, Registry, Step};

pub struct Protocol1_21_2To1_21;

impl Protocol for Protocol1_21_2To1_21 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_21_2,
            to: JavaMinecraftVersion::V_1_21,
        }
    }

    fn register(&self, _reg: &mut Registry) {}
}
