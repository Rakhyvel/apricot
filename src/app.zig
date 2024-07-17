const std = @import("std");
const SDL = @import("sdl2");
const Scene_Object = @import("scene.zig").Scene_Object;

pub const App = struct {
    // TODO: Actually split these up into resource structs
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

    // Mouse stuff
    mouse_x: i32,
    mouse_y: i32,
    mouse_rel_x: i32,
    mouse_rel_y: i32,
    mouse_left_down: bool,
    mouse_right_down: bool,
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
            // Mouse stuff
            .mouse_x = 0,
            .mouse_y = 0,
            .mouse_rel_x = 0,
            .mouse_rel_y = 0,
            .mouse_left_down = false,
            .mouse_right_down = false,
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

            if (self.scene_stack.items.len > 0) {
                var top = self.scene_stack.getLast();
                // update
                while (lag >= DELTA_T) {
                    self.reset_input();
                    self.poll_input();
                    top.vtable.update(top.self);
                    self.ticks += 1;
                    lag -= DELTA_T;
                }

                // render
                try self.renderer.setColorRGB(0x33, 0x4C, 0x65);
                try self.renderer.clear();
                top.vtable.render(top.self);
                frames += 1;
                self.renderer.present();
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
        self.mouse_rel_x = 0;
        self.mouse_rel_y = 0;
        self.mouse_wheel = 0.0;
    }

    fn poll_input(self: *@This()) void {
        while (SDL.pollEvent()) |ev| {
            switch (ev) {
                .quit => self.running = false,
                .mouse_motion => {
                    self.mouse_x = ev.mouse_motion.x;
                    self.mouse_y = ev.mouse_motion.y;
                    self.mouse_rel_x = ev.mouse_motion.delta_x;
                    self.mouse_rel_y = ev.mouse_motion.delta_y;
                },
                .mouse_button_down => switch (ev.mouse_button_down.button) {
                    .left => self.mouse_left_down = true,
                    .right => self.mouse_right_down = true,
                    else => {}, // TODO: These might be cool to add!
                },
                .mouse_button_up => switch (ev.mouse_button_up.button) {
                    .left => self.mouse_left_down = false,
                    .right => self.mouse_right_down = false,
                    else => {}, // TODO: These might be cool to add!
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
