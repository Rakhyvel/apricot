# Apricot ECS Library
This library provides basic utilities for making games and *ap*plications. These include:
- Entity-Component-System worlds
- Scene stacks
- Font cacheing
- Builin GUI components and systems

### Installation and Build File
`zig fetch --save https://github.com/rakhyvel/apricot/archive/[commit_hash].tar.gz`

```zig
const std = @import("std");
// Import the apricot build file
const apricot = @import("third-party/apricot/build.zig");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const exe = b.addExecutable(.{
        .name = "raycast",
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });

    // Build Apricot
    var apricot_package = apricot.init(b);
    apricot_package.link(exe, .{
        .image = "path\to\sdl2_image\lib",
        .ttf = "path\to\sdl2_ttf\lib",
    });
    // Add apricot's "sdl2" subpackage
    exe.root_module.addImport("sdl2", apricot_package.sdl2());
    // Add apricot package
    exe.root_module.addImport("apricot", apricot_package.get_module(b));

    b.installArtifact(exe);
}
```