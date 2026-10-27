pub struct RenderList {
    pub ops: Vec<Op>,
}

pub enum Op {
    Brightness(f32),
    Saturation(f32),
}
