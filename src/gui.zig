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

pub const background_color = SDL.Color.rgb(54, 56, 62);
pub const text_color = SDL.Color.rgb(255, 255, 255);
pub const border_color = SDL.Color.rgb(114, 117, 126);
pub const hover_color = SDL.Color.rgb(255, 255, 255);
pub const active_color = SDL.Color.rgb(60, 100, 250);
pub const error_color = SDL.Color.rgb(250, 80, 80);
pub const inactive_background_color = SDL.Color.rgba(77, 82, 88, 64);
pub const inactive_text_color = SDL.Color.rgb(200, 200, 200);

const Gui_Event_Callback = *const fn (world: *World, id: Entity_Id) void;

const Gui_Component = struct {
    pos: Vector,
    size: Vector,
    shown: bool = true,
    // TODO: Move these to a Click_Component
    is_hovered: bool = false,
    clicked_in: bool = false,
};

const Font_Id_Component = struct {
    font_id: font_.Font_Manager_Resource.Font_Id,
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
    try world.register_component(Image_Component);
    try world.register_component(Progress_Bar_Component);
    try world.register_component(Checkbox_Component);
    try world.register_component(Label_Component);
    try world.register_component(Rocker_Switch_Component);
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
    ).entity_id();

    return label_id;
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

pub fn update(world: *World) void {
    update_checkbox(world);
    update_rocker_switch(world);
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
            }
            entity.gui.clicked_in = false;
        }
    }
}

pub fn render(world: *World) !void {
    try render_image(world);
    try render_progress_bar(world);
    try render_checkbox(world);
    try render_label(world);
    try render_rocker_switch(world);
}

fn render_image(world: *World) !void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *const Gui_Component,
        image: *const Image_Component,
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
        try app.renderer.copyEx(
            entity.image.texture,
            dest,
            null,
            entity.image.angle,
            null,
            SDL.RendererFlip.none,
        );
    }
}

fn render_progress_bar(world: *World) !void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *const Gui_Component,
        progress_bar: *const Progress_Bar_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }
        try app.renderer.setColor(border_color);
        var rect = SDL.Rectangle{
            .x = @intFromFloat(entity.gui.pos.x),
            .y = @intFromFloat(entity.gui.pos.y),
            .width = @intFromFloat(entity.gui.size.x),
            .height = 6,
        };
        try app.renderer.fillRect(rect);
        try app.renderer.setColor(active_color);
        rect = SDL.Rectangle{
            .x = @intFromFloat(entity.gui.pos.x),
            .y = @intFromFloat(entity.gui.pos.y),
            .width = @intFromFloat(entity.gui.size.x * entity.progress_bar.value),
            .height = 6,
        };
        try app.renderer.fillRect(rect);
    }
}

fn render_checkbox(world: *World) !void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *const Gui_Component,
        checkbox: *const Checkbox_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }

        // Draw the box
        if (entity.checkbox.value) {
            try app.renderer.setColor(active_color);
        } else if (entity.gui.is_hovered) {
            try app.renderer.setColor(border_color);
        } else {
            try app.renderer.setColor(background_color);
        }
        try app.renderer.fillRect(SDL.Rectangle{
            .x = @intFromFloat(entity.gui.pos.x),
            .y = @intFromFloat(entity.gui.pos.y),
            .width = @intFromFloat(entity.gui.size.x),
            .height = @intFromFloat(entity.gui.size.y),
        });
    }
}

fn render_label(world: *World) !void {
    const app = world.get_resource(App);
    const font_mgr = world.get_resource(font_.Font_Manager_Resource);
    var iter = world.iter(struct {
        gui: *const Gui_Component,
        label: *const Label_Component,
        font: *const Font_Id_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }
        const font = font_mgr.get_font(entity.font.font_id);
        try font.draw(
            app.renderer,
            @intFromFloat(entity.gui.pos.x),
            @intFromFloat(entity.gui.pos.y),
            entity.label.text,
        );
    }
}

fn render_rocker_switch(world: *World) !void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *const Gui_Component,
        rocker_switch: *const Rocker_Switch_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }

        var rect = SDL.Rectangle{
            .x = @intFromFloat(entity.gui.pos.x),
            .y = @intFromFloat(entity.gui.pos.y),
            .width = 44,
            .height = 20,
        };
        var switch_rect = SDL.Rectangle{
            .x = @intFromFloat(entity.gui.pos.x + 4),
            .y = @intFromFloat(entity.gui.pos.y + 4),
            .width = 8,
            .height = 12,
        };
        if (entity.rocker_switch.value) {
            try app.renderer.setColor(active_color);
            try app.renderer.fillRect(rect);
            try app.renderer.setColor(text_color);
            switch_rect.x += 28;
            try app.renderer.fillRect(switch_rect);
        } else {
            try app.renderer.setColor(text_color);
            try app.renderer.fillRect(switch_rect);
        }

        if (entity.gui.is_hovered) {
            try app.renderer.setColor(text_color);
        } else if (entity.rocker_switch.value) {
            try app.renderer.setColor(active_color);
        } else {
            try app.renderer.setColor(border_color);
        }

        try app.renderer.drawRect(rect);
        rect.x += 1;
        rect.y += 1;
        rect.width -= 2;
        rect.height -= 2;
        try app.renderer.drawRect(rect);
    }
}
