x: f32,
y: f32,

pub fn add(self: @This(), other: @This()) @This() {
    return @This(){ .x = self.x + other.x, .y = self.y + other.y };
}
