const std = @import("std");
const SDL = @import("sdl2");

pub fn load(renderer: SDL.Renderer, filename: [:0]const u8) !SDL.Texture {
    // Create a new `static` texture from img, which cannot be rendered on
    const img_texture = try SDL.image.loadTexture(renderer, filename);
    defer img_texture.destroy();

    // Create a new `target` texture, which can be rendered on
    const info = try img_texture.query();
    const accessible_texture = try SDL.createTexture(
        renderer,
        SDL.PixelFormatEnum.argb8888,
        SDL.Texture.Access.target,
        info.width,
        info.height,
    );
    errdefer accessible_texture.destroy();

    try renderer.setTarget(accessible_texture);
    try accessible_texture.setBlendMode(SDL.BlendMode.blend);
    try renderer.copy(
        img_texture,
        null,
        SDL.Rectangle{ .x = 0, .y = 0, .width = @intCast(info.width), .height = @intCast(info.height) },
    );
    try renderer.setTarget(null);

    return accessible_texture;
}
