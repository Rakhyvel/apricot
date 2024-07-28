const std = @import("std");

x: f32,
y: f32,

pub fn new(x: f32, y: f32) @This() {
    return @This(){ .x = x, .y = y };
}

pub fn zero() @This() {
    return @This(){ .x = 0.0, .y = 0.0 };
}

/// Component-wise addition
pub fn add(self: @This(), other: @This()) @This() {
    return @This(){ .x = self.x + other.x, .y = self.y + other.y };
}

/// Component-wise subtraction
pub inline fn sub(self: @This(), other: @This()) @This() {
    return @This(){
        .x = self.x - other.x,
        .y = self.y - other.y,
    };
}

/// Component-wise multiplication
pub inline fn mul(self: @This(), other: @This()) @This() {
    return @This(){
        .x = self.x * other.x,
        .y = self.y * other.y,
    };
}

/// Component-wise division
pub inline fn div(self: @This(), other: @This()) @This() {
    return @This(){
        .x = self.x / other.x,
        .y = self.y / other.y,
    };
}

/// Component-wise power
pub inline fn pow(self: @This(), other: @This()) @This() {
    return @This(){
        .x = std.math.pow(f32, self.x, other.x),
        .y = std.math.pow(f32, self.y, other.y),
    };
}

/// Scales a vector by a factor
pub inline fn scale(self: @This(), factor: f32) @This() {
    return @This(){
        .x = self.x * factor,
        .y = self.y * factor,
    };
}

/// Returns the dot product of two vectors
pub inline fn dot(self: @This(), other: @This()) f32 {
    return self.x * other.x + self.y * other.y;
}

/// Returns the length of a vector, squared
pub inline fn length_squared(self: @This()) f32 {
    return self.dot(self);
}

/// Returns the length of a vector
pub inline fn length(self: @This()) f32 {
    return std.math.sqrt(self.length_squared());
}

/// Normalizes a vector to have a magnitude of 1
/// Check that length is not 0 first.
pub inline fn normalize(self: @This()) @This() {
    const len = self.length();
    return @This(){
        .x = self.x / len,
        .y = self.y / len,
    };
}
