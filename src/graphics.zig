const std = @import("std");
const SDL = @import("sdl2");
const gl = @import("zgl");

pub fn texture_from_surface(surf: SDL.Surface) gl.Texture {
    const retval = gl.genTexture();
    gl.bindTexture(retval, .@"2d");

    const width = surf.ptr.w;
    const height = surf.ptr.h;
    gl.textureImage2D(
        .@"2d",
        0,
        .rgba,
        @intCast(width),
        @intCast(height),
        gl.PixelFormat.bgra,
        .unsigned_byte,
        @ptrCast(surf.ptr.pixels.?),
    );
    retval.parameter(.wrap_s, .clamp_to_edge);
    retval.parameter(.wrap_t, .clamp_to_edge);
    retval.parameter(.min_filter, .nearest);
    retval.parameter(.mag_filter, .nearest);
    gl.bindTexture(.invalid, .@"2d");
    return retval;
}
