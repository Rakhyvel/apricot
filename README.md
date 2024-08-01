# Apricot ECS Library
Apricot is a lightweight library that provides basic utilities for rapid prototyping and developing games and *ap*plications. It includes features like:
- Easy-to-use `App` SDL2 wrapper
- Entity-Component-System worlds
- Scene stacks
- Font cacheing
- Builin GUI components and systems

## Installation and Build File
1. `zig fetch --save https://github.com/rakhyvel/apricot/archive/[commit_hash].tar.gz`

1. Add this to your `build.zig`
```zig
const std = @import("std");
// Import the apricot build file

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
    var apricot_package = @import("apricot").init(b);
    apricot_package.link(exe, .{
        .image = "path/to/SDL2_image/lib",
        .ttf = "path/to/SDL2_ttf/lib",
    });
    // Add apricot's "sdl2" subpackage
    exe.root_module.addImport("sdl2", apricot_package.sdl2());
    // Add apricot package
    exe.root_module.addImport("apricot", apricot_package.get_module(b));

    b.installArtifact(exe);
}
```
1. Add `.build_config/sdl.json` to your project base path
1. Add `SDL2.dll` to your project base path
1. Add `SDL2_ttf.dll` to your `zig-out/bin` path

## Usage
Below is some starting code for a basic app:
```zig
const std = @import("std");
const apricot = @import("apricot");

pub fn main() !void {
    // Initialize the apricot library
    const width: usize = 640;
    const height: usize = 480;
    var app = try apricot.App.init("Apricot Demo", width, height, std.heap.c_allocator);
    defer app.deinit();
    
    // Create a scene
    const alloc = std.heap.c_allocator;
    var scene = try Scene.init(alloc, &app);

    // Push the scene to the app's scene stack
    app.push_scene(scene.as_scene());

    // Start the app's update/render loop
    try app.start_app();
}

pub const Scene = struct {
    // Scene global members here

    pub fn init(alloc: std.mem.Allocator) !*Scene {
        const self = try alloc.create(Scene);

        // Init scene here

        return self;
    }

    pub fn as_scene(self: *Scene) apricot.scene.Scene_Object {
        return .{
            .self = @ptrCast(self),
            .vtable = &scene_vtable,
        };
    }

    fn deinit(pself: *void) void {
        const self: *Scene = @alignCast(@ptrCast(pself));
        _ = self;
        // Scene deinit-ing here
    }

    fn update(pself: *void) void {
        const self: *Scene = @alignCast(@ptrCast(pself));
        _ = self;
        // Scene updating here
        std.debug.print("Scene update!\n", .{});
    }

    fn render(pself: *void) void {
        const self: *Scene = @alignCast(@ptrCast(pself));
        _ = self;
        // Scene rendering here
        std.debug.print("Scene render!\n", .{});
    }
};

const scene_vtable: apricot.scene.Scene_VTable = .{
    .deinit = Scene.deinit,
    .update = Scene.update,
    .render = Scene.render,
};
```

## Features
<!-- TODO 
- Showcase making a system
-->
