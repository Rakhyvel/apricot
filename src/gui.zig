const std = @import("std");
const App = @import("app.zig").App;
const Entity_Id = @import("world.zig").Entity_Id;
const SDL = @import("sdl2");
const Vector = @import("vector.zig").Vec2;
const Vec4 = @import("vector.zig").Vec4;
const World = @import("world.zig").World;
const font_ = @import("font.zig");

// TODO: Add these
// x Image
// x Progress bar
// x Check box
// x Label
// x Rocker switch
// x Slider
// x Container
// x Button
// x Spacer
// - Horizontal/vertical rule
// - Text box
// - Radio button group (requires circles)

var gui_alloc: std.mem.Allocator = undefined;

// TODO: Inherit Dear IMGUI color pallettes

// text
// text-disabled
// window background
// border
// checkmark
// slider grab
// slider grab-active
// button
// button hovered
// button active
pub const Color_Palette = struct {
    white: SDL.Color = cfv(Vec4.new(1.0, 1.0, 1.0, 1.0)),
    dark: SDL.Color = cfv(Vec4.new(0.0, 0.0, 0.0, 0.2)),
    darker: SDL.Color = cfv(Vec4.new(0.0, 0.0, 0.0, 0.5)),
    background: SDL.Color = cfv(Vec4.new(0.95, 0.95, 0.95, 1.0)),
    text: SDL.Color = cfv(Vec4.new(0.1, 0.1, 0.1, 1.0)),
    border: SDL.Color = cfv(Vec4.new(0.6, 0.6, 0.6, 1.0)),
    grab: SDL.Color = cfv(Vec4.new(0.69, 0.69, 0.69, 1.0)),
    solid: SDL.Color = cfv(Vec4.new(0.86, 0.86, 0.86, 1.0)),
    active: SDL.Color = cfv(Vec4.new(0.0, 0.47, 0.84, 1.0)),
    hover: SDL.Color = cfv(Vec4.new(0.00, 0.47, 0.84, 0.2)),

    fn cfv(vec4: Vec4) SDL.Color {
        return SDL.Color.rgba(
            @intFromFloat(vec4.x * 255.0),
            @intFromFloat(vec4.y * 255.0),
            @intFromFloat(vec4.z * 255.0),
            @intFromFloat(vec4.w * 255.0),
        );
    }
};
const palette = Color_Palette{};

const Gui_Event_Callback = *const fn (world: *World, id: Entity_Id) void;

pub const Gui_Component = struct {
    pos: Vector = .{ .x = 0, .y = 0 },
    size: Vector,
    shown: bool = true,
    // TODO: Move these to a Click_Component, and run through the whole gui lot with the python script
    is_hovered: bool = false,
    clicked_in: bool = false,
    parent: Entity_Id = Entity_Id.INVALID_ENTITY_ID,
    // TODO: Move to layout component?
    margin: Vector = .{ .x = 0, .y = 0 },
    border: f32 = 0,
    padding: Vector = .{ .x = 0, .y = 1 },
};

pub const Font_Id_Component = struct {
    font_id: font_.Font_Id,
};

pub const Animation_Component = struct {
    value: f32,
};

pub const Linear_Component = struct {
    vel: f32,
};

pub const Ease_Out_Component = struct {
    t: f32,
    vel: f32,
    inverted: bool,
};

pub const Image_Component = struct {
    texture: SDL.Texture,
    angle: f32 = 0.0,
};

pub const Progress_Bar_Component = struct {
    value: f32 = 0.0,
};

pub const Checkbox_Component = struct {
    value: bool = false,
};

pub const Label_Component = struct {
    text: []const u8,
};

pub const Rocker_Switch_Component = struct {
    value: bool = false,
    onchange: Gui_Event_Callback,
};

pub const Slider_Component = struct {
    onchange: ?Gui_Event_Callback,
    value: f32, // Will be [0, n_nothces - 1] when n_notches != 0, otherwise will be [0.0, 1.0]
    n_notches: usize, // The number of discrete nothces on the slider. If 0, slider will be continuous
};

// TODO: Have separate enums for:
// - flow  (down, left, up, right, stack)
// - align (center, down, left, up, right)
// - sizedness (fullscreen, fixed size, min size)
// - A way to say "hey if you're fixed size, just divide up your size horizontally/vertically"
// There's gotta be a better way...
pub const Container_Flow_Align = enum(u8) {
    DOWN_LEFT,
    DOWN_CENTER,
    DOWN_CENTER_EVEN,
    LEFT_TOP,
    LEFT_TOP_EVEN,
    // TODO: Add more
};

pub const Container_Component = struct {
    children: std.ArrayList(Entity_Id),
    flow_align: Container_Flow_Align,
    fixed_width: bool,
    fixed_height: bool,
};

pub const Button_Component = struct {
    onclick: Gui_Event_Callback,
    text: []const u8,
};

pub const Draggable_Component = struct {
    state: union(enum) {
        snapped_in: struct {
            container_id: Entity_Id,
            clicked_in_coords: ?Vector,
        },
        snapped_out: struct {
            old_container_id: Entity_Id,
            old_mouse_offset: Vector,
        },
        animated: struct {},
    },
    hovered_not_clicked: bool = false,
};

pub const Recepticle_Component = struct {};

pub fn init() void {
    gui_alloc = std.heap.page_allocator;
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
    try world.register_component(Container_Component);
    try world.register_component(Button_Component);
    try world.register_component(Draggable_Component);
    try world.register_component(Recepticle_Component);
}

pub fn create_spacer(world: *World, size: Vector) !Entity_Id {
    const entity_id = try world.create_entity().with(
        Gui_Component{
            .size = size,
        },
    ).build();

    return entity_id;
}

pub fn create_image(world: *World, image: SDL.Texture) !Entity_Id {
    const query = try image.query();
    const label_id = try world.create_entity().with(
        Gui_Component{
            .size = .{ .x = @floatFromInt(query.width), .y = @floatFromInt(query.height) },
        },
    ).with(
        Image_Component{ .texture = image },
    ).build();

    return label_id;
}

pub fn create_progress_bar(world: *World, value: f32, size: Vector) !Entity_Id {
    const entity_id = try world.create_entity().with(
        Gui_Component{
            .size = size,
        },
    ).with(
        Progress_Bar_Component{ .value = value },
    ).build();

    return entity_id;
}

pub fn create_checkbox(world: *World, value: bool) !Entity_Id {
    const entity_id = try world.create_entity().with(
        Gui_Component{
            .size = .{ .x = 20, .y = 20 },
        },
    ).with(
        Checkbox_Component{ .value = value },
    ).build();

    return entity_id;
}

// TODO: Accept formatting
pub fn create_label(world: *World, font_id: font_.Font_Id, comptime fmt: []const u8, args: anytype) !Entity_Id {
    const font_mgr = world.get_resource(font_.Font_Manager_Resource);
    var font = font_mgr.get_font(font_id);
    const duped_text = try std.fmt.allocPrint(gui_alloc, fmt, args);
    const width = font.width(duped_text);
    const height = font.height;
    const entity_id = try world.create_entity().with(
        Gui_Component{
            .size = .{ .x = @floatFromInt(width), .y = @floatFromInt(height) },
        },
    ).with(
        Label_Component{ .text = duped_text },
    ).with(
        Font_Id_Component{ .font_id = font_id },
    ).build();

    return entity_id;
}

pub fn create_rocker_switch(world: *World, value: bool, onchange: Gui_Event_Callback) !Entity_Id {
    const entity_id = try world.create_entity().with(
        Gui_Component{
            .size = .{ .x = 44, .y = 20 },
        },
    ).with(
        Rocker_Switch_Component{ .value = value, .onchange = onchange },
    ).with(
        Animation_Component{ .value = if (value) 1.0 else 0.0 },
    ).build();

    return entity_id;
}

pub fn create_slider(world: *World, value: f32, width: usize, n_notches: usize, onchange: ?Gui_Event_Callback) !Entity_Id {
    const entity_id = try world.create_entity().with(
        Gui_Component{
            .size = .{ .x = @floatFromInt(width), .y = 52 },
        },
    ).with(
        Slider_Component{
            .value = value,
            .onchange = onchange,
            .n_notches = n_notches,
        },
    ).build();

    return entity_id;
}

pub fn create_container(world: *World, pos: Vector, flow_align: Container_Flow_Align, fixed_width: bool, fixed_height: bool) !Entity_Id {
    const entity_id = try world.create_entity().with(
        Gui_Component{
            .pos = pos,
            .size = .{ .x = 0, .y = 0 },
        },
    ).with(
        Container_Component{
            .children = std.ArrayList(Entity_Id).init(gui_alloc),
            .flow_align = flow_align,
            .fixed_width = fixed_width,
            .fixed_height = fixed_height,
        },
    ).build();

    return entity_id;
}

// TODO: Think of a common arg layout for all these
pub fn create_button(world: *World, text: []const u8, font_id: font_.Font_Id, onclick: Gui_Event_Callback) !Entity_Id {
    const label_id = try world.create_entity().with(
        Gui_Component{
            .size = .{ .x = 220, .y = 40 },
        },
    ).with(
        Button_Component{
            .onclick = onclick,
            .text = text,
        },
    ).with(
        Font_Id_Component{ .font_id = font_id },
    ).build();

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

pub fn get_size(world: *World, entity_id: Entity_Id) Vector {
    const gui = world.get_component(Gui_Component, entity_id);
    return gui.size;
}

// TODO: Create a builder architecture so that you don't have to call these when you build/clutter component stuff
pub fn set_size(world: *World, entity_id: Entity_Id, size: Vector) void {
    const gui = world.get_component(Gui_Component, entity_id);
    gui.size = size;
    // TODO: Trigger re-flow
}

pub fn get_margin(world: *World, entity_id: Entity_Id) Vector {
    const gui = world.get_component(Gui_Component, entity_id);
    return gui.margin;
}

pub fn set_margin(world: *World, entity_id: Entity_Id, margin: Vector) void {
    const gui = world.get_component(Gui_Component, entity_id);
    gui.margin = margin;
    // TODO: Trigger re-flow
}

pub fn get_padding(world: *World, entity_id: Entity_Id) Vector {
    const gui = world.get_component(Gui_Component, entity_id);
    return gui.padding;
}

pub fn set_padding(world: *World, entity_id: Entity_Id, padding: Vector) void {
    const gui = world.get_component(Gui_Component, entity_id);
    gui.padding = padding;
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

pub fn set_font_id(world: *World, font_entity_id: Entity_Id, font_id: font_.Font_Id) void {
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
    gui_alloc.free(label.text); // TODO: I don't like this...
    label.text = try std.fmt.allocPrint(gui_alloc, fmt, args);
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

/// Retrieves the eldest parent of a gui, if any. If none are found, returns an invalid entity ID.
///
/// This does *NOT* return the immediate parent. For that, just use the `.parent` field of `Gui_Component`.
pub fn get_root(world: *World, id: Entity_Id) Entity_Id {
    std.debug.assert(id.is_valid());
    var curr_id = id;
    while (true) {
        const gui = world.get_component(Gui_Component, curr_id);
        if (!gui.parent.is_valid()) {
            return curr_id;
        } else {
            curr_id = gui.parent;
        }
    }
}

pub fn add_element(world: *World, container_id: Entity_Id, element_id: Entity_Id) !void {
    const container = world.get_component(Container_Component, container_id);
    try container.children.append(element_id);
    const element_gui = world.get_component(Gui_Component, element_id);
    element_gui.parent = container_id;
    const root_id = get_root(world, container_id);
    const root_gui = world.get_component(Gui_Component, root_id);
    _ = update_layout(
        world,
        get_root(world, container_id),
        root_gui.pos,
    );
}

pub fn remove_element(world: *World, container_id: Entity_Id, element_id: Entity_Id) void {
    std.debug.assert(world.entity_is_valid(container_id));
    std.debug.assert(world.entity_is_valid(element_id));
    const container = world.get_component(Container_Component, container_id);
    var i: usize = undefined;
    for (0..container.children.items.len) |j| {
        if (container.children.items[j].equals(element_id)) {
            i = j;
            break;
        }
    } else {
        std.debug.panic("{} not in container {}", .{ element_id, container_id });
    }
    _ = container.children.orderedRemove(i);
    const element_gui = world.get_component(Gui_Component, element_id);
    const root_id = get_root(world, container_id);
    const root_gui = world.get_component(Gui_Component, root_id);
    _ = update_layout(
        world,
        get_root(world, container_id),
        root_gui.pos,
    );
    element_gui.parent = Entity_Id.INVALID_ENTITY_ID;
}

/// Removes entity from any containers, and marks it as purged.
pub fn mark_purged(world: *World, id: Entity_Id) !void {
    std.debug.assert(world.entity_is_valid(id));
    const gui = world.get_component(Gui_Component, id);
    if (gui.parent.is_valid()) {
        remove_element(world, gui.parent, id);
    }
    try world.mark_purged(id);
}

/// Removes all entities from the container, then marks entity as purged
pub fn purge_all_elements(world: *World, container_id: Entity_Id) !void {
    std.debug.assert(world.entity_is_valid(container_id));
    const container = world.get_component(Container_Component, container_id);
    for (container.children.items) |item| {
        try world.mark_purged(item);
    }
    try mark_purged(world, container_id);
}

pub fn len(world: *World, container_id: Entity_Id) usize {
    const container = world.get_component(Container_Component, container_id);
    return container.children.items.len;
}

pub fn get_child(world: *World, container_id: Entity_Id, idx: usize) Entity_Id {
    const container = world.get_component(Container_Component, container_id);
    return container.children.items[idx];
}

/// Lays out the GUI, along with any children if element is a component with children, and returns the
/// size + padding + margin.
pub fn update_layout(world: *World, id: Entity_Id, parent_pos: Vector) Vector {
    const gui = world.get_component(Gui_Component, id);
    if (!gui.shown) {
        return Vector{ .x = 0, .y = 0 };
    }

    if (!gui.parent.is_valid()) {
        // For absolute roots. They do not have margins nor padding
        gui.pos = parent_pos;
    } else {
        gui.pos = parent_pos.add(gui.padding).add(gui.margin);
    }

    if (world.entity_has_all_components(struct { c: Container_Component }, id)) {
        const container = world.get_component(Container_Component, id);
        var working_content_size = Vector{ .x = 0, .y = 0 };
        var child_placement = Vector{ .x = 0, .y = 0 };

        switch (container.flow_align) {
            .DOWN_LEFT => {
                for (0..container.children.items.len) |i| {
                    const child_id = container.children.items[i];
                    const child_gui = world.get_component(Gui_Component, child_id);
                    const child_size_padding_margin = update_layout(
                        world,
                        child_id,
                        gui.pos.add(child_placement),
                    );
                    if (child_gui.shown and child_size_padding_margin.x > 0) {
                        child_placement.y += child_size_padding_margin.y;
                        working_content_size.x = @max(working_content_size.x, child_size_padding_margin.x);
                        working_content_size.y += child_size_padding_margin.y;
                    }
                }
            },

            .DOWN_CENTER => {
                // Find size of content
                for (0..container.children.items.len) |i| {
                    const child_id = container.children.items[i];
                    const child_gui = world.get_component(Gui_Component, child_id);
                    const child_size_padding_margin = update_layout(world, child_id, gui.pos);
                    if (child_gui.shown and child_size_padding_margin.x > 0) {
                        working_content_size.x = @max(working_content_size.x, child_size_padding_margin.x);
                        working_content_size.y += child_size_padding_margin.y;
                    }
                } // Size of content is now found

                // Place children on center line
                for (0..container.children.items.len) |i| {
                    const child_id = container.children.items[i];
                    const child_size_padding_margin = update_layout(world, child_id, gui.pos.add(child_placement));
                    _ = update_layout(world, child_id, .{
                        .x = gui.pos.x + working_content_size.x / 2.0 - child_size_padding_margin.x / 2.0 + gui.padding.x + gui.margin.x,
                        .y = gui.pos.y + child_placement.y,
                    });
                    child_placement.y += child_size_padding_margin.y;
                }
            },

            .DOWN_CENTER_EVEN => {
                std.debug.assert(container.fixed_height);
                // Go through and sum up horiz-space taken
                var space_left = gui.size.y;
                for (0..container.children.items.len) |i| {
                    const child_id = container.children.items[i];
                    const child_gui = world.get_component(Gui_Component, child_id);
                    if (child_gui.shown) {
                        const child_size_padding_margin = update_layout(world, child_id, gui.pos.add(child_placement));
                        space_left -= child_size_padding_margin.y;
                        working_content_size.x = @max(working_content_size.x, child_placement.x + child_size_padding_margin.x);
                    }
                }

                // let spacer = (container.width - sum_space) / (container.len + 1)
                const spacer = space_left / (@as(f32, @floatFromInt(container.children.items.len)) + 1.0);
                child_placement.y = spacer;
                for (0..container.children.items.len) |i| {
                    const child_id = container.children.items[i];
                    const child_gui = world.get_component(Gui_Component, child_id);
                    if (child_gui.shown) {
                        const child_size_padding_margin = update_layout(world, child_id, gui.pos.add(child_placement));
                        _ = update_layout(world, child_id, .{
                            .x = gui.pos.x + working_content_size.x / 2.0 - child_size_padding_margin.x / 2.0 + gui.padding.x + gui.margin.x,
                            .y = gui.pos.y + child_placement.y,
                        });
                        child_placement.y += spacer + child_size_padding_margin.y;
                    }
                }
            },

            .LEFT_TOP => {
                for (0..container.children.items.len) |i| {
                    const child_id = container.children.items[i];
                    const child_gui = world.get_component(Gui_Component, child_id);
                    if (child_gui.shown) {
                        const child_size_padding_margin = update_layout(world, child_id, gui.pos.add(child_placement));
                        child_placement.x += child_size_padding_margin.x;
                        working_content_size.x += child_size_padding_margin.x;
                        working_content_size.y = @max(working_content_size.y, child_placement.y + child_size_padding_margin.y);
                    }
                }
            },

            .LEFT_TOP_EVEN => {
                std.debug.assert(container.fixed_width);
                // Go through and sum up horiz-space taken
                var space_left = gui.size.x;
                for (0..container.children.items.len) |i| {
                    const child_id = container.children.items[i];
                    const child_gui = world.get_component(Gui_Component, child_id);
                    if (child_gui.shown) {
                        const child_size_padding_margin = update_layout(world, child_id, gui.pos.add(child_placement));
                        space_left -= child_size_padding_margin.x;
                    }
                }

                // let spacer = (container.width - sum_space) / (container.len + 1)
                const spacer = space_left / (@as(f32, @floatFromInt(container.children.items.len)) + 1.0);
                child_placement.x = spacer;
                for (0..container.children.items.len) |i| {
                    const child_id = container.children.items[i];
                    const child_gui = world.get_component(Gui_Component, child_id);
                    if (child_gui.shown) {
                        const child_size_padding_margin = update_layout(world, child_id, gui.pos.add(child_placement));
                        child_placement.x += spacer + child_size_padding_margin.x;
                        working_content_size.y = @max(working_content_size.y, child_placement.y + child_size_padding_margin.y);
                    }
                }
            },
        }
        if (!container.fixed_width) {
            gui.size.x = working_content_size.add(gui.padding.scale(2.0)).add(gui.margin.scale(2.0)).x;
        }
        if (!container.fixed_height) {
            gui.size.y = working_content_size.add(gui.padding.scale(2.0)).add(gui.margin.scale(2.0)).y;
        }
    }
    return gui.size.add(gui.padding.scale(2.0)).add(gui.margin.scale(2.0));
}

pub fn update(world: *World) void {
    update_linear(world);
    update_ease_out(world);
    update_checkbox(world);
    update_rocker_switch(world);
    update_slider(world);
    update_button(world);
    update_draggable(world) catch unreachable;
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
            app.mouse.x > entity.gui.pos.x and
            app.mouse.x <= entity.gui.pos.x + entity.gui.size.x and
            app.mouse.y > entity.gui.pos.y and
            app.mouse.y <= entity.gui.pos.y + entity.gui.size.y;
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
            app.mouse.x > entity.gui.pos.x and
            app.mouse.x <= entity.gui.pos.x + entity.gui.size.x and
            app.mouse.y > entity.gui.pos.y and
            app.mouse.y <= entity.gui.pos.y + entity.gui.size.y;
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
            app.mouse.x > entity.gui.pos.x and
            app.mouse.x <= entity.gui.pos.x + entity.gui.size.x and
            app.mouse.y > entity.gui.pos.y and
            app.mouse.y <= entity.gui.pos.y + entity.gui.size.y;

        if ((entity.gui.is_hovered or entity.gui.clicked_in) and app.mouse_left_down) {
            const val = std.math.clamp(
                (app.mouse.x - entity.gui.pos.x) / entity.gui.size.x,
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

pub fn update_button(world: *World) void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        id: Entity_Id,
        gui: *Gui_Component,
        button: *Button_Component,
    });
    while (iter.next()) |entity| {
        entity.gui.is_hovered = (!app.mouse_left_down or entity.gui.is_hovered) and entity.gui.shown and
            app.mouse.x > entity.gui.pos.x and
            app.mouse.x <= entity.gui.pos.x + entity.gui.size.x and
            app.mouse.y > entity.gui.pos.y and
            app.mouse.y <= entity.gui.pos.y + entity.gui.size.y;
        if (entity.gui.is_hovered and app.mouse_left_down) {
            entity.gui.clicked_in = true;
        }
        if (!app.mouse_left_down) {
            if (entity.gui.clicked_in and entity.gui.is_hovered) {
                entity.button.onclick(world, entity.id);
            }
            entity.gui.clicked_in = false;
        }
    }
}

fn update_draggable(world: *World) !void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        id: Entity_Id,
        gui: *Gui_Component,
        draggable: *Draggable_Component,
    });
    while (iter.next()) |entity| {
        entity.gui.is_hovered = entity.gui.shown and
            app.mouse.x > entity.gui.pos.x and
            app.mouse.x <= entity.gui.pos.x + entity.gui.size.x and
            app.mouse.y > entity.gui.pos.y and
            app.mouse.y <= entity.gui.pos.y + entity.gui.size.y;

        switch (entity.draggable.state) {
            .snapped_in => {
                if ((entity.gui.is_hovered or (app.mouse_left_down and entity.draggable.state == .snapped_out)) and
                    ((!entity.draggable.hovered_not_clicked and !app.mouse_left_down) or entity.draggable.hovered_not_clicked))
                {
                    entity.draggable.hovered_not_clicked = true;
                } else {
                    entity.draggable.hovered_not_clicked = false;
                }

                if (entity.draggable.hovered_not_clicked and app.mouse_left_down) {
                    entity.gui.clicked_in = true;
                } else {
                    entity.gui.clicked_in = false;
                }

                if (entity.gui.clicked_in) {
                    if (entity.draggable.state.snapped_in.clicked_in_coords == null) {
                        entity.draggable.state.snapped_in.clicked_in_coords = app.mouse;
                    } else if (!entity.draggable.state.snapped_in.clicked_in_coords.?.close_squared(
                        app.mouse,
                        100,
                    )) {
                        remove_element(
                            world,
                            entity.draggable.state.snapped_in.container_id,
                            entity.id,
                        );
                        const old_coords = entity.gui.pos.sub(app.mouse);
                        const old_container_id = entity.draggable.state.snapped_in.container_id;
                        entity.draggable.state = .{
                            .snapped_out = .{
                                .old_mouse_offset = old_coords,
                                .old_container_id = old_container_id,
                            },
                        };
                    }
                }
            },
            .snapped_out => {
                entity.gui.pos = app.mouse.add(entity.draggable.state.snapped_out.old_mouse_offset);
                _ = update_layout(world, entity.id, entity.gui.pos);

                if (!app.mouse_left_down) {
                    var recepticle_iter = world.iter(struct {
                        id: Entity_Id,
                        gui: *const Gui_Component,
                        recepticle: *Recepticle_Component,
                        container: *const Container_Component,
                    });
                    var recepticle_found: ?Entity_Id = null;
                    while (recepticle_iter.next()) |recpticle_entity| {
                        if (app.mouse.x > recpticle_entity.gui.pos.x and
                            app.mouse.x <= recpticle_entity.gui.pos.x + recpticle_entity.gui.size.x and
                            app.mouse.y > recpticle_entity.gui.pos.y and
                            app.mouse.y <= recpticle_entity.gui.pos.y + recpticle_entity.gui.size.y and
                            len(world, recpticle_entity.id) == 0)
                        {
                            recepticle_found = recpticle_entity.id;
                            break;
                        }
                    }
                    if (recepticle_found) |recepticle_id| {
                        try add_element(world, recepticle_id, entity.id);
                        entity.draggable.state = .{
                            .snapped_in = .{ // TODO: Switch to animated state instead, fly to recepticle pos
                                .container_id = recepticle_id,
                                .clicked_in_coords = null,
                            },
                        };
                    } else if (entity.draggable.state.snapped_out.old_container_id.is_valid()) {
                        const old_container_id = entity.draggable.state.snapped_out.old_container_id;
                        try add_element(world, old_container_id, entity.id);
                        entity.draggable.state = .{
                            .snapped_in = .{ // TODO: Switch to animated state instead, fly to recepticle pos
                                .container_id = old_container_id,
                                .clicked_in_coords = null,
                            },
                        };
                    }
                }
            },
            .animated => {},
        }
    }
}

pub fn render(world: *World) !void {
    const app = world.get_resource(App);
    app.background = palette.background;

    try render_container(world);
    try render_image(world);
    try render_progress_bar(world);
    try render_checkbox(world);
    try render_label(world);
    try render_rocker_switch(world);
    try render_slider(world);
    try render_button(world);
}

// TODO: Turn into render_border
fn render_container(world: *World) !void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *const Gui_Component,
        container: *const Container_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }
        const dest = SDL.RectangleF{
            .x = entity.gui.pos.x - entity.gui.padding.x,
            .y = entity.gui.pos.y - entity.gui.padding.y,
            .width = entity.gui.size.x + entity.gui.padding.x * 2,
            .height = entity.gui.size.y + entity.gui.padding.y * 2,
        };
        try app.renderer.setColorRGBA(0, 0, 0, 20);
        try app.renderer.drawRectF(dest);
    }
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
        // Draw full bar
        try app.renderer.setColor(palette.border);
        var rect = SDL.Rectangle{
            .x = @intFromFloat(entity.gui.pos.x),
            .y = @intFromFloat(entity.gui.pos.y),
            .width = @intFromFloat(entity.gui.size.x),
            .height = 6,
        };
        try app.renderer.fillRect(rect);

        // Draw completed bar
        try app.renderer.setColor(palette.active);
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

        var rect = SDL.RectangleF{
            .x = entity.gui.pos.x,
            .y = entity.gui.pos.y,
            .width = 20,
            .height = 20,
        };

        // Fill in box with color
        if (entity.gui.clicked_in) {
            try app.renderer.setColor(palette.active);
            try app.renderer.fillRectF(rect);
        } else if (entity.checkbox.value) {
            try app.renderer.setColor(palette.active);
            try app.renderer.fillRectF(rect);
        }

        // Draw border
        if (entity.gui.clicked_in) {
            try app.renderer.setColor(palette.border);
        } else if (entity.gui.is_hovered) {
            try app.renderer.setColor(palette.border);
        } else if (!entity.checkbox.value) {
            try app.renderer.setColor(palette.border);
        }
        try app.renderer.drawRectF(rect);
        rect.x += 1;
        rect.y += 1;
        rect.width -= 2;
        rect.height -= 2;
        try app.renderer.drawRectF(rect);

        // Draw check
        if (entity.checkbox.value) {
            try app.renderer.setColor(palette.text);
            try app.renderer.drawLineF(
                entity.gui.pos.x + 2,
                entity.gui.pos.y + 10,
                entity.gui.pos.x + 7,
                entity.gui.pos.y + 15,
            );
            try app.renderer.drawLineF(
                entity.gui.pos.x + 2,
                entity.gui.pos.y + 11,
                entity.gui.pos.x + 7,
                entity.gui.pos.y + 16,
            );
            try app.renderer.drawLineF(
                entity.gui.pos.x + 7,
                entity.gui.pos.y + 15,
                entity.gui.pos.x + 17,
                entity.gui.pos.y + 5,
            );

            try app.renderer.drawLineF(
                entity.gui.pos.x + 7,
                entity.gui.pos.y + 16,
                entity.gui.pos.x + 17,
                entity.gui.pos.y + 6,
            );
        }
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
            palette.text,
            entity.label.text,
        );
    }
}

fn render_rocker_switch(world: *World) !void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *const Gui_Component,
        rocker_switch: *const Rocker_Switch_Component,
        animation: *const Animation_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }
        // Draw border
        var rect = SDL.RectangleF{
            .x = entity.gui.pos.x,
            .y = entity.gui.pos.y,
            .width = 44,
            .height = 20,
        };
        if (entity.gui.clicked_in) {
            try app.renderer.setColor(palette.active);
        } else if (entity.gui.is_hovered) {
            if (entity.rocker_switch.value) {
                try app.renderer.setColor(palette.hover);
            } else {
                try app.renderer.setColor(palette.hover);
            }
        } else {
            if (entity.rocker_switch.value) {
                try app.renderer.setColor(palette.active);
            } else {
                try app.renderer.setColor(palette.border);
            }
        }
        if (entity.rocker_switch.value or entity.gui.clicked_in) {
            try app.renderer.fillRectF(rect);
        } else {
            try app.renderer.drawRectF(rect);
            rect.x += 1;
            rect.y += 1;
            rect.width -= 2;
            rect.height -= 2;
            try app.renderer.drawRectF(rect);
        }

        // Draw switch
        const switch_rect = SDL.RectangleF{
            .x = entity.gui.pos.x + 5 + 24 * entity.animation.value,
            .y = entity.gui.pos.y + 5,
            .width = 10,
            .height = 10,
        };
        if (entity.rocker_switch.value) {
            try app.renderer.setColor(palette.white);
        } else if (entity.gui.clicked_in) {
            try app.renderer.setColor(palette.grab);
        } else {
            if (entity.gui.is_hovered) {
                try app.renderer.setColor(palette.hover);
            } else {
                try app.renderer.setColor(palette.border);
            }
        }
        try app.renderer.fillRectF(switch_rect);
    }
}

fn render_slider(world: *World) !void {
    const app = world.get_resource(App);
    var iter = world.iter(struct {
        gui: *const Gui_Component,
        slider: *const Slider_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }
        const slider_offset = 6 + 12 + 16;
        const slider_value_denom = if (entity.slider.n_notches == 0) 1 else @as(f32, @floatFromInt(entity.slider.n_notches - 1));

        // Draw full slider track
        if (entity.gui.is_hovered) {
            try app.renderer.setColor(palette.border);
        } else {
            try app.renderer.setColor(palette.grab);
        }
        var rect = SDL.RectangleF{
            .x = entity.gui.pos.x,
            .y = entity.gui.pos.y + slider_offset,
            .width = entity.gui.size.x,
            .height = 2,
        };
        try app.renderer.fillRectF(rect);

        // Draw filled slider track
        try app.renderer.setColor(palette.active);
        rect = .{
            .x = entity.gui.pos.x,
            .y = entity.gui.pos.y + slider_offset,
            .width = entity.gui.size.x * (entity.slider.value) / slider_value_denom,
            .height = 2,
        };
        try app.renderer.fillRectF(rect);

        // Draw notches
        const notch_width = entity.gui.size.x / (@as(f32, @floatFromInt(entity.slider.n_notches)) - 1.0);
        try app.renderer.setColor(palette.border);
        for (0..entity.slider.n_notches) |i| {
            const f = @as(f32, @floatFromInt(i));
            try app.renderer.drawLineF(
                entity.gui.pos.x + f * notch_width,
                entity.gui.pos.y + 7 + 16,
                entity.gui.pos.x + f * notch_width,
                entity.gui.pos.y + 7 + 16 + 5,
            );
            try app.renderer.drawLineF(
                entity.gui.pos.x + f * notch_width,
                entity.gui.pos.y + 7 + 16 + 18,
                entity.gui.pos.x + f * notch_width,
                entity.gui.pos.y + 7 + 16 + 5 + 18,
            );
        }

        // Draw knob
        const knob_rect = SDL.RectangleF{
            .x = entity.gui.pos.x + entity.gui.size.x * (entity.slider.value) / slider_value_denom - 4,
            .y = entity.gui.pos.y + 11 + 12,
            .width = 8,
            .height = 24,
        };
        try app.renderer.setColor(palette.active);
        try app.renderer.fillRectF(knob_rect);
    }
}

fn render_button(world: *World) !void {
    const app = world.get_resource(App);
    const font_mgr = world.get_resource(font_.Font_Manager_Resource);
    var iter = world.iter(struct {
        gui: *const Gui_Component,
        button: *const Button_Component,
        font: *const Font_Id_Component,
    });
    while (iter.next()) |entity| {
        if (!entity.gui.shown) {
            continue;
        }

        var rect = SDL.RectangleF{
            .x = entity.gui.pos.x,
            .y = entity.gui.pos.y,
            .width = entity.gui.size.x,
            .height = entity.gui.size.y,
        };

        // Fill in box with color
        if (entity.gui.clicked_in) {
            try app.renderer.setColor(palette.border);
        } else {
            try app.renderer.setColor(palette.solid);
        }
        try app.renderer.fillRectF(rect);

        // Draw border
        if (entity.gui.clicked_in or entity.gui.is_hovered) {
            try app.renderer.setColor(palette.border);
            try app.renderer.drawRectF(rect);
            rect.x += 1;
            rect.y += 1;
            rect.width -= 2;
            rect.height -= 2;
            try app.renderer.drawRectF(rect);
        }

        // Draw text
        const font = font_mgr.get_font(entity.font.font_id);
        const text_width = @as(f32, @floatFromInt(font.width(entity.button.text)));
        const text_height = @as(f32, @floatFromInt(font.height));
        try font.draw(
            app.renderer,
            @intFromFloat(entity.gui.pos.x + entity.gui.size.x / 2.0 - text_width / 2.0),
            @intFromFloat(entity.gui.pos.y + entity.gui.size.y / 2.0 - text_height / 2.0),
            palette.text,
            entity.button.text,
        );
    }
}
