const std = @import("std");
const App = @import("app.zig").App;
const Entity_Id = @import("world.zig").Entity_Id;
const SDL = @import("sdl2");
const Vector = @import("vector.zig");
const World = @import("world.zig").World;
const font_ = @import("font.zig");

// x Image
// x Progress bar
// x Check box
// x Label
// x Rocker switch
// - Slider
// - Radio button group
// - Container
// - Button
// - Text box

var string_alloc: std.mem.Allocator = undefined;

pub const white_color = SDL.Color.rgb(255, 255, 255);

pub const background_color = SDL.Color.rgb(153, 153, 153);
pub const text_color = SDL.Color.rgb(255, 255, 255);

pub const border_color = SDL.Color.rgb(122, 122, 122);
pub const hovered_border_color = SDL.Color.rgb(51, 51, 51);

pub const active_color = SDL.Color.rgb(0, 120, 215);
pub const hovered_active_color = SDL.Color.rgb(77, 161, 227);

pub const clicked_color = SDL.Color.rgb(102, 102, 102);

pub const error_color = SDL.Color.rgb(250, 80, 80);
pub const inactive_background_color = SDL.Color.rgba(77, 82, 88, 64);
pub const inactive_text_color = SDL.Color.rgb(200, 200, 200);

const Gui_Event_Callback = *const fn (world: *World, id: Entity_Id) void;

const Gui_Component = struct {
    pos: Vector,
    size: Vector,
    shown: bool = true,
    // TODO: Move these to a Click_Component, and run through the whole gui lot with the python script
    is_hovered: bool = false,
    clicked_in: bool = false,
};

const Font_Id_Component = struct {
    font_id: font_.Font_Manager_Resource.Font_Id,
};

const Animation_Component = struct {
    value: f32,
};

const Linear_Component = struct {
    vel: f32,
};

const Ease_Out_Component = struct {
    t: f32,
    vel: f32,
    inverted: bool,
};

const Image_Component = struct {
    texture: SDL.Texture,
    angle: f32 = 0.0,
};

const Progress_Bar_Component = struct {
    value: f32 = 0.0,
};

const Checkbox_Component = struct {
    value: bool = false,
};

const Label_Component = struct {
    text: []const u8,
};

const Rocker_Switch_Component = struct {
    value: bool = false,
    onchange: Gui_Event_Callback,
};

const Slider_Component = struct {
    onchange: ?Gui_Event_Callback,
    value: f32, // Will be [0, n_nothces - 1] when n_notches != 0, otherwise will be [0.0, 1.0]
    n_notches: usize, // The number of discrete nothces on the slider. If 0, slider will be continuous
};

pub fn init() void {
    string_alloc = std.heap.page_allocator;
}

/// Registers components that are used in GUI systems.
///
/// This function must be called before `render` or `update` are called. Otherwise, you will receive errors that
/// components are not registered.
pub fn register_components(world: *World) !void {
    try world.register_component(Gui_Component);
    try world.register_component(Font_Id_Component);
    try world.register_component(Animation_Component);
    try world.register_component(Linear_Component);
    try world.register_component(Ease_Out_Component);
    try world.register_component(Image_Component);
    try world.register_component(Progress_Bar_Component);
    try world.register_component(Checkbox_Component);
    try world.register_component(Label_Component);
    try world.register_component(Rocker_Switch_Component);
    try world.register_component(Slider_Component);
}

pub fn create_image(world: *World, image: SDL.Texture, position: Vector) !Entity_Id {
    const query = try image.query();
    const label_id = world.create_entity().with(
        Gui_Component{
            .pos = position,
            .size = .{ .x = @floatFromInt(query.width), .y = @floatFromInt(query.height) },
        },
    ).with(
        Image_Component{ .texture = image },
    ).entity_id();

    return label_id;
}

pub fn create_progress_bar(world: *World, value: f32, position: Vector, size: Vector) Entity_Id {
    const label_id = world.create_entity().with(
        Gui_Component{
            .pos = position,
            .size = size,
        },
    ).with(
        Progress_Bar_Component{ .value = value },
    ).entity_id();

    return label_id;
}

pub fn create_checkbox(world: *World, value: bool, position: Vector) Entity_Id {
    const label_id = world.create_entity().with(
        Gui_Component{
            .pos = position,
            .size = .{ .x = 20, .y = 20 },
        },
    ).with(
        Checkbox_Component{ .value = value },
    ).entity_id();

    return label_id;
}

pub fn create_label(world: *World, text: []const u8, position: Vector, font_id: font_.Font_Manager_Resource.Font_Id) !Entity_Id {
    const font_mgr = world.get_resource(font_.Font_Manager_Resource);
    var font = font_mgr.get_font(font_id);
    const width = font.width(text);
    const height = font.height;
    const duped_text = try string_alloc.dupe(u8, text);
    const label_id = world.create_entity().with(
        Gui_Component{
            .pos = position,
            .size = .{ .x = @floatFromInt(width), .y = @floatFromInt(height) },
        },
    ).with(
        Label_Component{ .text = duped_text },
    ).with(
        Font_Id_Component{ .font_id = font_id },
    ).entity_id();

    return label_id;
}

pub fn create_rocker_switch(world: *World, value: bool, onchange: Gui_Event_Callback, pos: Vector) Entity_Id {
    const label_id = world.create_entity().with(
        Gui_Component{
            .pos = pos,
            .size = .{ .x = 44, .y = 20 },
        },
    ).with(
        Rocker_Switch_Component{ .value = value, .onchange = onchange },
    ).with(
        Animation_Component{ .value = if (value) 1.0 else 0.0 },
    ).entity_id();

    return label_id;
}

pub fn create_slider(world: *World, value: f32, pos: Vector, width: usize, n_notches: usize, onchange: ?Gui_Event_Callback) Entity_Id {
    const label_id = world.create_entity().with(
        Gui_Component{
            .pos = pos,
            .size = .{ .x = @floatFromInt(width), .y = 52 },
        },
    ).with(
        Slider_Component{
            .value = value,
            .onchange = onchange,
            .n_notches = n_notches,
        },
    ).entity_id();

    return label_id;
}

fn apply_linear(world: *World, entity_id: Entity_Id, anim_speed: f32) void {
    world.assign_component(Linear_Component, entity_id, Linear_Component{ .vel = anim_speed });
}

fn apply_ease_out(world: *World, entity_id: Entity_Id, anim_speed: f32, inverted: bool) void {
    world.assign_component(Ease_Out_Component, entity_id, Ease_Out_Component{
        .t = 0.0,
        .vel = anim_speed,
        .inverted = inverted,
    });
}

pub fn get_pos(world: *World, entity_id: Entity_Id) Vector {
    const gui = world.get_component(Gui_Component, entity_id);
    return gui.pos;
}

pub fn set_pos(world: *World, entity_id: Entity_Id, position: Vector) void {
    const gui = world.get_component(Gui_Component, entity_id);
    gui.pos = position;
    // TODO: Trigger re-flow
}

pub fn get_dimensions(world: *World, entity_id: Entity_Id) Vector {
    const gui = world.get_component(Gui_Component, entity_id);
    return gui.size;
}

pub fn set_dimensions(world: *World, entity_id: Entity_Id, size: Vector) void {
    const gui = world.get_component(Gui_Component, entity_id);
    gui.pos = size;
    // TODO: Trigger re-flow
}

pub fn is_shown(world: *World, entity_id: Entity_Id) bool {
    const gui = world.get_component(Gui_Component, entity_id);
    return gui.shown;
}

pub fn set_shown(world: *World, entity_id: Entity_Id, shown: bool) void {
    const gui = world.get_component(Gui_Component, entity_id);
    gui.shown = shown;
    // TODO: Trigger re-flow
}

pub fn set_font_id(world: *World, font_entity_id: Entity_Id, font_id: font_.Font_Manager_Resource.Font_Id) void {
    const font = world.get_component(Font_Id_Component, font_entity_id);
    font.font_id = font_id;
}

pub fn get_image_angle(world: *World, image_id: Entity_Id) f32 {
    const image = world.get_component(Image_Component, image_id);
    return image.angle;
}

pub fn set_image_angle(world: *World, image_id: Entity_Id, angle: f32) void {
    const image = world.get_component(Image_Component, image_id);
    image.angle = angle;
}

pub fn get_progress_bar_value(world: *World, progress_bar_id: Entity_Id) f32 {
    const progress_bar = world.get_component(Progress_Bar_Component, progress_bar_id);
    return progress_bar.value;
}

pub fn set_progress_bar_value(world: *World, progress_bar_id: Entity_Id, value: f32) void {
    const progress_bar = world.get_component(Progress_Bar_Component, progress_bar_id);
    progress_bar.value = value;
}

pub fn get_checkbox_value(world: *World, checkbox_id: Entity_Id) bool {
    const progress_bar = world.get_component(Checkbox_Component, checkbox_id);
    return progress_bar.value;
}

pub fn set_checkbox_value(world: *World, checkbox_id: Entity_Id, value: bool) void {
    const progress_bar = world.get_component(Checkbox_Component, checkbox_id);
    progress_bar.value = value;
}

pub fn set_label_text(
    world: *World,
    label_id: Entity_Id,
    comptime fmt: []const u8,
    args: anytype,
) !void {
    var label = world.get_component(Label_Component, label_id);
    string_alloc.free(label.text); // TODO: I don't like this...
    label.text = try std.fmt.allocPrint(string_alloc, fmt, args);
}

pub fn get_rocker_switch_value(world: *World, rocker_switch_id: Entity_Id) bool {
    const rocker_switch = world.get_component(Rocker_Switch_Component, rocker_switch_id);
    return rocker_switch.value;
}

pub fn set_rocker_switch_value(world: *World, rocker_switch_id: Entity_Id, value: bool) void {
    const rocker_switch = world.get_component(Rocker_Switch_Component, rocker_switch_id);
    rocker_switch.value = value;
}

pub fn get_slider_value(world: *World, slider_id: Entity_Id) f32 {
    const slider = world.get_component(Slider_Component, slider_id);
    return slider.value;
}

pub fn set_slider_value(world: *World, slider_id: Entity_Id, value: f32) void {
    const slider = world.get_component(Slider_Component, slider_id);
    slider.value = value;
}

pub fn update(world: *World) void {
    update_linear(world);
    update_ease_out(world);
    update_checkbox(world);
    update_rocker_switch(world);
    update_slider(world);
}

fn update_linear(world: *World) void {
    var iter = world.iter(struct { id: Entity_Id, animation: *Animation_Component, linear: *Linear_Component });
    while (iter.next()) |entity| {
        entity.animation.value += entity.linear.vel;
        if (entity.animation.value < 0.0) {
            entity.animation.value = 0.0;
            world.unassign_component(Linear_Component, entity.id);
        }
        if (entity.animation.value > 1.0) {
            entity.animation.value = 1.0;
            world.unassign_component(Linear_Component, entity.id);
        }
    }
}

fn update_ease_out(world: *World) void {
    var iter = world.iter(struct { id: Entity_Id, animation: *Animation_Component, ease_out: *Ease_Out_Component });
    while (iter.next()) |entity| {
        entity.ease_out.t += entity.ease_out.vel;
        entity.animation.value = 1 - std.math.pow(
            f32,
            entity.ease_out.t - @as(f32, @floatFromInt(@intFromBool(entity.ease_out.inverted))),
            2,
        ) + 0.01 * @as(f32, if (entity.ease_out.inverted) 1.0 else -1.0);
        if (entity.animation.value < 0.0) {
            entity.animation.value = 0.0;
            world.unassign_component(Ease_Out_Component, entity.id);
        }
        if (entity.animation.value > 1.0) {
            entity.animation.value = 1.0;
            world.unassign_component(Ease_Out_Component, entity.id);
        }
    }
}

fn update_checkbox(world: *World) void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *Gui_Component,
        checkbox: *Checkbox_Component,
    });
    while (iter.next()) |entity| {
        entity.gui.is_hovered = entity.gui.shown and
            app.mouse_x > @as(i32, @intFromFloat(entity.gui.pos.x)) and
            app.mouse_x <= @as(i32, @intFromFloat(entity.gui.pos.x + entity.gui.size.x)) and
            app.mouse_y > @as(i32, @intFromFloat(entity.gui.pos.y)) and
            app.mouse_y <= @as(i32, @intFromFloat(entity.gui.pos.y + entity.gui.size.y));
        if (entity.gui.is_hovered and app.mouse_left_down) {
            entity.gui.clicked_in = true;
        }
        if (!app.mouse_left_down) {
            if (entity.gui.clicked_in and entity.gui.is_hovered) {
                entity.checkbox.value = !entity.checkbox.value;
            }
            entity.gui.clicked_in = false;
        }
    }
}

fn update_rocker_switch(world: *World) void {
    const app = world.get_resource(App);
    const anim_speed: f32 = 0.08;
    var iter = world.iter(struct {
        id: Entity_Id,
        gui: *Gui_Component,
        rocker_switch: *Rocker_Switch_Component,
    });
    while (iter.next()) |entity| {
        entity.gui.is_hovered = entity.gui.shown and
            app.mouse_x > @as(i32, @intFromFloat(entity.gui.pos.x)) and
            app.mouse_x <= @as(i32, @intFromFloat(entity.gui.pos.x + entity.gui.size.x)) and
            app.mouse_y > @as(i32, @intFromFloat(entity.gui.pos.y)) and
            app.mouse_y <= @as(i32, @intFromFloat(entity.gui.pos.y + entity.gui.size.y));
        if (entity.gui.is_hovered and app.mouse_left_down) {
            entity.gui.clicked_in = true;
        }
        if (!app.mouse_left_down) {
            if (entity.gui.clicked_in and entity.gui.is_hovered) {
                entity.rocker_switch.value = !entity.rocker_switch.value;
                entity.rocker_switch.onchange(world, entity.id);
                apply_ease_out(world, entity.id, anim_speed, entity.rocker_switch.value);
            }
            entity.gui.clicked_in = false;
        }
    }
}

fn update_slider(world: *World) void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        id: Entity_Id,
        gui: *Gui_Component,
        slider: *Slider_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }
        entity.gui.is_hovered = entity.gui.shown and
            app.mouse_x > @as(i32, @intFromFloat(entity.gui.pos.x)) and
            app.mouse_x <= @as(i32, @intFromFloat(entity.gui.pos.x + entity.gui.size.x)) and
            app.mouse_y > @as(i32, @intFromFloat(entity.gui.pos.y)) and
            app.mouse_y <= @as(i32, @intFromFloat(entity.gui.pos.y + entity.gui.size.y));

        if ((entity.gui.is_hovered or entity.gui.clicked_in) and app.mouse_left_down) {
            const val = std.math.clamp(
                (@as(f32, @floatFromInt(app.mouse_x)) - entity.gui.pos.x) / entity.gui.size.x,
                0.0,
                1.0,
            );
            if (entity.slider.n_notches == 0) { // Notchless behavior
                entity.slider.value = @max(0.0, @min(1.0, val));
            } else {
                const float_n_notches: f32 = @floatFromInt(entity.slider.n_notches - 1);
                entity.slider.value = @round(val * float_n_notches);
            }
            entity.gui.clicked_in = true;
            if (entity.slider.onchange) |onchange| {
                onchange(world, entity.id);
            }
        }

        if (entity.gui.clicked_in and !app.mouse_left_down) {
            entity.gui.clicked_in = false;
        }
    }
}

pub fn render(world: *World) !void {
    _ = world; // autofix
}
