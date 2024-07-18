x: f32,
y: f32,

pub fn new(x: f32, y: f32) @This() {
    return @This(){ .x = x, .y = y };
}

pub fn zero() @This() {
    return @This(){ .x = 0.0, .y = 0.0 };
}

pub fn add(self: @This(), other: @This()) @This() {
    return @This(){ .x = self.x + other.x, .y = self.y + other.y };
}
