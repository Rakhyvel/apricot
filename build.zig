const std = @import("std");
const sdl = @import("third-party/SDL.zig/build.zig");

const Apricot = @This();

sdl2_sdk: *sdl,
sdl2_module: *std.Build.Module,

pub const SDL2_Subsys_Lib_Paths = struct {
    image: ?[]const u8,
    ttf: ?[]const u8,
};

pub fn init(b: *std.Build) Apricot {
    const sdk = sdl.init(b, null, null);
    const sdl2_module = sdk.getWrapperModule();
    return .{ .sdl2_sdk = sdk, .sdl2_module = sdl2_module };
}

pub fn link(sdk: *Apricot, exe: *std.Build.Step.Compile, subsys_lib_paths: SDL2_Subsys_Lib_Paths) void {
    sdk.sdl2_sdk.link(exe, .dynamic, .SDL2);
    if (subsys_lib_paths.image) |image_path| {
        exe.addLibraryPath(.{ .cwd_relative = image_path });
        exe.linkSystemLibrary2("SDL2_image", .{ .use_pkg_config = .no });
    }
    if (subsys_lib_paths.ttf) |ttf_path| {
        exe.addLibraryPath(.{ .cwd_relative = ttf_path });
        exe.linkSystemLibrary2("SDL2_ttf", .{ .use_pkg_config = .no });
    }
}

pub fn get_module(apricot: *Apricot, b: *std.Build) *std.Build.Module {
    var retval = b.createModule(.{
        .root_source_file = .{ .cwd_relative = lib_path("/src/root.zig") },
    });
    retval.addImport("sdl2", apricot.sdl2_module);
    return retval;
}

pub fn sdl2(apricot: *Apricot) *std.Build.Module {
    return apricot.sdl2_module;
}

fn lib_path(comptime suffix: []const u8) []const u8 {
    if (suffix[0] != '/') @compileError("relToPath requires an absolute path!");
    return comptime blk: {
        const root_dir = std.fs.path.dirname(@src().file) orelse ".";
        break :blk root_dir ++ suffix;
    };
}
