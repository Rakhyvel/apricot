const std = @import("std");
const SDL = @import("sdl2");
const Scene_Object = @import("scene.zig").Scene_Object;
const Vec2 = @import("vector.zig");

pub const App = struct {
    // I don't feel a real reason to split these up, besides to group them
    // Window stuff
    title: [:0]const u8,
    width: usize,
    height: usize,

    // Gameloop stuff
    running: bool,
    seconds: i64,
    ticks: u64,

    // Scene stack stuff
    scene_stack: std.ArrayList(Scene_Object),
    scene_stale: bool,

    // Mouse stuff TODO: Maybe use Vec2
    mouse: Vec2,
    mouse_rel: Vec2,
    mouse_left_down: bool,
    mouse_middle_down: bool,
    mouse_right_down: bool,
    mouse_extra_1_down: bool,
    mouse_extra_2_down: bool,
    mouse_wheel: i32,

    // Keyboard stuff
    keys: [256]bool,

    // SDL stuff
    window: SDL.Window,
    renderer: SDL.Renderer,

    pub fn init(title: [:0]const u8, width: usize, height: usize, alloc: std.mem.Allocator) !@This() {
        try SDL.init(.{
            .video = true,
            .events = true,
            .audio = true,
        });
        errdefer SDL.quit();

        try SDL.ttf.init();
        errdefer SDL.ttf.quit();

        var window = try SDL.createWindow(
            title,
            .{ .centered = {} },
            .{ .centered = {} },
            width,
            height,
            .{ .vis = .shown },
        );
        errdefer window.destroy();

        const renderer = try SDL.createRenderer(window, null, .{ .accelerated = true });
        errdefer renderer.destroy();

        try renderer.setDrawBlendMode(SDL.BlendMode.blend);

        return .{
            // Window stuff
            .title = title,
            .width = width,
            .height = height,
            // Gameloop stuff
            .running = false,
            .seconds = 0,
            .ticks = 0,
            // Scene stack stuff
            .scene_stack = std.ArrayList(Scene_Object).init(alloc),
            .scene_stale = false,
            // Mouse stuff
            .mouse = Vec2.zero(),
            .mouse_rel = Vec2.zero(),
            .mouse_left_down = false,
            .mouse_middle_down = false,
            .mouse_right_down = false,
            .mouse_extra_1_down = false,
            .mouse_extra_2_down = false,
            .mouse_wheel = 0,
            // Keyboard stuff
            .keys = [_]bool{false} ** 256,
            // SDL stuff
            .renderer = renderer,
            .window = window,
        };
    }

    pub fn deinit(self: *App) void {
        self.renderer.destroy();
        self.window.destroy();
        SDL.ttf.quit();
        SDL.quit();
    }

    pub fn push_scene(self: *App, scene: Scene_Object) void {
        self.scene_stack.append(scene) catch @panic("memory error");
        self.scene_stale = true;
    }

    pub fn start_app(self: *@This()) !void {
        self.running = true;

        var prev_seconds: i64 = std.time.timestamp();
        var current: i64 = undefined;
        var previous: i64 = std.time.milliTimestamp();
        var lag: i64 = 0;
        var elapsed: i64 = 0;
        var frames: i64 = 0;
        const DELTA_T: i64 = 16;

        while (self.running) {
            self.seconds = std.time.timestamp();
            current = std.time.milliTimestamp();
            elapsed = current - previous;

            previous = current;
            lag += elapsed;
            self.scene_stale = false;

            if (self.scene_stack.items.len > 0) {
                var top = self.scene_stack.getLast();

                // update
                while (lag >= DELTA_T) {
                    self.reset_input();
                    self.poll_input();
                    if (!self.scene_stale) {
                        top.vtable.update(top.self);
                        self.ticks += 1;
                        lag -= DELTA_T;
                    } else {
                        break;
                    }
                }

                // render
                if (!self.scene_stale) {
                    try self.renderer.setColorRGB(0xFF, 0xFF, 0xFF);
                    try self.renderer.clear();
                    top.vtable.render(top.self);
                    frames += 1;
                    self.renderer.present();
                }
            }

            const now_seconds = std.time.timestamp();
            if (now_seconds - prev_seconds >= 5) {
                std.debug.print("5 seconds; fps: {}\n", .{@divTrunc(frames, 5)});
                prev_seconds = std.time.timestamp();
                frames = 0;
            }
        }
    }

    fn reset_input(self: *@This()) void {
        self.mouse_rel = Vec2.zero();
        self.mouse_wheel = 0.0;
    }

    fn poll_input(self: *@This()) void {
        while (SDL.pollEvent()) |ev| {
            switch (ev) {
                .quit => self.running = false,
                .mouse_motion => {
                    self.mouse = Vec2.new(
                        @floatFromInt(ev.mouse_motion.x),
                        @floatFromInt(ev.mouse_motion.y),
                    );
                    self.mouse_rel = Vec2.new(
                        @floatFromInt(ev.mouse_motion.delta_x),
                        @floatFromInt(ev.mouse_motion.delta_y),
                    );
                },
                .mouse_button_down => switch (ev.mouse_button_down.button) {
                    .left => self.mouse_left_down = true,
                    .middle => self.mouse_middle_down = true,
                    .right => self.mouse_right_down = true,
                    .extra_1 => self.mouse_extra_1_down = true,
                    .extra_2 => self.mouse_extra_2_down = true,
                },
                .mouse_button_up => switch (ev.mouse_button_up.button) {
                    .left => self.mouse_left_down = false,
                    .middle => self.mouse_middle_down = false,
                    .right => self.mouse_right_down = false,
                    .extra_1 => self.mouse_extra_1_down = false,
                    .extra_2 => self.mouse_extra_2_down = false,
                },
                .mouse_wheel => self.mouse_wheel = ev.mouse_wheel.delta_y,
                .window => if (ev.window.type == .resized) {
                    self.width = @intCast(ev.window.type.resized.width);
                    self.height = @intCast(ev.window.type.resized.height);
                },
                .key_down => {
                    self.keys[@intFromEnum(ev.key_down.scancode)] = true;
                    if (self.keys[@intFromEnum(SDL.Scancode.escape)]) {
                        self.running = false;
                    }
                },
                .key_up => self.keys[@intFromEnum(ev.key_up.scancode)] = false,

                else => {},
            }
        }
    }
};
