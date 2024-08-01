const std = @import("std");
const SDL = @import("sdl2");

const Glyph = struct {
    rect: SDL.Rectangle,
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
    cache_texture: SDL.Texture,
    glyphs: [95]Glyph, // All 95 printable ASCII glyphs
    renderer: SDL.Renderer, // Maybe don't keep this
    height: usize,
    style: SDL.ttf.Font.Style,
    alloc: std.mem.Allocator,

    fn init(filename: [:0]const u8, point_size: i32, style: SDL.ttf.Font.Style, renderer: SDL.Renderer, alloc: std.mem.Allocator) !Font {
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

    pub inline fn draw(
        self: *const Font,
        renderer: SDL.Renderer,
        x: i32,
        y: i32,
        text: []const u8,
    ) !void {
        var cursor_x = x;
        for (text) |c| {
            if (c < 32 or c > 127) {
                continue;
            }
            const glyph = self.get_glyph(c);
            const dest_rect = SDL.Rectangle{
                .x = @intCast(cursor_x),
                .y = @intCast(y),
                .width = glyph.rect.width,
                .height = glyph.rect.height,
            };
            try renderer.copy(self.cache_texture, dest_rect, glyph.rect);
            cursor_x += @intCast(glyph.advance);
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
                SDL.Color.rgb(0, 0, 0),
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

            var minx: c_int = 0;
            var maxx: c_int = 0;
            var miny: c_int = 0;
            var maxy: c_int = 0;
            var advance: c_int = 0;
            _ = TTF_GlyphMetrics32(self.ttf_font.ptr, c, &minx, &maxx, &miny, &maxy, &advance);

            self.set_glyph(c, Glyph{ .rect = dest_rect, .advance = @intCast(advance) });
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
        renderer: SDL.Renderer,
        alloc: std.mem.Allocator,
    ) !Font_Id {
        self.fonts[self.num_fonts] = try Font.init(filename, point_size, style, renderer, alloc);
        self.num_fonts += 1;
        return self.num_fonts - 1;
    }

    pub inline fn get_font(self: *const Font_Manager_Resource, id: Font_Id) *const Font {
        std.debug.assert(id < self.num_fonts);
        return &self.fonts[id];
    }
};
