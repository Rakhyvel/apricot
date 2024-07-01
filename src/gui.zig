const App = @import("app.zig").App;
const Entity_Id = @import("world.zig").Entity_Id;
const SDL = @import("sdl2");
const Vector = @import("vector.zig");
const World = @import("world.zig").World;

// - Image
// - Progress bar
// - Check box
// - Button
// - Rocker switch
// - Radio button group
// - Slider
// - Label
// - Text box
// - Container

pub const Gui_Component = struct {
    pos: Vector,
    size: Vector,
    shown: bool,
};

pub const Image_Component = struct {
    texture: SDL.Texture,
    angle: f32,
};

/// Registers components that are used in GUI systems.
///
/// This function must be called before `render` or `update` are called. Otherwise, you will receive errors that
/// components are not registered.
pub fn register_components(world: *World) void {
    world.register_component(Gui_Component);
    world.register_component(Image_Component);
}

pub fn render(world: *World) void {
    render_image(world);
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
            .height = @intFromFloat(entity.gui.size.x),
            .width = @intFromFloat(entity.gui.size.y),
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
