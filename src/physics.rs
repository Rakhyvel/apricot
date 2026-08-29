//! NOTE: This isn't really implemented yet
//! TODO: Maybe an implicit euler + newton solver impl? velocity verlet?

pub struct PositionComponent {
    pub pos: nalgebra_glm::Vec3,
}

pub struct VelocityComponent {
    pub vel: nalgebra_glm::Vec3,
}
