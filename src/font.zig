const std = @import("std");
const SDL = @import("sdl2");

const Glyph = struct {
    rect: SDL.Rectangle,
};

pub const Font = struct {
    ttf_font: SDL.ttf.Font,
    cache_texture: SDL.Texture,
    glyphs: [95]Glyph, // All 95 printable ASCII glyphs
    renderer: SDL.Renderer, // Maybe don't keep this
    height: usize,
    style: SDL.ttf.Font.Style,
    alloc: std.mem.Allocator,

    pub fn init(filename: [:0]const u8, point_size: i32, style: SDL.ttf.Font.Style, renderer: SDL.Renderer, alloc: std.mem.Allocator) !Font {
        const ttf_font = try SDL.ttf.openFont(filename, @intCast(point_size));
        var retval: Font = .{
            .ttf_font = ttf_font,
            .cache_texture = undefined,
            .glyphs = [_]Glyph{undefined} ** 95,
            .renderer = renderer,
            .height = @intCast(ttf_font.height()),
            .style = style,
            .alloc = alloc,
        };
        try retval.pack_glyphs();
        return retval;
    }

    pub fn deinit() void {}

    pub fn draw(
        self: *Font,
        renderer: SDL.Renderer,
        x: usize,
        y: usize,
        text: []const u8,
    ) !void {
        var cursor_x = x;
        for (text) |c| {
            const glyph = self.get_glyph(c);
            const dest_rect = SDL.Rectangle{
                .x = @intCast(cursor_x),
                .y = @intCast(y),
                .width = glyph.rect.width,
                .height = glyph.rect.height,
            };
            try renderer.copy(self.cache_texture, dest_rect, glyph.rect);
            cursor_x += @intCast(glyph.rect.width);
        }
    }

    /// Packs the glyphs into the cache texture. Uses a basic lightning-fast scanline packing algorithm
    fn pack_glyphs(self: *Font) !void {
        var x_offset: usize = 0;
        var y_offset: usize = 0;
        const mega_surface = try SDL.createRgbSurfaceWithFormat(
            @intCast(self.height * 10),
            @intCast(self.height * 10),
            SDL.PixelFormatEnum.rgba8888,
        );
        defer mega_surface.destroy();
        for (32..127) |i| {
            const c: u8 = @intCast(i);
            const byte_array: []const u8 = &[_]u8{c};
            const byte_slice = try self.alloc.dupeZ(u8, byte_array);
            defer self.alloc.free(byte_slice);

            self.ttf_font.setStyle(self.style);

            const surf = try self.ttf_font.renderTextBlended(
                byte_slice,
                SDL.Color.rgb(255, 255, 255),
            );
            defer surf.destroy();

            if (x_offset + @as(usize, @intCast(surf.ptr.w)) > @as(usize, @intCast(self.height * 10))) {
                x_offset = 0;
                y_offset += self.height;
            }

            var src_rect = SDL.Rectangle{
                .x = 0,
                .y = 0,
                .width = surf.ptr.w,
                .height = surf.ptr.h,
            };
            var dest_rect = SDL.Rectangle{
                .x = @intCast(x_offset),
                .y = @intCast(y_offset),
                .width = surf.ptr.w,
                .height = surf.ptr.h,
            };
            _ = SDL.c.SDL_SetSurfaceBlendMode(surf.ptr, SDL.c.SDL_BLENDMODE_NONE);
            try SDL.blitScaled(surf, &src_rect, mega_surface, &dest_rect);
            x_offset += @intCast(surf.ptr.w);
            self.set_glyph(c, Glyph{ .rect = dest_rect });
        }
        self.cache_texture = try SDL.createTextureFromSurface(self.renderer, mega_surface);
    }

    inline fn set_glyph(self: *Font, char: u8, glyph: Glyph) void {
        self.glyphs[char - 32] = glyph;
    }

    inline fn get_glyph(self: *const Font, char: u8) Glyph {
        return self.glyphs[char - 32];
    }
};
