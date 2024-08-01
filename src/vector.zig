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

/// In-place incremetal assignment
pub fn incr(self: *@This(), other: @This()) void {
    self.x += other.x;
    self.y += other.y;
}

/// Component-wise subtraction
pub inline fn sub(self: @This(), other: @This()) @This() {
    return @This(){
        .x = self.x - other.x,
        .y = self.y - other.y,
    };
}

/// In-place decremetal assignment
pub fn decr(self: *@This(), other: @This()) void {
    self.x -= other.x;
    self.y -= other.y;
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

/// Returns the length of a vector, squared.
/// Gauranteed to be non-negative.
pub inline fn length_squared(self: @This()) f32 {
    return @abs(self.dot(self));
}

/// Returns the length of a vector
pub inline fn length(self: @This()) f32 {
    return std.math.sqrt(self.length_squared());
}

pub inline fn distance_squared(self: @This(), other: @This()) f32 {
    return self.sub(other).length_squared();
}

pub inline fn distance(self: @This(), other: @This()) f32 {
    return self.sub(other).length();
}

pub inline fn close_squared(self: @This(), other: @This(), epsilon_squared: f32) bool {
    return self.distance_squared(other) < epsilon_squared;
}

pub inline fn close(self: @This(), other: @This(), epsilon: f32) bool {
    return self.distance(other) < epsilon;
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

/// Format function to be used with `std.fmt` functions, like `std.debug.print`.
///
/// Represents the string in the format "(x, y)"
pub fn format(self: @This(), comptime fmt: []const u8, options: std.fmt.FormatOptions, writer: anytype) !void {
    _ = options;
    _ = fmt;
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();

    const out = try std.fmt.allocPrint(arena.allocator(), "({d}, {d})", .{ self.x, self.y });

    try writer.print("{s}", .{out});
}
