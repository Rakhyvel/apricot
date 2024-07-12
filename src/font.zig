const std = @import("std");
const SDL = @import("sdl2");
const gl = @import("zgl");
const glm = @import("zlm");
const graphics = @import("graphics.zig");

const Glyph = struct {
    rect: SDL.RectangleF,
    advance: usize,
};

extern fn TTF_GlyphMetrics32(
    font: *SDL.c.TTF_Font,
    ch: c_uint,
    minx: *c_int,
    maxx: *c_int,
    miny: *c_int,
    maxy: *c_int,
    advance: *c_int,
) c_int;

pub const Font = struct {
    ttf_font: SDL.ttf.Font,
    cache_texture: gl.Texture,
    glyphs: [95]Glyph, // All 95 printable ASCII glyphs
    height: usize,
    style: SDL.ttf.Font.Style,
    alloc: std.mem.Allocator,

    fn init(filename: [:0]const u8, point_size: i32, style: SDL.ttf.Font.Style, alloc: std.mem.Allocator) !Font {
        const ttf_font = try SDL.ttf.openFont(filename, @intCast(point_size));
        var retval: Font = .{
            .ttf_font = ttf_font,
            .cache_texture = undefined,
            .glyphs = [_]Glyph{undefined} ** 95,
            .height = @intCast(ttf_font.height()),
            .style = style,
            .alloc = alloc,
        };
        try retval.pack_glyphs();
        return retval;
    }

    pub fn deinit() void {}

    pub inline fn draw(
        self: *const Font,
        x: f32,
        y: f32,
        text: []const u8,
        program: gl.Program,
    ) !void {
        var cursor_x = x;
        for (text) |c| {
            if (c < 32 or c > 127) {
                continue;
            }
            const glyph = self.get_glyph(c);
            var glyph_vertices = graphics.quad_vertices;
            const model = glm.Mat4.createScale(glyph.rect.width, glyph.rect.height, 1.0).mul(
                glm.Mat4.createTranslationXYZ(cursor_x, y, 0.0),
            );
            var glyph_uv = graphics.quad_uv;
            for (0..4) |i| {
                if (glyph_uv[2 * i] == 0) {
                    glyph_uv[2 * i] = glyph.rect.x / 240.0;
                } else {
                    glyph_uv[2 * i] = (glyph.rect.x + glyph.rect.width) / 240.0;
                }
                if (glyph_uv[2 * i + 1] == 0) {
                    glyph_uv[2 * i + 1] = glyph.rect.y / 240.0;
                } else {
                    glyph_uv[2 * i + 1] = (glyph.rect.y + glyph.rect.height) / 240.0;
                }
            }
            for (0..8) |i| {
                glyph_vertices[i + 12] = glyph_uv[i];
            }
            const quad = graphics.Mesh.from_data(&glyph_vertices, &graphics.quad_indices);
            const view = glm.Mat4.identity;
            const proj = glm.Mat4.createOrthogonal(0.0, 640.0, 480.0, 0.0, -0.1, 10.0);
            quad.draw(model, view, proj, program, self.cache_texture);
            cursor_x += @floatFromInt(glyph.advance);
        }
    }

    /// Returns how wide the text will be if drawn
    pub fn width(self: *const Font, text: []const u8) usize {
        var retval: usize = 0;
        for (text) |c| {
            if (c < 32 or c > 127) {
                continue;
            }
            const glyph = self.get_glyph(c);
            retval += @intCast(glyph.advance);
        }
        return retval;
    }

    /// Packs the glyphs into the cache texture. Uses a basic lightning-fast scanline packing algorithm
    fn pack_glyphs(self: *Font) !void {
        var x_offset: usize = 0;
        var y_offset: usize = 0;
        const mega_surface = try SDL.createRgbSurfaceWithFormat(
            @intCast(self.height * 10),
            @intCast(self.height * 10),
            SDL.PixelFormatEnum.argb8888,
        );
        std.debug.print("height: {}\n", .{self.height * 10});
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
            const glyph_rect = SDL.RectangleF{
                .x = @floatFromInt(dest_rect.x),
                .y = @floatFromInt(dest_rect.y),
                .width = @floatFromInt(dest_rect.width),
                .height = @floatFromInt(dest_rect.height),
            };
            _ = SDL.c.SDL_SetSurfaceBlendMode(surf.ptr, SDL.c.SDL_BLENDMODE_NONE);
            try SDL.blitScaled(surf, &src_rect, mega_surface, &dest_rect);
            x_offset += @intCast(surf.ptr.w);

            var minx: c_int = 0;
            var maxx: c_int = 0;
            var miny: c_int = 0;
            var maxy: c_int = 0;
            var advance: c_int = 0;
            _ = TTF_GlyphMetrics32(self.ttf_font.ptr, c, &minx, &maxx, &miny, &maxy, &advance);

            self.set_glyph(c, Glyph{ .rect = glyph_rect, .advance = @intCast(advance) });
        }
        self.cache_texture = graphics.texture_from_surface(mega_surface);
    }

    inline fn set_glyph(self: *Font, char: u8, glyph: Glyph) void {
        self.glyphs[char - 32] = glyph;
    }

    inline fn get_glyph(self: *const Font, char: u8) Glyph {
        return self.glyphs[char - 32];
    }
};

pub const Font_Id: type = u8; // An index into the `fonts` array

pub const Font_Manager_Resource = struct {
    pub const MAX_FONTS: usize = 255;
    fonts: [MAX_FONTS]Font,
    num_fonts: u8 = 0,

    pub fn create(alloc: std.mem.Allocator) !*Font_Manager_Resource {
        const retval = try alloc.create(Font_Manager_Resource);
        retval.* = .{ .fonts = [_]Font{undefined} ** MAX_FONTS, .num_fonts = 0 };
        return retval;
    }

    pub fn add_font(
        self: *Font_Manager_Resource,
        filename: [:0]const u8,
        point_size: i32,
        style: SDL.ttf.Font.Style,
        alloc: std.mem.Allocator,
    ) !Font_Id {
        self.fonts[self.num_fonts] = try Font.init(filename, point_size, style, alloc);
        self.num_fonts += 1;
        return self.num_fonts - 1;
    }

    pub inline fn get_font(self: *const Font_Manager_Resource, id: Font_Id) *const Font {
        std.debug.assert(id < self.num_fonts);
        return &self.fonts[id];
    }
};
