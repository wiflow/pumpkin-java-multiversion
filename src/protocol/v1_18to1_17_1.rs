use pumpkin_util::version::JavaMinecraftVersion;

use crate::api::{Protocol, Registry, Step};

pub struct Protocol1_18To1_17_1;

impl Protocol for Protocol1_18To1_17_1 {
    fn step(&self) -> Step {
        Step {
            from: JavaMinecraftVersion::V_1_18,
            to: JavaMinecraftVersion::V_1_17_1,
        }
    }

    fn register(&self, _reg: &mut Registry) {}
}
