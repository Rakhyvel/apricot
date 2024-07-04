const SDL = @import("sdl2");

const Glyph = struct {
    rect: SDL.Rectangle,
    cache_layer: usize,
};

pub const Font = struct {
    ttf_font: SDL.ttf.Font,
    // arraylist of textures
    // map of codepoint to glyph

    pub fn init() !void {
        _ = try SDL.ttf.openFont("res/HelveticaNeue Medium.ttf", 24);
    }

    pub fn deinit() void {}

    pub fn draw() void {}
};
