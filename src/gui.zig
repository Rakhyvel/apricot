const App = @import("app.zig").App;
const Entity_Id = @import("world.zig").Entity_Id;
const SDL = @import("sdl2");
const Vector = @import("vector.zig");
const World = @import("world.zig").World;

// x Image
// x Progress bar
// - Check box
// - Button
// - Rocker switch
// - Radio button group
// - Slider
// - Label
// - Text box
// - Container

pub const background_color = SDL.Color.rgb(54, 56, 62);
pub const text_color = SDL.Color.rgb(255, 255, 255);
pub const border_color = SDL.Color.rgb(114, 117, 126);
pub const hover_color = SDL.Color.rgb(255, 255, 255);
pub const active_color = SDL.Color.rgb(60, 100, 250);
pub const error_color = SDL.Color.rgb(250, 80, 80);
pub const inactive_background_color = SDL.Color.rgba(77, 82, 88, 64);
pub const inactive_text_color = SDL.Color.rgb(200, 200, 200);

pub const Gui_Component = struct {
    pos: Vector,
    size: Vector,
    shown: bool = true,
    is_hovered: bool = false,
    clicked_in: bool = false,
    active: bool = true,
};

pub const Image_Component = struct {
    texture: SDL.Texture,
    angle: f32 = 0.0,
};

pub const Progress_Bar_Component = struct {
    value: f32 = 0.0,
};

pub const Check_Box_Component = struct {
    value: bool = false,
};

/// Registers components that are used in GUI systems.
///
/// This function must be called before `render` or `update` are called. Otherwise, you will receive errors that
/// components are not registered.
pub fn register_components(world: *World) void {
    world.register_component(Gui_Component);
    world.register_component(Image_Component);
    world.register_component(Progress_Bar_Component);
    world.register_component(Check_Box_Component);
}

pub fn update(world: *World) void {
    update_check_box(world);
}

pub fn update_check_box(world: *World) void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *Gui_Component,
        check_box: *Check_Box_Component,
    });
    while (iter.next()) |entity| {
        const prev_is_hovered = entity.gui.is_hovered;
        entity.gui.is_hovered = entity.gui.shown and
            app.mouse_x > @as(i32, @intFromFloat(entity.gui.pos.x)) and
            app.mouse_x <= @as(i32, @intFromFloat(entity.gui.pos.x + entity.gui.size.x)) and
            app.mouse_y > @as(i32, @intFromFloat(entity.gui.pos.y)) and
            app.mouse_y <= @as(i32, @intFromFloat(entity.gui.pos.y + entity.gui.size.y));
        if (!prev_is_hovered and entity.gui.is_hovered and entity.gui.active) {
            // Play a sound?
            // Idk, you don't typically hear a sound when you hover over a checkbox in a form
            // Maybe games and forms are different?
        }
        if (entity.gui.is_hovered and app.mouse_left_down) {
            entity.gui.clicked_in = true;
        }
        if (!app.mouse_left_down) {
            if (entity.gui.clicked_in and entity.gui.is_hovered) {
                entity.check_box.value = !entity.check_box.value;
                if (entity.gui.active) {
                    // Play a sound?
                }
            }
            entity.gui.clicked_in = false;
        }
    }
}

pub fn render(world: *World) void {
    render_image(world);
    render_progress_bar(world);
}

fn render_image(world: *World) void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *Gui_Component,
        image: *Image_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }
        const dest = SDL.Rectangle{
            .x = @intFromFloat(entity.gui.pos.x),
            .y = @intFromFloat(entity.gui.pos.y),
            .width = @intFromFloat(entity.gui.size.x),
            .height = @intFromFloat(entity.gui.size.y),
        };
        app.renderer.copyEx(
            entity.image.texture,
            dest,
            null,
            entity.image.angle,
            null,
            SDL.RendererFlip.none,
        ) catch @panic("error!");
    }
}

fn render_progress_bar(world: *World) void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *Gui_Component,
        progress_bar: *Progress_Bar_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }
        app.renderer.setColorRGB(border_color.r, border_color.g, border_color.b) catch @panic("whoa!");
        var rect = SDL.Rectangle{
            .x = @intFromFloat(entity.gui.pos.x),
            .y = @intFromFloat(entity.gui.pos.y),
            .width = @intFromFloat(entity.gui.size.x),
            .height = 6,
        };
        app.renderer.fillRect(rect) catch @panic("whoa!");
        app.renderer.setColorRGB(active_color.r, active_color.g, active_color.b) catch @panic("whoa!");
        rect = SDL.Rectangle{
            .x = @intFromFloat(entity.gui.pos.x),
            .y = @intFromFloat(entity.gui.pos.y),
            .width = @intFromFloat(entity.gui.size.x * entity.progress_bar.value),
            .height = 6,
        };
        app.renderer.fillRect(rect) catch @panic("whoa!");
    }
}
